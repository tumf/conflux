//! Regressions for the failed-dependency reanalysis loop.
//!
//! A change whose dependency failed used to be deleted from the scheduler-local
//! candidate list while reducer queue intent still held it. Reconciliation then
//! restored it on the next pass, reported a synthetic queue addition, armed a
//! `QueueNotification` edge, bypassed unchanged-input suppression, re-analysed,
//! re-skipped, and deleted it again — an unbounded cycle of analyzer
//! invocations, repository scans, and identical `Skipped ... Dependency ...
//! failed` diagnostics with no state transition behind any of it.
//!
//! Evidence classes used here:
//! - integration: real scheduler loop / real reducer state / real `DynamicQueue`
//!   under paused Tokio time, plus real repository fixtures
//!
//! The assertions are deliberately exact in both directions. A zero-analysis
//! no-op, a log-only patch, analyzer-only suppression, and retained local queue
//! churn each fail at least one of them.

use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::queue_state::QueuedWorkClass;
use crate::parallel::{ParallelEvent, ParallelExecutor, SchedulerLifetime, SchedulerRunReport};
use crate::tui::queue::DynamicQueue;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

/// Comfortably longer than the 10s queue-coalescing debounce, so every wake this
/// suite drives is debounce-eligible. A wake that cannot pass debounce would
/// hide the loop under test instead of exercising it.
const DEBOUNCE_ELIGIBLE_WAKE: Duration = Duration::from_secs(30);

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

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

fn test_change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn git(repo_root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("run git command");
    assert!(output.status.success(), "git {:?} failed", args);
}

/// Write one active OpenSpec change with explicit dependency metadata.
fn write_change(repo_root: &std::path::Path, change_id: &str, dependencies: &[&str]) {
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).expect("create change directory");
    let declared = dependencies
        .iter()
        .map(|dependency| format!("  - {dependency}\n"))
        .collect::<String>();
    std::fs::write(
        change_dir.join("proposal.md"),
        format!("---\ndependencies:\n{declared}---\n# {change_id}\n"),
    )
    .expect("write proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .expect("write tasks");
}

/// A committed repository holding `a` (the blocker), `b` (its dependent), and
/// the independent `c`.
fn init_repo() -> TempDir {
    let repo = TempDir::new().expect("create temp repo");
    let root = repo.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "test@example.com"]);
    git(root, &["config", "user.name", "Test User"]);
    std::fs::write(root.join("README.md"), "base\n").expect("write base file");
    write_change(root, "a", &[]);
    write_change(root, "b", &["a"]);
    write_change(root, "c", &[]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-m", "Base"]);
    repo
}

/// Move one active change into the archive and commit it to the base branch.
///
/// This is the authoritative resolution evidence dispatch checks read: the
/// archive commit is reachable from the effective dependency base and the
/// active change directory is gone.
fn archive_to_base(repo_root: &std::path::Path, archive_leaf: &str, change_id: &str) {
    let archive_dir = repo_root
        .join("openspec/changes/archive")
        .join(archive_leaf);
    std::fs::create_dir_all(&archive_dir).expect("create archive directory");
    std::fs::write(
        archive_dir.join("proposal.md"),
        format!("# Archived {change_id}\n"),
    )
    .expect("write archived proposal");
    let active_dir = repo_root.join("openspec/changes").join(change_id);
    if active_dir.exists() {
        std::fs::remove_dir_all(&active_dir).expect("remove active change directory");
    }
    git(repo_root, &["add", "-A"]);
    git(
        repo_root,
        &["commit", "-m", &format!("Archive {change_id}")],
    );
}

/// Reducer state carrying ordinary queue intent for `queued_ids`.
///
/// Every candidate the scheduler holds locally must also be reducer-queued, or
/// reconciliation revokes it — which is a different code path from the one
/// under test.
async fn reducer_state(known: &[&str], queued_ids: &[&str]) -> Arc<RwLock<OrchestratorState>> {
    let state = Arc::new(RwLock::new(OrchestratorState::new(
        known.iter().map(|id| id.to_string()).collect(),
        10,
    )));
    {
        let mut guard = state.write().await;
        for id in queued_ids {
            guard.apply_command(ReducerCommand::AddToQueue(id.to_string()));
        }
    }
    state
}

