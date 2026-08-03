//! Scope regressions for background base-lane failures.
//!
//! A real run once exhausted bounded post-archive conflict resolution for one
//! change, correctly returned it to `merge wait` — and then emitted the same
//! failure as a process-scoped `ParallelEvent::Error` from both the merge layer
//! and its queue wrapper. The TUI, which treats a global Error as fatal,
//! retained `AppMode::Error` for hours while the persistent scheduler was still
//! alive.
//!
//! These tests pin the end-to-end contract that replaced it: an exhaustive typed
//! outcome at the background boundary, one authoritative change-scoped owner per
//! change-local failure, a single global-Error owner bound to an actual run
//! abort, and truthful finite completion.
//!
//! Everything here uses in-memory channels and constructed outcomes, so it stays
//! well under the default-suite one-second budget. The two scheduler-loop tests
//! drive the real loop with a merge-result channel double, so no detached merge
//! task or worktree is created.

use super::super::merge::BaseLaneFailure;
use crate::analyzer::{AnalysisOutcome, AnalysisProvenance, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::parallel::{
    AlreadyReportedFailureKind, MergeResult, MergeResultDisposition, MergeResultOrigin,
    MergeTaskOutcome, ParallelEvent, ParallelExecutor, ResolveFailureClassification,
    SchedulerLifetime,
};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Bounded must-arrive wait. Only reached when an assertion has already failed.
const EVENT_WAIT: Duration = Duration::from_secs(5);

/// Window in which the persistent scheduler must NOT return.
const STAY_ALIVE_WINDOW: Duration = Duration::from_millis(150);

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

fn init_minimal_git_repo(repo_root: &std::path::Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test User"],
    ] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git setup command");
        assert!(output.status.success(), "git setup command failed");
    }
    std::fs::write(repo_root.join("README.md"), "base\n").expect("write base file");
    for args in [vec!["add", "-A"], vec!["commit", "-m", "Base"]] {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .expect("run git commit command");
        assert!(output.status.success(), "git commit command failed");
    }
}

/// Analyzer double that dispatches nothing and counts how many times the
/// scheduler asked it to analyze.
///
/// The count is the dispatch-admission probe: the loop only reaches the analyzer
/// through `evaluate_queued_reanalysis_and_dispatch`, which sits *after* the
/// abort check at the top of the loop. A second invocation therefore means new
/// work was still being admitted after a fatal classification.
fn counting_idle_analyzer(
    calls: Arc<AtomicUsize>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |_changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        calls.fetch_add(1, Ordering::SeqCst);
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

/// Analyzer double that orders nothing on its first call and `dispatchable` on
/// every call after it.
///
/// The staging is the trap: the run-fatal result is the next thing the loop sees
/// after the first (empty) analysis, so a second analysis can only happen if the
/// loop kept admitting work — and that second analysis would dispatch real work,
/// which is exactly what the abort must prevent.
fn trapping_analyzer(
    calls: Arc<AtomicUsize>,
    dispatchable: &str,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    let dispatchable = dispatchable.to_string();
    move |_changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        let previous = calls.fetch_add(1, Ordering::SeqCst);
        let order = if previous == 0 {
            Vec::new()
        } else {
            vec![dispatchable.clone()]
        };
        Box::pin(async move {
            AnalysisOutcome::new(
                AnalysisResult {
                    order,
                    dependencies: std::collections::HashMap::new(),
                    groups: None,
                },
                AnalysisProvenance::HealthyLlm,
            )
        })
    }
}

/// Executor wired to a merge-result channel double with one outstanding result.
///
/// Returns the executor plus the event receiver, so a test can drive the real
/// scheduler loop against a constructed background outcome without spawning a
/// detached merge task.
fn executor_with_pending_outcome(
    repo_root: &std::path::Path,
    workspace_base: &std::path::Path,
    outcome: MergeTaskOutcome,
    change_id: &str,
) -> (ParallelExecutor, mpsc::Receiver<ParallelEvent>) {
    let (event_tx, events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_root.to_path_buf(),
        test_config(workspace_base),
        Some(event_tx),
    );

    let (merge_result_tx, merge_result_rx) = mpsc::channel(8);
    merge_result_tx
        .try_send(MergeResult {
            change_id: change_id.to_string(),
            workspace_name: format!("ws-{change_id}"),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome,
        })
        .expect("merge-result double must accept the pre-loaded outcome");
    executor.merge_result_channel_override = Some((merge_result_tx, merge_result_rx));
    executor.pending_merge_count.store(1, Ordering::SeqCst);

    (executor, events)
}

