//! Unit tests for the shared run-lifecycle service.
//!
//! Every test drives the service over in-memory doubles: the reducer is a plain
//! `OrchestratorState`, the scheduler is [`RecordingScheduler`], and the queue is
//! an in-memory fake. No process, repository, network, or timer is involved.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::testing::{RecordingScheduler, SchedulerCall};
use super::*;
use crate::events::{ExecutionEvent, StalledBlocker};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, QueuePort, TerminationWaiter,
};
use crate::orchestration::state::ExecutionMode;

/// In-memory [`QueuePort`] with no runtime behind it.
#[derive(Debug, Default)]
struct FakeQueue {
    entries: Mutex<Vec<String>>,
    notifications: Mutex<usize>,
}

#[async_trait]
impl QueuePort for FakeQueue {
    async fn add(&self, change_id: &str) -> bool {
        let mut guard = self.entries.lock().unwrap();
        if guard.iter().any(|id| id == change_id) {
            return false;
        }
        guard.push(change_id.to_string());
        true
    }

    async fn remove(&self, change_id: &str) -> bool {
        let mut guard = self.entries.lock().unwrap();
        let before = guard.len();
        guard.retain(|id| id != change_id);
        guard.len() != before
    }

    async fn request_cancellation(
        &self,
        _change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        Ok(None)
    }

    async fn notify_scheduler(&self) {
        *self.notifications.lock().unwrap() += 1;
    }
}

/// A fully wired service over in-memory doubles.
struct Harness {
    service: RunControlService,
    state: Arc<RwLock<OrchestratorState>>,
    scheduler: Arc<RecordingScheduler>,
    marks: Arc<ExecutionMarkStore>,
    resolves: Arc<ResolveReservations>,
    eligibility: Arc<StartEligibility>,
}

impl Harness {
    fn new(change_ids: &[&str]) -> Self {
        let state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
            ExecutionMode::Parallel,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let operator = Arc::new(OperatorCommandService::new(
            state.clone(),
            Arc::new(FakeQueue::default()),
            Arc::new(NoopQueueHooks),
            marks.clone(),
        ));
        let scheduler = Arc::new(RecordingScheduler::new());
        let resolves = Arc::new(ResolveReservations::new());
        let eligibility = Arc::new(StartEligibility::new());
        Self {
            service: RunControlService::new(
                state.clone(),
                operator,
                scheduler.clone(),
                resolves.clone(),
                eligibility.clone(),
            ),
            state,
            scheduler,
            marks,
            resolves,
            eligibility,
        }
    }

    fn mark(&self, change_ids: &[&str]) {
        self.marks
            .replace(change_ids.iter().map(|id| id.to_string()));
    }

    async fn apply(&self, event: ExecutionEvent) {
        self.state.write().await.apply_execution_event(&event);
    }

    async fn status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    /// Move a change into a reducer-visible merge wait.
    async fn to_merge_wait(&self, change_id: &str) {
        self.apply(ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        })
        .await;
    }

    /// Move a change into a terminal error state.
    async fn to_error(&self, change_id: &str) {
        self.apply(ExecutionEvent::ProcessingError {
            id: change_id.to_string(),
            error: "boom".to_string(),
        })
        .await;
    }
}

fn external_blocker() -> StalledBlocker {
    StalledBlocker {
        category: "external_service".to_string(),
        phase: "acceptance".to_string(),
        gate: "prerequisite".to_string(),
        error_summary: "registry unreachable".to_string(),
        evidence: vec!["curl: (6) could not resolve host".to_string()],
        unblock_condition: Some("registry responds to a health check".to_string()),
        prerequisite_owner: Some("platform".to_string()),
        next_action: "retry once the registry is reachable".to_string(),
        resumable: true,
        worktree_preserved: true,
    }
}

// ============================================================================
// Start
// ============================================================================

#[tokio::test]
async fn start_consumes_the_authoritative_marked_target_set() {
    let harness = Harness::new(&["a", "b", "c"]);
    harness.mark(&["a", "c"]);

    let outcome = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("marked changes are startable");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["a".to_string(), "c".to_string()],
            explicit_retry: false,
            scheduler: SchedulerEffect::Started,
        }
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["a".to_string(), "c".to_string()]],
        "the run must be dispatched for exactly the marked target set"
    );
    assert_eq!(harness.status("a").await, "queued");
    assert_eq!(harness.status("b").await, "not queued");
}