/// Executor with the ephemeral failure state a failed `a` leaves behind.
///
/// `set_dependencies` mirrors what `handle_post_analysis_capacity` records from
/// a completed analysis, and `mark_failed` mirrors what a failed workspace task
/// records. Both are process-local: a restart begins with an empty tracker.
fn seed_failed_blocker(executor: &mut ParallelExecutor, dependent: &str, blocker: &str) {
    let mut dependencies = HashMap::new();
    dependencies.insert(dependent.to_string(), vec![blocker.to_string()]);
    executor.failed_tracker.set_dependencies(dependencies);
    executor.failed_tracker.mark_failed(blocker);
}

fn dependency_analysis(
    order: &[&str],
    dependencies: HashMap<String, Vec<String>>,
) -> AnalysisResult {
    AnalysisResult {
        order: order.iter().map(|id| id.to_string()).collect(),
        dependencies,
        groups: None,
    }
}

fn b_depends_on_a() -> HashMap<String, Vec<String>> {
    let mut dependencies = HashMap::new();
    dependencies.insert("b".to_string(), vec!["a".to_string()]);
    dependencies
}

/// Analyzer double that counts invocations and keeps `b -> a` in the dependency
/// map, so a completed analysis cannot silently release the blocked dependent.
fn counting_analyzer(
    analyses: Arc<AtomicUsize>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        analyses.fetch_add(1, Ordering::SeqCst);
        let order: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        Box::pin(async move {
            AnalysisOutcome::new(
                dependency_analysis(
                    &order.iter().map(String::as_str).collect::<Vec<_>>(),
                    b_depends_on_a(),
                ),
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

// ============================================================================
// Local queue convergence and reconciliation (integration)
// ============================================================================

#[tokio::test]
async fn failed_dependency_keeps_dependent_locally_queued_and_blocked() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    let state = reducer_state(&["a", "b", "c"], &["b", "c"]).await;

    let mut executor = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    executor.set_shared_orchestrator_state(state);
    seed_failed_blocker(&mut executor, "b", "a");

    let mut queued = vec![test_change("b"), test_change("c")];
    let in_flight = HashSet::new();

    let classification = executor.classify_queued_work(&queued, &in_flight).await;
    assert_eq!(
        classification.class_for("b"),
        Some(QueuedWorkClass::DependencyBlocked),
        "a failed dependency must classify its dependent as blocked queued work"
    );
    assert_eq!(
        classification.class_for("c"),
        Some(QueuedWorkClass::DispatchableApply),
        "independent queued work must stay dispatchable"
    );

    // Repeated reconciliation passes are the loop's pressure test: a candidate
    // that is never removed can never be re-added.
    for pass in 0..5 {
        let outcome = executor
            .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
            .await;
        assert_eq!(
            outcome.queued_added, 0,
            "pass {pass}: rediscovering an already represented blocked candidate is not a queue addition"
        );
        assert_eq!(
            outcome.revoked_removed, 0,
            "pass {pass}: accepted queue intent must not be revoked by dependency blocking"
        );
        assert!(
            queued.iter().any(|change| change.id == "b"),
            "pass {pass}: the blocked dependent must remain locally represented"
        );
    }
}

#[tokio::test]
async fn failed_dependency_blocks_dispatch_selection_but_not_independent_work() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    let state = reducer_state(&["a", "b", "c"], &["b", "c"]).await;

    let mut executor = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    executor.set_shared_orchestrator_state(state);
    seed_failed_blocker(&mut executor, "b", "a");

    let analysis = dependency_analysis(&["b", "c"], b_depends_on_a());
    let selected = executor
        .select_changes_for_dispatch(&analysis, 4, &HashSet::new())
        .await;

    assert_eq!(
        selected,
        vec!["c".to_string()],
        "the failed-dependent candidate must be excluded from dispatch while the \
         independent candidate still reaches it"
    );
}

// ============================================================================
// Bounded blocker-epoch events (integration)
// ============================================================================

/// Drive the production pre-analysis pipeline repeatedly and count what it emits.
struct EpochProbe {
    executor: ParallelExecutor,
    events: mpsc::Receiver<ExecutionEvent>,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    analyses: Arc<AtomicUsize>,
    _repo: TempDir,
    _workspace_base: TempDir,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct EmittedEvents {
    change_skipped: Vec<String>,
    dependency_blocked: Vec<String>,
}

impl EpochProbe {
    async fn new(queued_ids: &[&str]) -> Self {
        let repo = init_repo();
        let workspace_base = TempDir::new().expect("workspace base");
        let state = reducer_state(&["a", "b", "c"], queued_ids).await;
        let (tx, events) = mpsc::channel(256);
        let mut executor = ParallelExecutor::new(
            repo.path().to_path_buf(),
            test_config(workspace_base.path()),
            Some(tx),
        );
        executor.set_shared_orchestrator_state(state);
        seed_failed_blocker(&mut executor, "b", "a");

        Self {
            executor,
            events,
            queued: queued_ids.iter().map(|id| test_change(id)).collect(),
            in_flight: HashSet::new(),
            analyses: Arc::new(AtomicUsize::new(0)),
            _repo: repo,
            _workspace_base: workspace_base,
        }
    }

    /// One scheduler pass: reconcile reducer intent, then run the production
    /// re-analysis/dispatch evaluation exactly as the loop does.
    async fn wake(&mut self, iteration: u32) -> usize {
        let reconciliation = self
            .executor
            .reconcile_queued_candidates_from_shared_state(&mut self.queued, &self.in_flight)
            .await;

        let analyzer = counting_analyzer(self.analyses.clone());

        let mut reanalysis_reason = if reconciliation.has_queued_additions() {
            crate::parallel::dynamic_queue::ReanalysisReason::QueueNotification
        } else {
            crate::parallel::dynamic_queue::ReanalysisReason::Initial
        };
        let mut join_set = tokio::task::JoinSet::new();
        let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
            crate::vcs::VcsBackend::Git,
            self.executor.repo_root.clone(),
        );
        self.executor
            .evaluate_queued_reanalysis_and_dispatch(
                crate::parallel::queue_state::ReanalysisDispatchContext {
                    queued: &mut self.queued,
                    in_flight: &mut self.in_flight,
                    max_parallelism: 0,
                    iteration,
                    reanalysis_reason,
                    analyzer: &analyzer,
                    semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
                    join_set: &mut join_set,
                    cleanup_guard: &mut cleanup_guard,
                    work_snapshot: None,
                },
                &mut reanalysis_reason,
            )
            .await
            .expect("scheduler evaluation must not fail");

        reconciliation.queued_added
    }

    fn drain(&mut self) -> EmittedEvents {
        let mut emitted = EmittedEvents::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::ChangeSkipped { change_id, reason } => {
                    assert!(
                        reason.contains("Dependency 'a' failed"),
                        "the compatibility reason must keep its existing wording: {reason}"
                    );
                    emitted.change_skipped.push(change_id);
                }
                ExecutionEvent::DependencyBlocked {
                    change_id,
                    dependency_ids,
                } => {
                    if dependency_ids.contains(&"a".to_string()) {
                        emitted.dependency_blocked.push(change_id);
                    }
                }
                _ => {}
            }
        }
        emitted
    }
}

