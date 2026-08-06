//! Run-owned command ownership at the scheduler boundary.
//!
//! The defect these pin: an orchestration run could lose every caller-side
//! handle to an AI command while the runner task and its process group were
//! still alive, and then report terminal stop, release preparation, or return
//! run-fatal failure anyway. The scope is the missing ownership layer; these
//! tests cover the three places the run must consult it — construction, the
//! terminal barriers, and the deadline arithmetic that bounds them.
//!
//! Everything here uses constructed outcomes, a merge-result channel double, and
//! fake registrations, so no real agent process or worktree is created and the
//! whole module stays well under the default-suite one-second budget.

use crate::ai_command_runner::RunCommandScope;
use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::parallel::orchestration::{
    remaining_cleanup_budget, CANCELLATION_MERGE_DRAIN_DEADLINE, RUN_COMMAND_CLEANUP_DEADLINE,
};
use crate::parallel::{
    MergeResult, MergeResultOrigin, MergeTaskOutcome, ParallelEvent, ParallelExecutor,
    SchedulerLifetime,
};
use crate::parallel_run_service::ParallelRunService;
use crate::tui::orchestrator::PARALLEL_CANCELLATION_CLEANUP_DEADLINE;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::mpsc;

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Bounded must-arrive wait. Only reached when an assertion has already failed.
const EVENT_WAIT: Duration = Duration::from_secs(5);

/// Window in which the scheduler must NOT return.
const BARRIER_WINDOW: Duration = Duration::from_millis(200);

fn test_config(workspace_base: &std::path::Path) -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    }
}

fn idle_analyzer(
) -> impl for<'a> Fn(&'a [crate::openspec::Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync
{
    move |_changes: &[crate::openspec::Change],
          _in_flight: &[String],
          _iteration: u32|
          -> AnalysisFuture<'_> {
        Box::pin(async move {
            AnalysisOutcome::new(
                AnalysisResult {
                    order: Vec::new(),
                    dependencies: std::collections::HashMap::new(),
                    groups: None,
                },
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

fn init_minimal_git_repo(repo_root: &std::path::Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test User"],
    ] {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git setup command");
        assert!(output.status.success(), "git setup command failed");
    }
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for args in [vec!["add", "-A"], vec!["commit", "-m", "Base"]] {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git commit command");
        assert!(output.status.success(), "git commit command failed");
    }
}

// ---------------------------------------------------------------------------
// Construction: one scope identity across every run command surface
// ---------------------------------------------------------------------------

/// Production run-owned command paths, by the parameter they all receive.
///
/// Named here so a new surface added without a scoped runner is a visible
/// omission rather than a silent one; the source guard below is what actually
/// enforces it.
const RUN_COMMAND_SURFACES: &[&str] = &[
    "analyze",
    "apply",
    "archive",
    "acceptance",
    "cleanup review",
    "rejection review",
    "conflict resolve",
    "upstream repair",
];

/// Files that build or hand out run-owned command runners.
///
/// Every one of them must go through `AiCommandRunner::for_run` (or clone a
/// runner that did). A runner built from a bare stagger timestamp on these
/// paths launches a command that no run cleanup barrier owns — which is exactly
/// the defect this change exists to remove.
const RUN_OWNED_RUNNER_SOURCES: &[(&str, &str)] = &[
    (
        "src/parallel_run_service.rs",
        include_str!("../../parallel_run_service.rs"),
    ),
    ("src/parallel/builder.rs", include_str!("../builder.rs")),
    ("src/parallel/conflict.rs", include_str!("../conflict.rs")),
    ("src/parallel/merge.rs", include_str!("../merge.rs")),
    (
        "src/upstream/repair.rs",
        include_str!("../../upstream/repair.rs"),
    ),
    (
        "src/tui/orchestrator.rs",
        include_str!("../../tui/orchestrator.rs"),
    ),
];

#[test]
fn run_command_scope_is_shared_by_every_run_command_surface() {
    assert_eq!(
        RUN_COMMAND_SURFACES.len(),
        8,
        "the enumerated surface list is the checklist this guard is written against"
    );

    for (path, source) in RUN_OWNED_RUNNER_SOURCES {
        for forbidden in [
            "AiCommandRunner::from_orchestrator_config(",
            "AiCommandRunner::new(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} constructs an unscoped run-owned runner via `{forbidden}`; \
                 run paths must use `AiCommandRunner::for_run` or clone the invocation runner \
                 so {RUN_COMMAND_SURFACES:?} all share one scope"
            );
        }
    }
}

#[test]
fn run_command_scope_construction_binds_service_executor_and_runner() {
    let repo = TempDir::new().expect("repo tempdir");
    let workspace_base = TempDir::new().expect("workspace base");
    let service = ParallelRunService::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
    );
    let service_scope = service.run_command_scope();

    let executor = service.create_executor_with_queue_state(None, None, None, None, None, None);

    assert!(
        executor.run_command_scope().is_same(&service_scope),
        "the executor must adopt the invocation scope, not build a second one"
    );
    assert!(
        executor
            .ai_runner_for_test()
            .run_command_scope()
            .is_some_and(|scope| scope.is_same(&service_scope)),
        "the shared runner every operation clones must carry that same scope"
    );
}

#[test]
fn run_command_scope_is_fresh_for_each_run_owner() {
    let repo = TempDir::new().expect("repo tempdir");
    let workspace_base = TempDir::new().expect("workspace base");
    let first = ParallelRunService::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
    );
    let second = ParallelRunService::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
    );

    let first_scope = first.run_command_scope();
    first_scope.close();

    assert!(
        !second.run_command_scope().is_same(&first_scope),
        "a closed scope is never reused; a new run starts with open admission"
    );
    assert!(!second.run_command_scope().is_closed());
}