// ---------------------------------------------------------------------------
// Producer classification: the error-classification table
// ---------------------------------------------------------------------------

#[test]
fn exhausted_resolve_failure_keeps_change_scope_and_bounded_diagnosis() {
    let failure = BaseLaneFailure::ResolveExhausted {
        attempts: 3,
        classification: ResolveFailureClassification::UnresolvedConflict,
        detail: "conflicts remain in a.rs".to_string(),
    };

    match failure.into_outcome("alpha") {
        MergeTaskOutcome::ResolveExhausted {
            change_id,
            attempts,
            classification,
            detail,
        } => {
            assert_eq!(change_id, "alpha");
            assert_eq!(attempts, 3);
            assert_eq!(
                classification,
                ResolveFailureClassification::UnresolvedConflict
            );
            assert_eq!(detail, "conflicts remain in a.rs");
        }
        other => panic!("bounded exhaustion must stay change-local, got {other:?}"),
    }
}

#[test]
fn already_reported_failures_keep_their_typed_owner() {
    for kind in [
        AlreadyReportedFailureKind::Push,
        AlreadyReportedFailureKind::Hook,
        AlreadyReportedFailureKind::RejectionReview,
    ] {
        let outcome = BaseLaneFailure::AlreadyReported {
            kind,
            detail: "already reported".to_string(),
        }
        .into_outcome("alpha");

        match outcome {
            MergeTaskOutcome::RecoverableAlreadyReported {
                change_id,
                kind: reported,
                ..
            } => {
                assert_eq!(change_id, "alpha");
                assert_eq!(reported, kind);
            }
            other => panic!("{kind:?} must not fall through to run-fatal, got {other:?}"),
        }
    }
}

/// Base identity loss, pre-transition repository failure, uncertain post-merge
/// verification, and unknown invariant failure all fail closed.
///
/// The conversion — not a message match — is what does it: every unclassified
/// error that reaches this boundary through `?` becomes run-fatal, so the
/// default for anything not explicitly proven change-local is to abort.
#[test]
fn unclassified_errors_fail_closed_to_run_fatal() {
    use crate::error::OrchestratorError;
    use crate::vcs::{VcsBackend, VcsError};

    let unclassified: Vec<BaseLaneFailure> = vec![
        // Base branch cannot be identified (detached HEAD, no safe base).
        OrchestratorError::GitCommand("HEAD is detached; no safe base identity".to_string()).into(),
        // Conflict detection / repository query failed before any change-scoped
        // transition could be established.
        VcsError::Command {
            backend: VcsBackend::Git,
            message: "conflict query failed".to_string(),
            command: Some("git status".to_string()),
            working_dir: None,
            stderr: Some("not a work tree".to_string()),
            stdout: None,
        }
        .into(),
        // Post-merge verification left base integration truth unknown.
        BaseLaneFailure::fatal("Post-merge verification left base integration truth unknown: x"),
        // Unknown internal invariant failure.
        BaseLaneFailure::fatal("Merge failed: exhausted all attempts without success or error"),
    ];

    for failure in unclassified {
        let outcome = failure.clone().into_outcome("alpha");
        assert!(
            matches!(outcome, MergeTaskOutcome::RunFatal { .. }),
            "{failure:?} must fail closed, got {outcome:?}"
        );
        assert_eq!(
            outcome.disposition(),
            MergeResultDisposition::AbortRun,
            "{failure:?} must abort the run"
        );
        assert_eq!(
            outcome.scoped_change_id(),
            None,
            "a run-fatal outcome must not claim change scope it cannot prove"
        );
    }
}