#[tokio::test]
async fn failed_dependency_events_are_emitted_once_per_blocker_epoch() {
    let mut probe = EpochProbe::new(&["b"]).await;

    // Iteration 1 unconditionally analyses in the real loop, so start where the
    // scheduler already performed its first evaluation.
    for iteration in 2..8 {
        let added = probe.wake(iteration).await;
        assert_eq!(
            added, 0,
            "iteration {iteration}: an already represented blocked candidate is not a new queue edge"
        );
    }

    let emitted = probe.drain();
    assert_eq!(
        emitted.change_skipped,
        vec!["b".to_string()],
        "exactly one compatibility ChangeSkipped per blocker epoch"
    );
    assert_eq!(
        emitted.dependency_blocked,
        vec!["b".to_string()],
        "exactly one authoritative DependencyBlocked per blocker epoch"
    );
    assert_eq!(
        probe.analyses.load(Ordering::SeqCst),
        0,
        "rediscovering the same blocked candidate must not invoke the analyzer"
    );
    assert!(
        probe.queued.iter().any(|change| change.id == "b"),
        "the blocked dependent must still be locally represented"
    );
}

#[tokio::test]
async fn failed_dependency_events_reopen_when_the_blocker_set_changes() {
    let mut probe = EpochProbe::new(&["b"]).await;
    probe.wake(2).await;
    assert_eq!(probe.drain().change_skipped, vec!["b".to_string()]);

    // A second failed blocker is a genuinely different blocked state.
    let mut dependencies = HashMap::new();
    dependencies.insert("b".to_string(), vec!["a".to_string(), "c".to_string()]);
    probe.executor.failed_tracker.set_dependencies(dependencies);
    probe.executor.failed_tracker.mark_failed("c");

    probe.wake(3).await;
    probe.wake(4).await;

    let emitted = probe.drain();
    assert_eq!(
        emitted.change_skipped,
        vec!["b".to_string()],
        "a changed blocker set opens exactly one new epoch"
    );
    assert_eq!(emitted.dependency_blocked, vec!["b".to_string()]);
}