#[tokio::test]
async fn start_without_any_mark_fails_and_starts_nothing() {
    let harness = Harness::new(&["a"]);

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("an empty target set is not a successful start");

    assert!(matches!(
        error,
        RunControlError::NoEligibleTarget {
            command: RunCommandKind::Start,
            ..
        }
    ));
    assert!(
        harness.scheduler.calls().is_empty(),
        "a failed start must not touch the scheduler"
    );
}

#[tokio::test]
async fn start_with_only_ineligible_marks_fails_and_starts_nothing() {
    let harness = Harness::new(&["a"]);
    harness.mark(&["a"]);
    harness.to_merge_wait("a").await;

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("a merge-wait row is not startable");

    match error {
        RunControlError::NoEligibleTarget { detail, .. } => {
            assert!(detail.contains("not queued"), "detail must be actionable");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(harness.scheduler.calls().is_empty());
}

#[tokio::test]
async fn start_refuses_parallel_ineligible_targets() {
    let harness = Harness::new(&["a"]);
    harness.mark(&["a"]);
    harness.eligibility.set_parallel_mode(true);
    harness
        .eligibility
        .set_parallel_ineligible(["a".to_string()]);

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("parallel mode refuses uncommitted changes");

    assert!(matches!(error, RunControlError::NoEligibleTarget { .. }));
    assert!(harness.scheduler.calls().is_empty());
}

/// Parallel start is all-or-nothing over the *complete* marked set.
///
/// The eligible remainder is a target set the operator never asked for, so one
/// ineligible mark must refuse the whole operation — and it must do so even
/// when the ineligible row is not itself startable, which is the case a fence
/// applied only to the startable subset would wave through.
#[tokio::test]
async fn one_ineligible_mark_refuses_the_whole_parallel_start() {
    for arrange_ineligible_as_unstartable in [false, true] {
        let harness = Harness::new(&["eligible", "ineligible"]);
        harness.mark(&["eligible", "ineligible"]);
        harness.eligibility.set_parallel_mode(true);
        harness
            .eligibility
            .set_parallel_ineligible(["ineligible".to_string()]);
        if arrange_ineligible_as_unstartable {
            harness.to_merge_wait("ineligible").await;
        }

        let error = harness
            .service
            .start(OperatorMode::Select)
            .await
            .expect_err("one ineligible marked target refuses the whole start");

        match &error {
            RunControlError::NoEligibleTarget { detail, .. } => assert!(
                detail.contains("ineligible") && !detail.contains("eligible,"),
                "the refusal must name the ineligible target: {detail}"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            harness.scheduler.calls().is_empty(),
            "no scheduler may be spawned for a partially eligible target set"
        );
        assert_eq!(
            harness.status("eligible").await,
            "not queued",
            "the eligible target must not be left queued by a refused start"
        );
        assert_eq!(
            harness.marks.marked_ids(),
            vec!["eligible".to_string(), "ineligible".to_string()],
            "a refused start leaves marks coherent"
        );
    }
}

#[tokio::test]
async fn start_wakes_a_live_scheduler_instead_of_spawning_a_second_run() {
    let harness = Harness::new(&["a"]);
    harness.mark(&["a"]);
    harness.scheduler.set_running(true);

    let outcome = harness.service.start(OperatorMode::Stopped).await.unwrap();

    assert!(matches!(
        outcome,
        RunControlOutcome::RunDispatched {
            scheduler: SchedulerEffect::Notified,
            ..
        }
    ));
    assert_eq!(harness.scheduler.calls(), vec![SchedulerCall::Notified]);
}

#[tokio::test]
async fn start_is_refused_while_a_run_owns_the_lifecycle() {
    for mode in [OperatorMode::Running, OperatorMode::Stopping] {
        let harness = Harness::new(&["a"]);
        harness.mark(&["a"]);

        let error = harness.service.start(mode).await.expect_err("mode refuses");

        assert!(matches!(
            error,
            RunControlError::InvalidMode {
                command: RunCommandKind::Start,
                ..
            }
        ));
        assert!(harness.scheduler.calls().is_empty());
    }
}

#[tokio::test]
async fn start_reports_a_runtime_launch_failure_instead_of_claiming_success() {
    let harness = Harness::new(&["a"]);
    harness.mark(&["a"]);
    harness.scheduler.fail_launch("serial mode cannot run");

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("a refused launch is not a successful start");

    assert!(matches!(
        error,
        RunControlError::DispatchFailed {
            command: RunCommandKind::Start,
            ..
        }
    ));
}

// ============================================================================
// Retry
// ============================================================================

#[tokio::test]
async fn retry_routes_a_terminal_error_and_dispatches_the_scheduler() {
    let harness = Harness::new(&["a"]);
    harness.to_error("a").await;

    let outcome = harness.service.retry_change("a").await.unwrap();

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["a".to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
        }
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["a".to_string()]]
    );
}

#[tokio::test]
async fn retry_resumes_a_resumable_external_hold() {
    let harness = Harness::new(&["a"]);
    harness
        .apply(ExecutionEvent::ExecutionBlocked {
            change_id: "a".to_string(),
            blocker: external_blocker(),
        })
        .await;
    assert_eq!(harness.status("a").await, "blocked");

    let outcome = harness.service.retry_change("a").await.unwrap();

    assert!(matches!(
        outcome,
        RunControlOutcome::RunDispatched {
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            ..
        }
    ));
}

#[tokio::test]
async fn retry_of_an_unsupported_target_changes_nothing() {
    let harness = Harness::new(&["a"]);

    let error = harness
        .service
        .retry_change("a")
        .await
        .expect_err("a not-queued row carries no retryable evidence");

    assert!(matches!(error, RunControlError::Operator(_)));
    assert!(
        harness.scheduler.calls().is_empty(),
        "an unsupported retry must not dispatch work"
    );
}

#[tokio::test]
async fn bulk_retry_without_retryable_evidence_is_a_no_op() {
    let harness = Harness::new(&["a", "b"]);

    let outcome = harness
        .service
        .retry_errors(&["a".to_string(), "b".to_string()])
        .await
        .unwrap();

    assert_eq!(
        outcome,
        RunControlOutcome::NoOp {
            reason: RunNoOpReason::NoRetryableTarget
        }
    );
    assert!(harness.scheduler.calls().is_empty());
}

#[tokio::test]
async fn start_in_error_mode_retries_the_marked_error_rows() {
    let harness = Harness::new(&["a", "b"]);
    harness.to_error("a").await;
    harness.mark(&["a", "b"]);

    let outcome = harness.service.start(OperatorMode::Error).await.unwrap();

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["a".to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
        },
        "only the row that carries retryable evidence is retried"
    );
}