/// A bounded exhaustion crossing the resolve/merge boundary keeps its
/// classification; anything else the resolve layer reports fails closed.
#[test]
fn resolve_failures_convert_by_type_not_by_message() {
    use super::super::conflict::ResolveFailure;
    use crate::error::OrchestratorError;

    let exhausted: BaseLaneFailure = ResolveFailure::Exhausted {
        attempts: 2,
        classification: ResolveFailureClassification::ResolveAgentFailed,
        detail: "agent exited 1".to_string(),
    }
    .into();
    assert!(matches!(
        exhausted,
        BaseLaneFailure::ResolveExhausted {
            attempts: 2,
            classification: ResolveFailureClassification::ResolveAgentFailed,
            ..
        }
    ));

    // The message deliberately quotes change-local wording; classification must
    // still come from the type.
    let unclassified: BaseLaneFailure =
        ResolveFailure::Unclassified(OrchestratorError::GitCommand(
            "resolve exhausted after 3 attempt(s) [unresolved_conflict]: quoted".to_string(),
        ))
        .into();
    assert!(
        matches!(unclassified, BaseLaneFailure::RunFatal { .. }),
        "scope must never be inferred from diagnostic text, got {unclassified:?}"
    );
}

// ---------------------------------------------------------------------------
// Event ownership: exactly one authoritative change-scoped transition
// ---------------------------------------------------------------------------

/// The producer-side event sequence for a bounded exhaustion.
///
/// One change-scoped `ResolveFailed` per change, optional non-state
/// presentation telemetry, and zero global Errors.
#[tokio::test]
async fn bounded_exhaustion_emits_one_resolve_failed_and_no_global_error() {
    let (event_tx, mut events) = mpsc::channel(32);

    let failure = super::super::conflict::fail_resolve(
        &Some(event_tx),
        &["alpha".to_string()],
        3,
        ResolveFailureClassification::UnresolvedConflict,
        "conflicts still present after merge resolution attempt: a.rs".to_string(),
    )
    .await;

    let mut resolve_failed = Vec::new();
    let mut telemetry = Vec::new();
    let mut global_errors = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            ExecutionEvent::ResolveFailed { change_id, error } => {
                resolve_failed.push((change_id, error))
            }
            ExecutionEvent::ConflictResolutionFailed { error } => telemetry.push(error),
            ExecutionEvent::Error { message } => global_errors.push(message),
            _ => {}
        }
    }

    assert_eq!(
        resolve_failed.len(),
        1,
        "exactly one authoritative change-scoped transition"
    );
    assert_eq!(resolve_failed[0].0, "alpha");
    assert!(
        global_errors.is_empty(),
        "bounded exhaustion must emit no global Error, got {global_errors:?}"
    );
    assert_eq!(
        telemetry.len(),
        1,
        "presentation telemetry rides along exactly once"
    );

    // Diagnostic contract: attempts, bounded classification token, and a
    // sanitized summary — never raw unbounded agent output.
    let detail = &resolve_failed[0].1;
    assert!(detail.contains("3 attempt(s)"), "{detail}");
    assert!(detail.contains("unresolved_conflict"), "{detail}");
    assert!(detail.contains("a.rs"), "{detail}");
    assert!(matches!(
        failure,
        super::super::conflict::ResolveFailure::Exhausted { attempts: 3, .. }
    ));
}

/// `ConflictResolutionFailed` stays presentation-only in the shared classifier
/// both frontends read, so it can never become a workflow-state owner.
#[test]
fn conflict_resolution_failed_is_presentation_only() {
    use crate::events::{event_ownership, EventOwnership};

    assert_eq!(
        event_ownership(&ExecutionEvent::ConflictResolutionFailed {
            error: "resolve exhausted after 3 attempt(s)".to_string(),
        }),
        EventOwnership::Presentation,
        "telemetry must never advance reducer or snapshot state"
    );
    assert_eq!(
        event_ownership(&ExecutionEvent::ResolveFailed {
            change_id: "alpha".to_string(),
            error: "resolve exhausted after 3 attempt(s)".to_string(),
        }),
        EventOwnership::State,
        "ResolveFailed is the one workflow-state owner"
    );
}

// ---------------------------------------------------------------------------
// Scheduler lifetime: persistent continuation, finite truthful completion,
// fatal abort
// ---------------------------------------------------------------------------