#[tokio::test]
async fn failed_dependency_dequeue_readd_is_a_genuine_state_change() {
    let mut probe = EpochProbe::new(&["b"]).await;
    probe.wake(2).await;
    assert_eq!(probe.drain().change_skipped, vec!["b".to_string()]);

    // Revoke ordinary queue intent, exactly as `RemoveFromQueue` does.
    {
        let state = probe
            .executor
            .shared_orchestrator_state
            .clone()
            .expect("reducer state");
        let mut guard = state.write().await;
        guard.apply_command(ReducerCommand::RemoveFromQueue("b".to_string()));
    }
    probe.wake(3).await;
    assert!(
        !probe.queued.iter().any(|change| change.id == "b"),
        "revocation must drop the blocked candidate from the local queue"
    );
    assert!(
        !probe.executor.failed_tracker.has_blocker_epoch("b"),
        "revocation must clear the blocker notification epoch"
    );

    // An explicit re-add is a genuine queue addition and may announce once more.
    {
        let state = probe
            .executor
            .shared_orchestrator_state
            .clone()
            .expect("reducer state");
        let mut guard = state.write().await;
        guard.apply_command(ReducerCommand::AddToQueue("b".to_string()));
    }
    let added = probe.wake(4).await;
    assert_eq!(added, 1, "an explicit re-add is a genuine queue addition");
    probe.wake(5).await;

    let emitted = probe.drain();
    assert_eq!(
        emitted.change_skipped,
        vec!["b".to_string()],
        "re-add announces exactly one new bounded blocker transition"
    );
    assert_eq!(emitted.dependency_blocked, vec!["b".to_string()]);
}

// ============================================================================
// Explicit-retry edge consumption (integration)
// ============================================================================

#[tokio::test]
async fn failed_dependency_retry_edge_clears_only_the_retried_failure() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    let state = reducer_state(&["a", "b", "c"], &["b"]).await;
    let queue = Arc::new(DynamicQueue::new());

    let mut executor = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    executor.set_shared_orchestrator_state(state);
    executor.set_dynamic_queue(queue.clone());

    let mut dependencies = HashMap::new();
    dependencies.insert("b".to_string(), vec!["a".to_string()]);
    dependencies.insert("other".to_string(), vec!["unrelated".to_string()]);
    executor.failed_tracker.set_dependencies(dependencies);
    executor.failed_tracker.mark_failed("a");
    executor.failed_tracker.mark_failed("unrelated");
    let blockers = executor.failed_tracker.failed_blockers("b");
    executor.failed_tracker.begin_blocker_epoch("b", &blockers);
    let other_blockers = executor.failed_tracker.failed_blockers("other");
    executor
        .failed_tracker
        .begin_blocker_epoch("other", &other_blockers);

    assert!(
        !executor.consume_explicit_retry_edges().await,
        "no pending edge means no armed reevaluation"
    );
    assert_eq!(
        executor.failed_tracker.should_skip("b"),
        Some("a".to_string())
    );

    queue.publish_explicit_retry("a".to_string()).await;
    assert!(
        executor.consume_explicit_retry_edges().await,
        "a consumed edge must arm exactly one reevaluation"
    );
    assert!(
        executor.failed_tracker.should_skip("b").is_none(),
        "the retried change's ephemeral failed classification must be cleared"
    );
    assert!(
        !executor.failed_tracker.has_blocker_epoch("b"),
        "the dependent's blocker notification epoch must be cleared"
    );
    assert_eq!(
        executor.failed_tracker.should_skip("other"),
        Some("unrelated".to_string()),
        "an unrelated failure must survive"
    );
    assert!(
        executor.failed_tracker.has_blocker_epoch("other"),
        "an unrelated notification epoch must survive"
    );

    assert!(
        !executor.consume_explicit_retry_edges().await,
        "the retry edge is one-shot and must not be replayable"
    );
}