// ============================================================================
// Stop family
// ============================================================================

#[tokio::test]
async fn stop_and_cancel_stop_enforce_the_mode_matrix() {
    let harness = Harness::new(&["a"]);

    assert_eq!(
        harness.service.stop(OperatorMode::Running).await.unwrap(),
        RunControlOutcome::StopRequested
    );
    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::GracefulStop(true)]
    );

    for refused in [
        OperatorMode::Select,
        OperatorMode::Stopped,
        OperatorMode::Stopping,
        OperatorMode::Error,
    ] {
        let error = harness.service.stop(refused).await.expect_err("refused");
        assert!(matches!(
            error,
            RunControlError::InvalidMode {
                command: RunCommandKind::Stop,
                ..
            }
        ));
    }

    assert_eq!(
        harness
            .service
            .cancel_stop(OperatorMode::Stopping)
            .await
            .unwrap(),
        RunControlOutcome::StopCancelled
    );
    let error = harness
        .service
        .cancel_stop(OperatorMode::Running)
        .await
        .expect_err("cancel stop needs a pending stop");
    assert!(matches!(
        error,
        RunControlError::InvalidMode {
            command: RunCommandKind::CancelStop,
            ..
        }
    ));
}

#[tokio::test]
async fn force_stop_reports_classification_truthfully_and_always_cancels() {
    use crate::tui::stop_classification::{ExecutionEvidence, ProcessReport, ShutdownWorkEvidence};

    // Active execution: force-stop reporting plus a scheduler-owned boundary.
    let harness = Harness::new(&["a"]);
    harness.scheduler.set_running(true);
    harness.scheduler.set_activity(StopActivitySnapshot {
        execution_handles: ExecutionEvidence::Known { registered: 1 },
        reducer_agent_execution_active: false,
        shutdown_work: ShutdownWorkEvidence::Known { pending: false },
    });

    let outcome = harness
        .service
        .force_stop(OperatorMode::Running)
        .await
        .unwrap();
    match outcome {
        RunControlOutcome::ForceStopped {
            classification,
            awaiting_safe_boundary,
        } => {
            assert_eq!(classification.process_report, ProcessReport::ForceStopped);
            assert!(awaiting_safe_boundary);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(harness
        .scheduler
        .calls()
        .contains(&SchedulerCall::Cancelled));

    // Idle scheduler: ordinary stop, no boundary wait, cancellation still issued.
    let idle = Harness::new(&["a"]);
    let outcome = idle
        .service
        .force_stop(OperatorMode::Stopping)
        .await
        .unwrap();
    match outcome {
        RunControlOutcome::ForceStopped {
            classification,
            awaiting_safe_boundary,
        } => {
            assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
            assert!(!awaiting_safe_boundary);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(idle.scheduler.calls().contains(&SchedulerCall::Cancelled));
}

#[tokio::test]
async fn force_stop_is_refused_outside_running_and_stopping() {
    let harness = Harness::new(&["a"]);
    for refused in [
        OperatorMode::Select,
        OperatorMode::Stopped,
        OperatorMode::Error,
    ] {
        let error = harness
            .service
            .force_stop(refused)
            .await
            .expect_err("refused");
        assert!(matches!(
            error,
            RunControlError::InvalidMode {
                command: RunCommandKind::ForceStop,
                ..
            }
        ));
    }
    assert!(
        harness.scheduler.calls().is_empty(),
        "a refused force stop must not cancel the run"
    );
}

// ============================================================================
// Resolve
// ============================================================================

#[tokio::test]
async fn resolve_reserves_one_active_resolver_and_queues_the_rest_in_fifo_order() {
    let harness = Harness::new(&["a", "b", "c"]);
    for id in ["a", "b", "c"] {
        harness.to_merge_wait(id).await;
    }

    let first = harness.service.resolve_merge("a").await.unwrap();
    assert_eq!(
        first,
        RunControlOutcome::ResolveReserved {
            change_id: "a".to_string(),
            reservation: ResolveReservation::Active,
            scheduler: SchedulerEffect::Started,
        }
    );

    let second = harness.service.resolve_merge("b").await.unwrap();
    let third = harness.service.resolve_merge("c").await.unwrap();
    assert_eq!(
        second,
        RunControlOutcome::ResolveReserved {
            change_id: "b".to_string(),
            reservation: ResolveReservation::Queued { position: 1 },
            scheduler: SchedulerEffect::None,
        }
    );
    assert_eq!(
        third,
        RunControlOutcome::ResolveReserved {
            change_id: "c".to_string(),
            reservation: ResolveReservation::Queued { position: 2 },
            scheduler: SchedulerEffect::None,
        }
    );
    assert_eq!(
        harness.resolves.waiting(),
        vec!["b".to_string(), "c".to_string()]
    );
    assert_eq!(
        harness.scheduler.started_targets().len(),
        1,
        "only the active resolver dispatches scheduler work"
    );
}

#[tokio::test]
async fn duplicate_resolve_submission_does_not_create_a_second_queue_entry() {
    let harness = Harness::new(&["a", "b"]);
    harness.to_merge_wait("a").await;
    harness.to_merge_wait("b").await;
    harness.service.resolve_merge("a").await.unwrap();
    harness.service.resolve_merge("b").await.unwrap();

    let duplicate = harness.service.resolve_merge("b").await.unwrap();

    assert_eq!(
        duplicate,
        RunControlOutcome::NoOp {
            reason: RunNoOpReason::ResolveAlreadyReserved {
                change_id: "b".to_string()
            }
        }
    );
    assert_eq!(harness.resolves.waiting(), vec!["b".to_string()]);
}

#[tokio::test]
async fn resolve_of_a_stale_target_is_refused_without_a_reservation() {
    let harness = Harness::new(&["a"]);
    harness
        .apply(ExecutionEvent::MergeCompleted {
            change_id: "a".to_string(),
            revision: "rev-a".to_string(),
        })
        .await;

    let error = harness
        .service
        .resolve_merge("a")
        .await
        .expect_err("a merged change is not waiting on a merge");

    assert!(matches!(
        error,
        RunControlError::TargetIneligible {
            command: RunCommandKind::Resolve,
            ..
        }
    ));
    assert!(!harness.resolves.is_active());
    assert!(harness.scheduler.calls().is_empty());
}

#[tokio::test]
async fn resolve_wakes_a_live_scheduler_instead_of_starting_a_second_one() {
    let harness = Harness::new(&["a"]);
    harness.to_merge_wait("a").await;
    harness.scheduler.set_running(true);

    let outcome = harness.service.resolve_merge("a").await.unwrap();

    assert_eq!(
        outcome,
        RunControlOutcome::ResolveReserved {
            change_id: "a".to_string(),
            reservation: ResolveReservation::Active,
            scheduler: SchedulerEffect::Notified,
        }
    );
    assert_eq!(harness.scheduler.calls(), vec![SchedulerCall::Notified]);
}

#[tokio::test]
async fn finishing_the_active_resolve_promotes_the_next_waiting_change() {
    let harness = Harness::new(&["a", "b"]);
    harness.to_merge_wait("a").await;
    harness.to_merge_wait("b").await;
    harness.service.resolve_merge("a").await.unwrap();
    harness.service.resolve_merge("b").await.unwrap();

    assert_eq!(harness.resolves.finish_active(), Some("b".to_string()));
    assert!(!harness.resolves.is_active());
    assert!(
        !harness.resolves.is_reserved("b"),
        "a promoted change releases its reservation so it can reserve again"
    );
}

// ============================================================================
// Reservation ledger
// ============================================================================

#[test]
fn cancelling_a_queued_reservation_preserves_fifo_order() {
    let ledger = ResolveReservations::new();
    assert_eq!(ledger.reserve("a"), Some(ResolveReservation::Active));
    assert_eq!(
        ledger.reserve("b"),
        Some(ResolveReservation::Queued { position: 1 })
    );
    assert_eq!(
        ledger.reserve("c"),
        Some(ResolveReservation::Queued { position: 2 })
    );

    assert!(ledger.cancel("b"));
    assert!(!ledger.cancel("b"), "cancelling twice reports no change");
    assert_eq!(ledger.waiting(), vec!["c".to_string()]);
    assert_eq!(ledger.finish_active(), Some("c".to_string()));
}

#[test]
fn marking_an_active_resolver_removes_it_from_the_waiting_queue() {
    let ledger = ResolveReservations::new();
    ledger.reserve("a");
    ledger.reserve("b");

    // A scheduler-owned resolve can start for a change that was queued.
    ledger.mark_active("b");

    assert_eq!(ledger.active(), Some("b".to_string()));
    assert!(ledger.waiting().is_empty());
}

#[test]
fn start_eligibility_only_rejects_in_parallel_mode() {
    let eligibility = StartEligibility::new();
    eligibility.set_parallel_ineligible(["a".to_string()]);
    let targets = vec!["a".to_string(), "b".to_string()];

    assert!(
        eligibility.rejected(&targets).is_empty(),
        "serial mode has no parallel-eligibility constraint"
    );

    eligibility.set_parallel_mode(true);
    assert_eq!(eligibility.rejected(&targets), vec!["a".to_string()]);
}

/// The authoritative start set is the intersection of the mark store and the
/// reducer's own "not queued" status, so a marked row that is already in flight —
/// waiting on a merge, or already queued — is never started a second time.
#[tokio::test]
async fn start_targets_only_returns_marked_rows_the_reducer_calls_not_queued() {
    let harness = Harness::new(&["a", "b", "c"]);
    harness.mark(&["a", "b", "c"]);
    harness.to_merge_wait("b").await;
    harness
        .service
        .operator()
        .add_to_queue("c")
        .await
        .expect("queueing a change is allowed");

    assert_eq!(harness.status("b").await, "merge wait");
    assert_eq!(harness.status("c").await, "queued");
    assert_eq!(harness.service.start_targets().await, vec!["a".to_string()]);
}