/// After a change-local failure, the scheduler's own eligibility classification
/// still admits unrelated work and still blocks the failed change's dependents.
///
/// `alpha` holds manual `MergeWait`, `beta` depends on nothing, and `gamma`
/// depends on `alpha`. Continuing the run must not turn into continuing *past*
/// the dependency the failure invalidated.
#[tokio::test]
async fn resolve_exhaustion_keeps_unrelated_work_eligible_and_dependents_blocked() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (event_tx, _events) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);

    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);
    executor.pending_merge_count.store(1, Ordering::SeqCst);
    executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "alpha".to_string(),
                workspace_name: "ws-alpha".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: MergeTaskOutcome::resolve_exhausted(
                    "alpha",
                    3,
                    ResolveFailureClassification::UnresolvedConflict,
                    "conflicts remain",
                ),
            },
            &merge_result_tx,
        )
        .await;
    // The reducer transition the conflict layer's `ResolveFailed` produces.
    executor.merge_wait_changes.insert("alpha".to_string());

    let mut gamma = test_change("gamma");
    gamma.dependencies = vec!["alpha".to_string()];
    let queued = vec![test_change("alpha"), test_change("beta"), gamma];

    let classification = executor
        .classify_queued_work(&queued, &std::collections::HashSet::new())
        .await;

    assert!(
        classification
            .dispatchable
            .iter()
            .any(|change| change.id == "beta"),
        "an unrelated non-dependent change stays dispatchable: {classification:?}"
    );
    assert!(
        classification
            .dependency_blocked
            .contains(&"gamma".to_string()),
        "a dependent of the failed change stays blocked: {classification:?}"
    );
    assert!(
        classification
            .manual_merge_wait
            .contains(&"alpha".to_string()),
        "the failed change itself waits for explicit retry: {classification:?}"
    );
}

/// A persistent scheduler survives a change-local failure.
///
/// It records the failure, emits no global Error, and stays available for
/// dynamic queue notifications rather than terminating the run.
#[tokio::test]
async fn persistent_scheduler_continues_after_resolve_exhaustion() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (mut executor, mut events) = executor_with_pending_outcome(
        repo_dir.path(),
        workspace_base.path(),
        MergeTaskOutcome::resolve_exhausted(
            "alpha",
            3,
            ResolveFailureClassification::UnresolvedConflict,
            "conflicts remain",
        ),
        "alpha",
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);
    let cancel_token = CancellationToken::new();
    executor.set_cancel_token(cancel_token.clone());
    let queue = Arc::new(crate::tui::queue::DynamicQueue::new());
    executor.set_dynamic_queue(queue.clone());

    let analysis_calls = Arc::new(AtomicUsize::new(0));
    let analyzer = counting_idle_analyzer(analysis_calls.clone());
    let mut scheduler = tokio::spawn(async move {
        let report = executor
            .execute_with_order_based_reanalysis(vec![test_change("alpha")], analyzer)
            .await;
        (report, ())
    });

    // The scheduler must still be running after the change-local failure.
    assert!(
        tokio::time::timeout(STAY_ALIVE_WINDOW, &mut scheduler)
            .await
            .is_err(),
        "a persistent scheduler must survive a change-local base-lane failure"
    );

    let mut global_errors = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let ParallelEvent::Error { message } = event {
            global_errors.push(message);
        }
    }
    assert!(
        global_errors.is_empty(),
        "an exhausted resolve must not invalidate a live run, got {global_errors:?}"
    );

    cancel_token.cancel();
    let (report, ()) = tokio::time::timeout(EVENT_WAIT, scheduler)
        .await
        .expect("cancelled persistent scheduler must return")
        .expect("scheduler task must not panic");
    assert_eq!(
        report.expect("cancellation is not a scheduler failure"),
        crate::parallel::SchedulerRunReport::Stopped
    );
}