#[tokio::test]
async fn failed_dependency_retry_does_not_prove_dependency_resolution() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    // `a` carries ordinary queue intent: it was retried, not resolved.
    let state = reducer_state(&["a", "b", "c"], &["a", "b"]).await;
    let queue = Arc::new(DynamicQueue::new());

    let mut executor = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    executor.set_shared_orchestrator_state(state);
    executor.set_dynamic_queue(queue.clone());
    seed_failed_blocker(&mut executor, "b", "a");

    queue.publish_explicit_retry("a".to_string()).await;
    assert!(executor.consume_explicit_retry_edges().await);

    let queued = vec![test_change("a"), test_change("b")];
    let classification = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;
    assert_eq!(
        classification.class_for("b"),
        Some(QueuedWorkClass::DependencyBlocked),
        "clearing the fast failure gate must not make an unmerged dependency look resolved"
    );

    // Refailure establishes a new epoch rather than resuming a suppressed one.
    executor.failed_tracker.mark_failed("a");
    let blockers = executor.failed_tracker.failed_blockers("b");
    assert!(
        executor.failed_tracker.begin_blocker_epoch("b", &blockers),
        "refailure after an accepted retry is a new blocker epoch"
    );
}

/// Authoritative resolution — the archive commit is present on the effective
/// dependency base and the active change directory is gone — is what actually
/// unblocks the dependent.
#[tokio::test]
async fn failed_dependency_retry_authoritative_resolution_unblocks_dependent() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    let state = reducer_state(&["a", "b", "c"], &["b"]).await;
    let queue = Arc::new(DynamicQueue::new());

    let mut executor = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    executor.set_shared_orchestrator_state(state);
    executor.set_dynamic_queue(queue.clone());
    seed_failed_blocker(&mut executor, "b", "a");

    queue.publish_explicit_retry("a".to_string()).await;
    assert!(executor.consume_explicit_retry_edges().await);

    let analysis = dependency_analysis(&["b"], b_depends_on_a());
    assert!(
        executor
            .select_changes_for_dispatch(&analysis, 2, &HashSet::new())
            .await
            .is_empty(),
        "a retried but unmerged dependency must keep its dependent blocked"
    );

    archive_to_base(repo.path(), "2026-05-12-a", "a");

    assert_eq!(
        executor
            .select_changes_for_dispatch(&analysis, 2, &HashSet::new())
            .await,
        vec!["b".to_string()],
        "authoritative repository evidence is what unblocks the dependent"
    );
}

#[tokio::test]
async fn failed_dependency_restart_discards_ephemeral_failure_tracking() {
    let repo = init_repo();
    let workspace_base = TempDir::new().expect("workspace base");
    let state = reducer_state(&["a", "b", "c"], &["b"]).await;

    let mut first = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    first.set_shared_orchestrator_state(state.clone());
    seed_failed_blocker(&mut first, "b", "a");
    let blockers = first.failed_tracker.failed_blockers("b");
    first.failed_tracker.begin_blocker_epoch("b", &blockers);
    assert!(first.failed_tracker.has_blocker_epoch("b"));
    drop(first);

    // A restart builds a fresh executor from the same workspace. Nothing about
    // the previous process's failures survives.
    let restarted = ParallelExecutor::new(
        repo.path().to_path_buf(),
        test_config(workspace_base.path()),
        None,
    );
    assert!(
        restarted.failed_tracker.failed_changes().is_empty(),
        "a restarted process must begin with an empty failure tracker"
    );
    assert!(
        !restarted.failed_tracker.has_blocker_epoch("b"),
        "a restarted process must begin with no blocker notification epochs"
    );
    assert!(
        restarted.failed_tracker.should_skip("b").is_none(),
        "routing after restart is recomputed from workspace and Git evidence"
    );
}

// ============================================================================
// Real scheduler-loop lifetime (integration, paused time)
// ============================================================================

/// Live counters a driver can synchronise on.
#[derive(Default)]
struct ObservedEvents {
    skips: AtomicUsize,
    blocked: AtomicUsize,
}

/// Everything a loop driver may touch while the real scheduler is running.
struct LoopHandles {
    queue: Arc<DynamicQueue>,
    state: Arc<RwLock<OrchestratorState>>,
    cancel: CancellationToken,
    observed: Arc<ObservedEvents>,
    analyses: Arc<AtomicUsize>,
}