// ---------------------------------------------------------------------------
// Deadlines: one absolute cancellation timeline
// ---------------------------------------------------------------------------

#[test]
fn run_command_scope_deadlines_fit_inside_the_outer_boundary() {
    assert_eq!(
        RUN_COMMAND_CLEANUP_DEADLINE,
        Duration::from_secs(30),
        "the command cleanup budget is fixed, with no new configuration surface"
    );
    assert_eq!(CANCELLATION_MERGE_DRAIN_DEADLINE, Duration::from_secs(90));
    assert!(
        RUN_COMMAND_CLEANUP_DEADLINE + CANCELLATION_MERGE_DRAIN_DEADLINE
            <= PARALLEL_CANCELLATION_CLEANUP_DEADLINE,
        "nested cleanup must not additively exceed the outer scheduler boundary"
    );
}

#[test]
fn run_command_scope_nested_waits_share_one_absolute_start() {
    let outer = Duration::from_secs(120);
    // Nothing consumed yet: each wait gets its own cap.
    let fresh = Instant::now();
    assert_eq!(
        remaining_cleanup_budget(fresh, outer, RUN_COMMAND_CLEANUP_DEADLINE),
        RUN_COMMAND_CLEANUP_DEADLINE
    );

    // A shutdown that already burned most of the outer boundary hands the merge
    // drain what is left, not a fresh 90 seconds.
    let nearly_spent = Instant::now() - Duration::from_secs(115);
    let remaining =
        remaining_cleanup_budget(nearly_spent, outer, CANCELLATION_MERGE_DRAIN_DEADLINE);
    assert!(
        remaining <= Duration::from_secs(5),
        "the merge drain must not reset the timeline: {remaining:?}"
    );

    let expired = Instant::now() - Duration::from_secs(200);
    assert_eq!(
        remaining_cleanup_budget(expired, outer, CANCELLATION_MERGE_DRAIN_DEADLINE),
        Duration::ZERO,
        "an exhausted timeline yields no further budget"
    );
}

// ---------------------------------------------------------------------------
// Run-fatal and cancellation: the terminal cleanup barrier
// ---------------------------------------------------------------------------