/// A finite run that drained with an unresolved change-local failure reports
/// completed-with-errors: warning plus the existing terminal event, no success
/// message, and no global Error.
///
/// The failure is recorded through the real queue boundary and the report is
/// produced by the real completion path; the drain itself is already covered by
/// the scheduler-loop regressions in `idle_parallel_stop`.
#[tokio::test]
async fn finite_scheduler_completes_with_errors_after_resolve_exhaustion() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (event_tx, mut events) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        test_config(workspace_base.path()),
        Some(event_tx),
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Finite);

    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);
    executor.pending_merge_count.store(1, Ordering::SeqCst);
    let disposition = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "alpha".to_string(),
                workspace_name: "ws-alpha".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: MergeTaskOutcome::resolve_exhausted(
                    "alpha",
                    3,
                    ResolveFailureClassification::UnresolvedConflict,
                    "conflicts remain",
                ),
            },
            &merge_result_tx,
        )
        .await;
    assert_eq!(disposition, MergeResultDisposition::ContinueWithErrors);

    let analysis_calls = Arc::new(AtomicUsize::new(0));
    let analyzer = counting_idle_analyzer(analysis_calls);
    let report = tokio::time::timeout(
        EVENT_WAIT,
        executor.execute_with_order_based_reanalysis(Vec::new(), analyzer),
    )
    .await
    .expect("a finite run with only a failed change must still drain")
    .expect("completed-with-errors is not a scheduler failure");

    assert_eq!(
        report,
        crate::parallel::SchedulerRunReport::CompletedWithErrors,
        "manual MergeWait does not block termination, but it does forbid claiming success"
    );

    let mut warnings = Vec::new();
    let mut saw_all_completed = false;
    let mut global_errors = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            ParallelEvent::Log(entry) => warnings.push(entry.message),
            ParallelEvent::AllCompleted => saw_all_completed = true,
            ParallelEvent::Error { message } => global_errors.push(message),
            _ => {}
        }
    }

    assert!(
        saw_all_completed,
        "the existing terminal event is still emitted"
    );
    assert!(
        global_errors.is_empty(),
        "a completed-with-errors run emits no global Error, got {global_errors:?}"
    );
    assert!(
        warnings.iter().any(
            |message| message.contains("Processing completed with errors")
                && message.contains("alpha")
        ),
        "the operator must be told which change is still owed, got {warnings:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|message| message.contains("All parallel changes completed")),
        "no success message may accompany an unresolved change-local failure"
    );
}

/// A run-fatal outcome actually aborts: one global Error, no further dispatch
/// admission, and a failed scheduler future.
#[tokio::test]
async fn run_fatal_outcome_stops_dispatch_and_fails_the_scheduler() {
    let repo_dir = TempDir::new().expect("repo tempdir");
    init_minimal_git_repo(repo_dir.path());
    let workspace_base = TempDir::new().expect("workspace base");

    let (mut executor, mut events) = executor_with_pending_outcome(
        repo_dir.path(),
        workspace_base.path(),
        MergeTaskOutcome::run_fatal("base branch could not be identified"),
        "alpha",
    );
    executor.set_scheduler_lifetime(SchedulerLifetime::Persistent);

    // The dispatch-admission trap: the first analysis orders nothing, so the
    // fatal result is the next thing the loop sees. Every later analysis would
    // order `beta`, so a loop that kept admitting work would dispatch it and
    // create a workspace.
    let analysis_calls = Arc::new(AtomicUsize::new(0));
    let analyzer = trapping_analyzer(analysis_calls.clone(), "beta");
    let result = tokio::time::timeout(
        EVENT_WAIT,
        executor.execute_with_order_based_reanalysis(vec![test_change("beta")], analyzer),
    )
    .await
    .expect("a run-fatal outcome must terminate the scheduler promptly");

    assert!(
        result.is_err(),
        "AbortRun must terminate the scheduler future as failure, got {result:?}"
    );
    assert_eq!(
        analysis_calls.load(Ordering::SeqCst),
        1,
        "no unrelated change may be admitted after fatal classification"
    );

    let mut global_errors = Vec::new();
    let mut terminal_events = Vec::new();
    let mut workspaces_created = 0usize;
    while let Ok(event) = events.try_recv() {
        match event {
            ParallelEvent::Error { message } => global_errors.push(message),
            ParallelEvent::AllCompleted => terminal_events.push("AllCompleted"),
            ParallelEvent::Stopped => terminal_events.push("Stopped"),
            ParallelEvent::WorkspaceCreated { .. } => workspaces_created += 1,
            _ => {}
        }
    }

    assert_eq!(
        global_errors.len(),
        1,
        "exactly one global Error owner, got {global_errors:?}"
    );
    assert!(
        global_errors[0].contains("base branch could not be identified"),
        "{:?}",
        global_errors[0]
    );
    assert!(
        terminal_events.is_empty(),
        "an aborted run neither completes nor reports an operator stop, got {terminal_events:?}"
    );
    assert_eq!(
        workspaces_created, 0,
        "no new workspace may start after fatal classification"
    );
}