/// Yield until `counter` reaches `target`.
///
/// Yielding rather than sleeping keeps this task runnable, which stops paused
/// Tokio time from auto-advancing while the scheduler is progressing through
/// real filesystem and Git work. The bound turns a genuine regression into a
/// failed assertion instead of a hung test.
async fn await_counter(counter: &AtomicUsize, target: usize, what: &str) {
    for _ in 0..1_000_000 {
        if counter.load(Ordering::SeqCst) >= target {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("scheduler loop never reached {target} {what}");
}

impl LoopHandles {
    async fn await_skips(&self, target: usize) {
        await_counter(&self.observed.skips, target, "ChangeSkipped events").await;
        await_counter(&self.observed.blocked, target, "DependencyBlocked events").await;
    }

    async fn await_analyses(&self, target: usize) {
        await_counter(&self.analyses, target, "analyzer invocations").await;
    }
}

/// What one real scheduler-loop run observed.
struct LoopRun {
    report: SchedulerRunReport,
    analyses: usize,
    change_skipped: Vec<String>,
    dependency_blocked: Vec<String>,
    all_completed: bool,
    stopped: bool,
}

/// Run the real `execute_with_order_based_reanalysis` loop over `queued`.
///
/// `drive` runs concurrently with the loop and owns the wakes and the eventual
/// cancellation, so the loop is never asked to stop before the behaviour under
/// test has been observed.
///
/// `occupy_dispatch_capacity` consumes every execution slot through the
/// automatic-resolve accounting the scheduler already uses. That keeps the
/// analysis and classification decisions under test while stopping the pass
/// before it creates real worktrees and spawns real agent commands, which is
/// what keeps these regressions on the sub-second default test path. Dispatch
/// selection itself is asserted directly in
/// `failed_dependency_blocks_dispatch_selection_but_not_independent_work`.
async fn run_loop<D, Fut>(
    repo_root: &std::path::Path,
    lifetime: SchedulerLifetime,
    queued_ids: &[&str],
    occupy_dispatch_capacity: bool,
    drive: D,
) -> LoopRun
where
    D: FnOnce(LoopHandles) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let workspace_base = TempDir::new().expect("workspace base");
    let (event_tx, mut events) = mpsc::channel(512);
    let state = reducer_state(&["a", "b", "c"], queued_ids).await;
    let queue = Arc::new(DynamicQueue::new());
    let cancel_token = CancellationToken::new();

    let mut executor = ParallelExecutor::new(
        repo_root.to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_shared_orchestrator_state(state.clone());
    executor.set_dynamic_queue(queue.clone());
    executor.set_scheduler_lifetime(lifetime);
    executor.set_cancel_token(cancel_token.clone());
    if occupy_dispatch_capacity {
        executor.auto_resolve_count.store(64, Ordering::SeqCst);
    }
    seed_failed_blocker(&mut executor, "b", "a");

    let analyses = Arc::new(AtomicUsize::new(0));
    let analyzer = counting_analyzer(analyses.clone());

    // The observed-event counters are what let a driver synchronise on real loop
    // progress. Paused Tokio time auto-advances whenever every task is idle —
    // including while the scheduler waits on a real git subprocess — so a driver
    // that only slept could cancel the run before its first pass ever ran.
    let observed = Arc::new(ObservedEvents::default());
    let forwarder_observed = observed.clone();
    let forwarder = tokio::spawn(async move {
        let mut collected = Vec::new();
        while let Some(event) = events.recv().await {
            match &event {
                ParallelEvent::ChangeSkipped { .. } => {
                    forwarder_observed.skips.fetch_add(1, Ordering::SeqCst);
                }
                ParallelEvent::DependencyBlocked { .. } => {
                    forwarder_observed.blocked.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
            collected.push(event);
        }
        collected
    });

    let driver = tokio::spawn(drive(LoopHandles {
        queue,
        state,
        cancel: cancel_token,
        observed,
        analyses: analyses.clone(),
    }));

    let report = executor
        .execute_with_order_based_reanalysis(
            queued_ids.iter().map(|id| test_change(id)).collect(),
            analyzer,
        )
        .await
        .expect("scheduler loop must not fail");
    driver.abort();
    drop(executor);
    let collected = forwarder.await.expect("event forwarder must not panic");

    let mut run = LoopRun {
        report,
        analyses: analyses.load(Ordering::SeqCst),
        change_skipped: Vec::new(),
        dependency_blocked: Vec::new(),
        all_completed: false,
        stopped: false,
    };
    for event in collected {
        match event {
            ParallelEvent::AllCompleted => run.all_completed = true,
            ParallelEvent::Stopped => run.stopped = true,
            ParallelEvent::ChangeSkipped { change_id, .. } => run.change_skipped.push(change_id),
            ParallelEvent::DependencyBlocked {
                change_id,
                dependency_ids,
            } => {
                if dependency_ids.contains(&"a".to_string()) {
                    run.dependency_blocked.push(change_id);
                }
            }
            _ => {}
        }
    }
    run
}

#[tokio::test(start_paused = true)]
async fn failed_dependency_lifetime_finite_reports_blocked_rather_than_completed() {
    let repo = init_repo();

    let run = run_loop(
        repo.path(),
        SchedulerLifetime::Finite,
        &["b"],
        false,
        |_handles| async move {
            // A finite blocked-only scheduler must terminate on its own.
            std::future::pending::<()>().await;
        },
    )
    .await;

    assert_eq!(
        run.report,
        SchedulerRunReport::BlockedOrStalled,
        "blocked-only finite scheduling must not report completion"
    );
    assert!(
        !run.all_completed,
        "a run with blocked work remaining must not emit AllCompleted"
    );
    assert!(!run.stopped, "nothing cancelled this run");
    assert_eq!(
        run.analyses, 0,
        "blocked-only work must not invoke the analyzer"
    );
    assert_eq!(run.change_skipped, vec!["b".to_string()]);
    assert_eq!(run.dependency_blocked, vec!["b".to_string()]);
}

#[tokio::test(start_paused = true)]
async fn failed_dependency_loop_converges_across_repeated_notifications() {
    let repo = init_repo();

    let run = run_loop(
        repo.path(),
        SchedulerLifetime::Persistent,
        &["b"],
        false,
        |handles| async move {
            // Only start driving once the first blocked pass has actually been
            // observed, so cancellation can never pre-empt the behaviour the
            // assertions describe.
            handles.await_skips(1).await;

            // A persistent blocked-only scheduler waits for explicit
            // notifications; each sleep clears the 10s coalescing debounce, so
            // every wake below is debounce-eligible.
            for _ in 0..6 {
                tokio::time::sleep(DEBOUNCE_ELIGIBLE_WAKE).await;
                handles.queue.notify_scheduler();
            }
            tokio::time::sleep(DEBOUNCE_ELIGIBLE_WAKE).await;
            handles.cancel.cancel();
        },
    )
    .await;

    assert_eq!(run.report, SchedulerRunReport::Stopped);
    assert!(
        run.stopped,
        "the loop must still have been alive when the driver cancelled it"
    );
    assert!(!run.all_completed, "blocked work never completes");
    assert_eq!(
        run.analyses, 0,
        "unchanged wakes must not invoke the analyzer even once"
    );
    assert_eq!(
        run.change_skipped,
        vec!["b".to_string()],
        "repeated wakes must not repeat the compatibility observation"
    );
    assert_eq!(
        run.dependency_blocked,
        vec!["b".to_string()],
        "repeated wakes must not repeat the blocked transition"
    );
}

#[tokio::test(start_paused = true)]
async fn failed_dependency_lifetime_admits_genuine_dynamic_additions() {
    let repo = init_repo();

    let run = run_loop(
        repo.path(),
        SchedulerLifetime::Persistent,
        &["b"],
        true,
        |handles| async move {
            handles.await_skips(1).await;

            // A genuine dynamic addition: reducer intent first, then the hint.
            {
                let mut guard = handles.state.write().await;
                guard.apply_command(ReducerCommand::AddToQueue("c".to_string()));
            }
            handles.queue.push("c".to_string()).await;

            // The addition must reach analysis before the run is cancelled.
            handles.await_analyses(1).await;
            handles.cancel.cancel();
        },
    )
    .await;

    assert!(run.stopped);
    assert_eq!(
        run.analyses, 1,
        "a genuine queue addition must still receive its one immediate analysis"
    );
    assert_eq!(
        run.change_skipped,
        vec!["b".to_string()],
        "the still-blocked dependent must not be re-announced by unrelated new work"
    );
    assert_eq!(run.dependency_blocked, vec!["b".to_string()]);
}