fn test_change(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

/// Drive the scheduler future for `window` while collecting its events.
///
/// Completion is latched into `finished` rather than assumed: polling a future
/// that already returned is a panic, and "did it return yet?" is exactly the
/// question these barrier tests ask.
async fn pump<F>(
    scheduler: &mut Pin<Box<F>>,
    finished: &mut Option<F::Output>,
    events: &mut mpsc::Receiver<ParallelEvent>,
    collected: &mut Vec<ParallelEvent>,
    window: Duration,
) where
    F: Future,
{
    if finished.is_some() {
        return;
    }
    let deadline = Instant::now() + window;
    while Instant::now() < deadline {
        if let Ok(output) = tokio::time::timeout(Duration::from_millis(10), &mut *scheduler).await {
            *finished = Some(output);
        }
        // Drain before breaking: the terminal event is published inside the
        // scheduler, just before it returns.
        while let Ok(event) = events.try_recv() {
            collected.push(event);
        }
        if finished.is_some() {
            break;
        }
    }
}

fn global_errors(collected: &[ParallelEvent]) -> Vec<String> {
    collected
        .iter()
        .filter_map(|event| match event {
            ParallelEvent::Error { message } => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn terminal_events(collected: &[ParallelEvent]) -> Vec<&'static str> {
    collected
        .iter()
        .filter_map(|event| match event {
            ParallelEvent::AllCompleted => Some("AllCompleted"),
            ParallelEvent::Stopped => Some("Stopped"),
            _ => None,
        })
        .collect()
}

/// The run-fatal contract has two halves that must not be traded for each
/// other: the operator hears about it immediately, and the scheduler still does
/// not return failure until every run-owned command is quiescent.
///
/// A held registration keeps the barrier open, so the Error must be observable
/// while the scheduler future is provably still pending.
#[tokio::test]
async fn run_fatal_error_precedes_cleanup_barrier() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);

    let (merge_result_tx, merge_result_rx) = mpsc::channel(8);
    merge_result_tx
        .try_send(MergeResult {
            change_id: "alpha".to_string(),
            workspace_name: "ws-alpha".to_string(),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome: MergeTaskOutcome::run_fatal("base branch could not be identified"),
        })
        .expect("merge-result double must accept the pre-loaded outcome");
    executor.merge_result_channel_override = Some((merge_result_tx, merge_result_rx));
    executor.pending_merge_count.store(1, Ordering::SeqCst);

    // A run-owned command that has not reported cleanup yet.
    let scope = executor.run_command_scope().clone();
    let held = scope
        .register_for_test("apply", Some("alpha"))
        .expect("an open scope admits the registration");

    let mut scheduler = Box::pin(
        executor.execute_with_order_based_reanalysis(vec![test_change("beta")], idle_analyzer()),
    );
    let mut finished = None;
    let mut collected = Vec::new();

    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        BARRIER_WINDOW,
    )
    .await;

    // Half one: prompt. The Error is published without waiting for cleanup.
    let errors = global_errors(&collected);
    assert_eq!(
        errors.len(),
        1,
        "exactly one prompt global Error, got {errors:?}"
    );
    assert!(
        errors[0].contains("base branch could not be identified"),
        "{}",
        errors[0]
    );
    assert!(
        scope.is_closed(),
        "run-fatal shutdown closes run command admission before it waits"
    );

    // Half two: ordered. While the registration is held, the scheduler owes its
    // barrier and must not have returned failure.
    assert!(
        finished.is_none(),
        "scheduler failure return must wait for run-owned command cleanup"
    );

    held.release_confirmed();
    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        EVENT_WAIT,
    )
    .await;

    let result = finished.expect("the barrier releases once the command reports quiescence");
    assert!(
        result.is_err(),
        "the run-fatal outcome still terminates the scheduler as failure, got {result:?}"
    );
    assert_eq!(
        global_errors(&collected).len(),
        1,
        "no second global Error is emitted for the same run-fatal outcome"
    );
    assert!(
        terminal_events(&collected).is_empty(),
        "a run-fatal run emits neither Stopped nor AllCompleted, got {:?}",
        terminal_events(&collected)
    );
}