// ---------------------------------------------------------------------------
// Frontend projection: change scope survives every adapter
// ---------------------------------------------------------------------------

/// The reducer applies the real exhaustion sequence as change-scoped state.
///
/// `ResolveFailed` returns `alpha` to `MergeWait` with its worktree evidence
/// intact, and the presentation telemetry that rides along changes nothing.
#[test]
fn reducer_keeps_exhausted_change_in_merge_wait() {
    use crate::orchestration::state::{ExecutionMode, OrchestratorState, WorkspaceObservation};

    let mut state = OrchestratorState::with_mode(
        vec!["alpha".to_string(), "beta".to_string()],
        3,
        ExecutionMode::Parallel,
    );
    state.apply_observation("alpha", WorkspaceObservation::WorkspaceArchived);

    let detail = crate::parallel::resolve_failure_detail(
        3,
        ResolveFailureClassification::UnresolvedConflict,
        "conflicts remain",
    );
    state.apply_execution_event(&ExecutionEvent::ConflictResolutionFailed {
        error: detail.clone(),
    });
    state.apply_execution_event(&ExecutionEvent::ResolveFailed {
        change_id: "alpha".to_string(),
        error: detail,
    });

    assert!(
        state.merge_wait_change_ids().contains(&"alpha".to_string()),
        "the exhausted change must remain available for explicit retry: {:?}",
        state.merge_wait_change_ids()
    );
    assert!(state.global_invariants_hold());
}

/// Ordered remote projection keeps the change ID and never sets `process_error`.
#[test]
fn web_projection_keeps_exhausted_resolve_change_scoped() {
    use crate::web::operator_facts::OperatorFactsStore;

    let detail = crate::parallel::resolve_failure_detail(
        3,
        ResolveFailureClassification::UnresolvedConflict,
        "conflicts remain",
    );

    let mut facts = OperatorFactsStore::new();
    facts.record_event(&ExecutionEvent::ConflictResolutionFailed {
        error: detail.clone(),
    });
    facts.record_event(&ExecutionEvent::ResolveFailed {
        change_id: "alpha".to_string(),
        error: detail.clone(),
    });
    assert_eq!(
        facts.process_error(),
        None,
        "a change-local failure must never become the process error"
    );

    let mut fatal_facts = OperatorFactsStore::new();
    fatal_facts.record_event(&ExecutionEvent::Error {
        message: "base branch could not be identified".to_string(),
    });
    assert!(
        fatal_facts.process_error().is_some(),
        "a run-fatal global Error stays process-fatal"
    );
}

/// External lifecycle projection stays non-process-fatal for an exhausted
/// resolve and process-fatal for a run-fatal abort.
#[test]
fn external_lifecycle_projection_preserves_failure_scope() {
    use crate::lifecycle_integration::{LifecycleEvent, LifecycleState};

    let detail = crate::parallel::resolve_failure_detail(
        3,
        ResolveFailureClassification::UnresolvedConflict,
        "conflicts remain",
    );

    for event in [
        ExecutionEvent::ResolveFailed {
            change_id: "alpha".to_string(),
            error: detail.clone(),
        },
        ExecutionEvent::ConflictResolutionFailed { error: detail },
    ] {
        assert!(
            crate::events::lifecycle_event_for_execution_event(&event, None).is_none(),
            "{event:?} must not project a process-scoped lifecycle state"
        );
    }

    let fatal = crate::events::lifecycle_event_for_execution_event(
        &ExecutionEvent::Error {
            message: "base branch could not be identified".to_string(),
        },
        None,
    )
    .expect("a run-fatal global Error must reach external lifecycle consumers");
    match fatal {
        LifecycleEvent::StateChanged { state, context } => {
            assert_eq!(state, LifecycleState::Blocked);
            assert_eq!(context.change_id, None, "a fatal run is not change-scoped");
        }
        other => panic!("expected a lifecycle state change, got {other:?}"),
    }
}