/// Operator cancellation reaches terminal `Stopped` only after the same
/// barrier.
#[tokio::test]
async fn run_command_scope_gates_terminal_stop_on_cancellation() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
    let cancel = tokio_util::sync::CancellationToken::new();
    executor.set_cancel_token(cancel.clone());

    let scope = executor.run_command_scope().clone();
    let held = scope
        .register_for_test("apply", Some("alpha"))
        .expect("an open scope admits the registration");

    let mut scheduler = Box::pin(
        executor.execute_with_order_based_reanalysis(vec![test_change("alpha")], idle_analyzer()),
    );
    let mut finished = None;
    let mut collected = Vec::new();

    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        Duration::from_millis(80),
    )
    .await;
    cancel.cancel();
    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        BARRIER_WINDOW,
    )
    .await;

    assert!(
        scope.is_closed(),
        "cancellation closes admission before the workspace futures are aborted"
    );
    assert!(
        finished.is_none(),
        "terminal stop must wait for run-owned command quiescence"
    );
    assert!(
        terminal_events(&collected).is_empty(),
        "Stopped must not precede the cleanup barrier, got {:?}",
        terminal_events(&collected)
    );

    held.release_confirmed();
    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        EVENT_WAIT,
    )
    .await;

    let report = finished
        .expect("the barrier releases once the command reports quiescence")
        .expect("operator cancellation is never an execution failure");
    assert_eq!(report, crate::parallel::SchedulerRunReport::Stopped);
    assert_eq!(terminal_events(&collected), vec!["Stopped"]);
    assert!(
        global_errors(&collected).is_empty(),
        "cancellation is not converted into an execution failure, got {:?}",
        global_errors(&collected)
    );
}

/// A cancellation whose command cleanup cannot be proven stays a cancellation
/// and reports an actionable diagnostic instead of a failure.
#[tokio::test]
async fn run_command_scope_unproven_cleanup_stays_operator_cancellation() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
    // Keep the whole test inside the default one-second budget; production uses
    // the fixed 30-second bound and adds no configuration surface.
    executor.set_cancellation_cleanup_budget_for_test(Duration::from_millis(60));
    let cancel = tokio_util::sync::CancellationToken::new();
    executor.set_cancel_token(cancel.clone());

    let scope = executor.run_command_scope().clone();
    // Never released: the barrier can only end by expiring.
    let _held = scope
        .register_for_test("acceptance", Some("alpha"))
        .expect("an open scope admits the registration");

    let mut scheduler = Box::pin(
        executor.execute_with_order_based_reanalysis(vec![test_change("alpha")], idle_analyzer()),
    );
    let mut finished = None;
    let mut collected = Vec::new();

    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        Duration::from_millis(80),
    )
    .await;
    cancel.cancel();
    pump(
        &mut scheduler,
        &mut finished,
        &mut events,
        &mut collected,
        EVENT_WAIT,
    )
    .await;

    let report = finished
        .expect("the bounded barrier must not hang")
        .expect("an unproven cleanup is a diagnostic, not an execution failure");
    assert_eq!(report, crate::parallel::SchedulerRunReport::Stopped);
    assert!(
        global_errors(&collected).is_empty(),
        "still a cancellation, got {:?}",
        global_errors(&collected)
    );

    let logs: Vec<String> = collected
        .iter()
        .filter_map(|event| match event {
            ParallelEvent::Log(entry) => Some(entry.message.clone()),
            _ => None,
        })
        .collect();
    assert!(
        logs.iter().any(|message| message
            .contains("Run-owned command cleanup could not be fully proven")
            && message.contains("op=acceptance")),
        "the operator must be told what could not be proven, got {logs:?}"
    );
}

/// The scope is invocation-scoped in-memory state only.
///
/// `openspec/CONSTITUTION.md` requires next-action routing to be derivable from
/// the workspace alone, so a scope must leave nothing behind for a later run to
/// read.
#[test]
fn run_command_scope_state_is_process_local_only() {
    let scope = RunCommandScope::new();
    let registration = scope.register_unproven_for_test("apply", Some("alpha"), 999_999);
    registration.release_confirmed();
    assert!(!scope.retained_process_ids().is_empty());

    drop(scope);

    // A new run starts from nothing: no registry, no identities, open admission.
    let restarted = RunCommandScope::new();
    assert!(!restarted.is_closed());
    assert_eq!(restarted.active_executions(), 0);
    assert!(restarted.retained_process_ids().is_empty());
    assert!(restarted.change_is_quiescent("alpha"));
}
