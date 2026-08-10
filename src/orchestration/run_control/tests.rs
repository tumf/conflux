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
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, ParallelEligibility, QueuePort,
    TerminationWaiter,
};

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
    queue: Arc<FakeQueue>,
    operator: Arc<OperatorCommandService>,
    resolves: Arc<ResolveReservations>,
    eligibility: Arc<StartEligibility>,
}

impl Harness {
    fn new(change_ids: &[&str]) -> Self {
        let state = Arc::new(RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        let queue = Arc::new(FakeQueue::default());
        // The scheduler is the liveness authority for the active Apply-limit
        // gate, so the service the run-control path composes with observes the
        // *same* handle the dispatch decision reads.
        let operator = Arc::new(
            OperatorCommandService::new(
                state.clone(),
                queue.clone(),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_run_boundary(scheduler.clone()),
        );
        let resolves = Arc::new(ResolveReservations::new());
        let eligibility = Arc::new(StartEligibility::new());
        Self {
            service: RunControlService::new(
                state.clone(),
                operator.clone(),
                scheduler.clone(),
                resolves.clone(),
                eligibility.clone(),
            ),
            state,
            scheduler,
            marks,
            queue,
            operator,
            resolves,
            eligibility,
        }
    }

    fn mark(&self, change_ids: &[&str]) {
        self.marks
            .replace(change_ids.iter().map(|id| id.to_string()));
    }

    /// Everything the run-control path could have mutated outside the mark store.
    ///
    /// A single snapshot keeps "failed admission left no partial effect" a
    /// comparison rather than a list of assertions that can silently miss one.
    async fn effects(&self) -> RunEffects {
        RunEffects {
            scheduler: self.scheduler.calls(),
            queue: self.queue.entries.lock().unwrap().clone(),
            notifications: *self.queue.notifications.lock().unwrap(),
            statuses: {
                let guard = self.state.read().await;
                let mut statuses: Vec<(String, String)> = guard
                    .tracked_change_ids()
                    .into_iter()
                    .map(|id| {
                        let status = guard.display_status(&id).to_string();
                        (id, status)
                    })
                    .collect();
                statuses.sort();
                statuses
            },
        }
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

    /// Reconstruct a merge wait the way TUI startup does.
    ///
    /// No prior reducer history exists; the only input is a refresh scan that
    /// reports the change as archived but not yet merged into base.
    async fn to_startup_merge_wait(&self, change_id: &str) {
        use std::collections::{HashMap, HashSet};

        self.apply(ExecutionEvent::ChangesRefreshed {
            changes: Vec::new(),
            rejected_changes: Vec::new(),
            committed_change_ids: HashSet::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::from([change_id.to_string()]),
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

/// Every observable effect a run command could leave behind, outside marks.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunEffects {
    scheduler: Vec<SchedulerCall>,
    queue: Vec<String>,
    notifications: usize,
    statuses: Vec<(String, String)>,
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
            excluded: Vec::new(),
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
    harness.eligibility.set_parallel_ineligible([(
        "a".to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("worktree execution refuses uncommitted changes");

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
        harness.eligibility.set_parallel_ineligible([(
            "ineligible".to_string(),
            ParallelEligibility::UncommittedProposalFiles,
        )]);
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
    harness
        .scheduler
        .fail_launch("the scheduler refused this launch");

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
            excluded: Vec::new(),
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
            excluded: Vec::new(),
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
        vec![SchedulerCall::GracefulStop(true), SchedulerCall::Notified],
        "the request is recorded, then the idle waiter is woken so a parked \
         scheduler can reach its stop boundary without an unrelated event"
    );

    // `Select` stays refused while no scheduler is alive: pre-run selection has
    // nothing to stop, and liveness — not the mode name — is the widening rule.
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
async fn resolve_from_startup_reconstructed_merge_wait_reserves_and_dispatches() {
    let harness = Harness::new(&["a"]);
    harness.to_startup_merge_wait("a").await;
    assert_eq!(
        harness.status("a").await,
        "merge wait",
        "workspace evidence alone must make the reducer agree with the row"
    );

    let outcome = harness.service.resolve_merge("a").await.unwrap();

    assert_eq!(
        outcome,
        RunControlOutcome::ResolveReserved {
            change_id: "a".to_string(),
            reservation: ResolveReservation::Active,
            scheduler: SchedulerEffect::Started,
        }
    );
    assert_eq!(harness.status("a").await, "resolve pending");
    assert!(harness.resolves.is_active());
    assert_eq!(
        harness.state.read().await.resolve_wait_change_ids(),
        vec!["a".to_string()],
        "the accepted intent must be scheduler-consumable retry membership"
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![Vec::<String>::new()]
    );
}

#[tokio::test]
async fn resolve_from_startup_merge_wait_notifies_a_live_scheduler() {
    let harness = Harness::new(&["a"]);
    harness.to_startup_merge_wait("a").await;
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
async fn resolve_of_a_not_queued_target_without_workspace_evidence_is_refused() {
    // Same reducer shape the startup case starts from, minus the refresh
    // evidence: admission must still refuse it.
    let harness = Harness::new(&["a"]);
    assert_eq!(harness.status("a").await, "not queued");

    let error = harness
        .service
        .resolve_merge("a")
        .await
        .expect_err("an idle, not-queued change is not waiting on a merge");

    assert!(matches!(
        error,
        RunControlError::TargetIneligible {
            command: RunCommandKind::Resolve,
            ..
        }
    ));
    assert_eq!(harness.status("a").await, "not queued");
    assert!(!harness.resolves.is_active());
    assert!(harness
        .state
        .read()
        .await
        .resolve_wait_change_ids()
        .is_empty());
    assert!(harness.scheduler.calls().is_empty());
}

#[tokio::test]
async fn refresh_evidence_for_another_change_leaves_a_stale_target_ineligible() {
    let harness = Harness::new(&["a", "b"]);
    harness.to_startup_merge_wait("b").await;

    let error = harness
        .service
        .resolve_merge("a")
        .await
        .expect_err("evidence for 'b' must not make 'a' resolve-eligible");

    assert!(matches!(
        error,
        RunControlError::TargetIneligible {
            command: RunCommandKind::Resolve,
            ..
        }
    ));
    assert!(!harness.resolves.is_reserved("a"));
    assert!(harness.scheduler.calls().is_empty());
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

/// Eligibility is an unconditional input to start selection: an ineligible
/// target is rejected with no mode to turn the constraint off.
#[test]
fn start_eligibility_always_rejects_an_ineligible_target() {
    let eligibility = StartEligibility::new();
    let targets = vec!["a".to_string(), "b".to_string()];

    assert!(
        eligibility.rejected(&targets).is_empty(),
        "nothing is rejected before an ineligible observation exists"
    );

    eligibility.set_parallel_ineligible([(
        "a".to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);
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

// ============================================================================
// Explicit-intent boundary
// ============================================================================

#[tokio::test]
async fn tui_and_remote_start_produce_identical_explicit_target_eligibility() {
    // Two frontends, one boundary. Each harness stands for one frontend's call
    // into the same shared service with the same observed marks.
    let tui = Harness::new(&["marked-a", "marked-b", "unmarked-residue"]);
    let remote = Harness::new(&["marked-a", "marked-b", "unmarked-residue"]);
    tui.mark(&["marked-a", "marked-b"]);
    remote.mark(&["marked-a", "marked-b"]);

    let tui_outcome = tui
        .service
        .start(OperatorMode::Select)
        .await
        .expect("TUI start");
    let remote_outcome = remote
        .service
        .start(OperatorMode::Select)
        .await
        .expect("remote start");

    assert_eq!(tui_outcome, remote_outcome);
    assert_eq!(
        tui.scheduler.started_targets(),
        remote.scheduler.started_targets()
    );

    let mut tui_queued = tui.state.read().await.queued_change_ids();
    let mut remote_queued = remote.state.read().await.queued_change_ids();
    tui_queued.sort();
    remote_queued.sort();
    assert_eq!(
        tui_queued,
        vec!["marked-a".to_string(), "marked-b".to_string()]
    );
    assert_eq!(
        tui_queued, remote_queued,
        "equivalent accepted intent must produce identical scheduler eligibility"
    );

    for harness in [&tui, &remote] {
        assert!(
            !harness
                .state
                .read()
                .await
                .is_ordinary_queue_eligible("unmarked-residue"),
            "an unmarked catalog or worktree entry must stay ineligible"
        );
    }
}

#[tokio::test]
async fn queue_removal_and_dequeue_revoke_eligibility_until_explicit_requeue() {
    let harness = Harness::new(&["alpha"]);
    harness.mark(&["alpha"]);
    harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("start");

    assert!(harness
        .state
        .read()
        .await
        .is_ordinary_queue_eligible("alpha"));

    harness
        .service
        .operator()
        .remove_from_queue("alpha")
        .await
        .expect("queue removal");
    assert!(
        !harness
            .state
            .read()
            .await
            .is_ordinary_queue_eligible("alpha"),
        "queue removal revokes ordinary execution eligibility immediately"
    );

    // Stop-and-dequeue is the other revocation path, and it is stickier: the
    // `dequeued` flag survives until an explicit requeue clears it.
    harness
        .service
        .operator()
        .add_to_queue("alpha")
        .await
        .expect("requeue");
    harness
        .service
        .operator()
        .stop_and_dequeue("alpha")
        .await
        .expect("stop and dequeue");
    assert!(
        !harness
            .state
            .read()
            .await
            .is_ordinary_queue_eligible("alpha"),
        "a dequeued change must not be reacquired"
    );

    harness
        .service
        .operator()
        .add_to_queue("alpha")
        .await
        .expect("explicit requeue");
    assert!(
        harness
            .state
            .read()
            .await
            .is_ordinary_queue_eligible("alpha"),
        "explicit requeue is the ordinary way back"
    );
}

// ============================================================================
// Active-run Apply iteration limit: boundary ordering and cross-adapter parity
// ============================================================================
//
// Integration evidence for the *service composition*: the reducer, the operator
// service, and the run-control dispatch path are wired together as production
// wires them, and scheduler liveness is driven deterministically instead of by a
// real task. No process, repository, network, or timer is involved.

impl Harness {
    /// Record a terminal error plus the typed Apply-ceiling evidence for it.
    async fn to_iteration_limit(&self, change_id: &str, attempts: u32, max: u32) {
        self.to_error(change_id).await;
        self.state
            .write()
            .await
            .record_apply_iteration_limit(change_id, attempts, max);
    }
}

#[tokio::test]
async fn active_iteration_limit_run_boundary_refuses_retry_while_the_task_is_live() {
    let harness = Harness::new(&["limited"]);
    harness.to_iteration_limit("limited", 50, 50).await;
    harness.scheduler.set_running(true);

    let error = harness
        .service
        .retry_change("limited")
        .await
        .expect_err("the owning scheduler task is still live");

    assert!(
        matches!(
            error,
            RunControlError::Operator(OperatorCommandError::ApplyIterationLimitActive { .. })
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(harness.status("limited").await, "error");
    assert!(
        harness.marks.marked_ids().is_empty(),
        "a refused retry marks nothing"
    );
    assert!(
        !harness.scheduler.calls().iter().any(|call| matches!(
            call,
            SchedulerCall::Notified | SchedulerCall::Started { .. }
        )),
        "no accepted retry may be notified into the exhausted scheduler: {:?}",
        harness.scheduler.calls()
    );
}

/// Task exit retires the gate, and a later admission *starts* a boundary rather
/// than waking the one that exited.
#[tokio::test]
async fn active_iteration_limit_run_boundary_admits_a_later_run_after_task_exit() {
    let harness = Harness::new(&["limited"]);
    harness.to_iteration_limit("limited", 50, 50).await;
    harness.scheduler.set_running(true);
    assert!(harness.service.retry_change("limited").await.is_err());

    // The run task returned; its record is deliberately still in shared state.
    harness.scheduler.set_running(false);

    let outcome = harness
        .service
        .retry_change("limited")
        .await
        .expect("a closed boundary admits the ordinary retry route");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["limited".to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            excluded: Vec::new(),
        },
        "a later boundary is started, never a wake-up of the exited scheduler"
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["limited".to_string()]]
    );
    assert!(
        !harness
            .scheduler
            .calls()
            .iter()
            .any(|call| matches!(call, SchedulerCall::Notified)),
        "the exited scheduler is never notified: {:?}",
        harness.scheduler.calls()
    );
    assert!(
        harness
            .state
            .read()
            .await
            .apply_iteration_limit("limited")
            .is_some(),
        "admission does not clear the record; only a later boundary replaces this state"
    );
}

/// A same-process later boundary owns fresh active-run state and a fresh budget,
/// derived from the preserved workspace rather than from the old counter.
#[tokio::test]
async fn active_iteration_limit_run_boundary_later_state_starts_with_a_fresh_budget() {
    let harness = Harness::new(&["limited"]);
    harness.to_iteration_limit("limited", 50, 50).await;

    // What a later scheduler boundary installs: new active-run state over the
    // same workspace-derived change set.
    let later = OrchestratorState::new(vec!["limited".to_string()], 50);

    assert!(
        later.apply_iteration_limit("limited").is_none(),
        "the ephemeral gate never crosses into a later boundary"
    );
    assert_eq!(later.apply_count("limited"), 0, "the budget starts fresh");
    assert_eq!(later.parallel_finish_report(), ("completed", 0));
}

#[tokio::test]
async fn active_iteration_limit_bulk_retry_dispatches_only_the_admitted_targets() {
    let harness = Harness::new(&["limited", "ordinary"]);
    harness.to_iteration_limit("limited", 50, 50).await;
    harness.to_error("ordinary").await;
    harness.scheduler.set_running(true);
    harness.mark(&["limited", "ordinary"]);

    let outcome = harness
        .service
        .retry_errors(&["limited".to_string(), "ordinary".to_string()])
        .await
        .expect("an unrelated retryable target keeps the bulk request useful");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["ordinary".to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Notified,
            excluded: Vec::new(),
        },
        "the limited target is neither dispatched nor reported as accepted"
    );
    assert_eq!(harness.status("limited").await, "error");
}

#[tokio::test]
async fn active_iteration_limit_bulk_retry_all_limited_produces_no_scheduler_effect() {
    let harness = Harness::new(&["a", "b"]);
    harness.to_iteration_limit("a", 50, 50).await;
    harness.to_iteration_limit("b", 50, 50).await;
    harness.scheduler.set_running(true);
    let calls_before = harness.scheduler.calls();

    let outcome = harness
        .service
        .retry_errors(&["a".to_string(), "b".to_string()])
        .await
        .expect("an all-limited request settles truthfully");

    assert_eq!(
        outcome,
        RunControlOutcome::NoOp {
            reason: RunNoOpReason::NoRetryableTarget
        }
    );
    assert_eq!(
        harness.scheduler.calls(),
        calls_before,
        "no notify and no start for an all-limited request"
    );
    assert_eq!(harness.status("a").await, "error");
    assert_eq!(harness.status("b").await, "error");
}

/// `start` in Error mode routes through the same bulk retry, so the F5 path
/// inherits the guard rather than reimplementing it.
#[tokio::test]
async fn active_iteration_limit_bulk_retry_error_mode_start_is_mutation_free() {
    let harness = Harness::new(&["limited"]);
    harness.to_iteration_limit("limited", 50, 50).await;
    harness.scheduler.set_running(true);
    harness.mark(&["limited"]);
    let calls_before = harness.scheduler.calls();

    let outcome = harness
        .service
        .start(OperatorMode::Error)
        .await
        .expect("Error-mode start settles as a no-op rather than failing");

    assert_eq!(
        outcome,
        RunControlOutcome::NoOp {
            reason: RunNoOpReason::NoRetryableTarget
        }
    );
    assert_eq!(harness.scheduler.calls(), calls_before);
    assert_eq!(harness.status("limited").await, "error");
}

// ============================================================================
// Final-admission ownership of run-target validity
// ============================================================================
//
// A mark is next-run target intent and nothing else: it carries no eligibility
// of its own and is accepted for any non-terminal row at any time. These tests
// pin the consequence — the *whole* run-target decision belongs to the final
// start/retry admission, is taken from current reducer and worktree facts, and
// leaves nothing behind when it refuses.

/// Marks recorded while a row was unrunnable are re-read at admission.
///
/// Nothing about the mark changes between the two starts; only the reducer
/// status does. That is what makes the eligibility decision admission-owned
/// rather than a mark-time verdict that outlived the state it was taken from.
#[tokio::test]
async fn run_mark_intent_start_admission_reads_current_status_not_mark_time_status() {
    let harness = Harness::new(&["a"]);
    harness
        .state
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue("a".to_string()));
    harness.mark(&["a"]);

    let refused = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("an already-queued row is not a start target");
    assert!(matches!(refused, RunControlError::NoEligibleTarget { .. }));

    // The same mark, a later reducer state: now it runs.
    harness
        .state
        .write()
        .await
        .apply_command(ReducerCommand::RemoveFromQueue("a".to_string()));
    assert_eq!(harness.status("a").await, "not queued");

    let outcome = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("the unchanged mark is admitted once the status allows it");

    assert!(matches!(
        outcome,
        RunControlOutcome::RunDispatched { .. }
    ));
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["a".to_string()]]
    );
}

/// A non-startable mark is excluded by name; it does not block runnable work.
#[tokio::test]
async fn run_mark_intent_start_admission_excludes_status_without_blocking_runnable_targets() {
    let harness = Harness::new(&["runnable", "waiting"]);
    harness.to_merge_wait("waiting").await;
    harness.mark(&["runnable", "waiting"]);

    let outcome = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("one non-startable mark does not refuse the runnable subset");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["runnable".to_string()],
            explicit_retry: false,
            scheduler: SchedulerEffect::Started,
            excluded: vec![ExcludedTarget {
                change_id: "waiting".to_string(),
                status: "merge wait".to_string(),
            }],
        },
        "the admitted run must name the marked target it left out, and why"
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["runnable".to_string()]]
    );
    assert_eq!(
        harness.marks.marked_ids(),
        vec!["runnable".to_string(), "waiting".to_string()],
        "admission consumes marks; it does not revoke them"
    );
}

/// Zero runnable targets rejects, and the refusal names every exclusion.
#[tokio::test]
async fn run_mark_intent_start_admission_rejects_when_no_runnable_target_remains() {
    let harness = Harness::new(&["waiting", "failed"]);
    harness.to_merge_wait("waiting").await;
    harness.to_error("failed").await;
    harness.mark(&["waiting", "failed"]);
    let before = harness.effects().await;

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("no marked target is startable");

    match &error {
        RunControlError::NoEligibleTarget { detail, .. } => {
            assert!(
                detail.contains("waiting (merge wait)") && detail.contains("failed (error)"),
                "every exclusion must be named with its status: {detail}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        harness.effects().await,
        before,
        "a rejected admission leaves no queue, scheduler, or reducer effect"
    );
}

/// The worktree fence refuses the complete request, effect-free.
#[tokio::test]
async fn run_mark_intent_start_admission_worktree_fence_rejects_the_whole_request() {
    let harness = Harness::new(&["runnable", "ineligible"]);
    harness.mark(&["runnable", "ineligible"]);
    harness.eligibility.set_parallel_ineligible([(
        "ineligible".to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);
    let before = harness.effects().await;

    let error = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect_err("one worktree-ineligible mark refuses the whole request");

    match &error {
        RunControlError::NoEligibleTarget { detail, .. } => assert!(
            detail.contains("ineligible"),
            "the refusal must name the fenced target: {detail}"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        harness.effects().await,
        before,
        "the fence refuses before any partial effect exists"
    );
}

/// Error-mode Start routes only marked retry-eligible rows.
#[tokio::test]
async fn run_mark_intent_start_admission_error_mode_routes_only_retryable_marks() {
    let harness = Harness::new(&["failed", "idle"]);
    harness.to_error("failed").await;
    harness.mark(&["failed", "idle"]);

    let outcome = harness
        .service
        .start(OperatorMode::Error)
        .await
        .expect("the retryable mark is routed");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec!["failed".to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            excluded: vec![ExcludedTarget {
                change_id: "idle".to_string(),
                status: "not queued".to_string(),
            }],
        },
        "a marked row without retryable evidence is reported, not retried"
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["failed".to_string()]]
    );
}

/// Error-mode Start with nothing retryable rejects and names the exclusions.
#[tokio::test]
async fn run_mark_intent_start_admission_error_mode_rejects_without_retryable_marks() {
    let harness = Harness::new(&["idle"]);
    harness.mark(&["idle"]);
    let before = harness.effects().await;

    let error = harness
        .service
        .start(OperatorMode::Error)
        .await
        .expect_err("no marked row carries retryable evidence");

    match &error {
        RunControlError::NoEligibleTarget { detail, .. } => assert!(
            detail.contains("idle (not queued)"),
            "the refusal must name the exclusion: {detail}"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(harness.effects().await, before);
}

/// Unmarking after admission changes only the next run, never this one.
#[tokio::test]
async fn run_mark_intent_start_admission_unmark_does_not_cancel_or_dequeue_admitted_work() {
    let harness = Harness::new(&["a", "b"]);
    harness.mark(&["a", "b"]);
    harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("both marked targets are startable");
    let admitted = harness.effects().await;

    // The operator changes their mind *after* the run took the target.
    let changed = harness
        .operator
        .set_execution_mark("a", false)
        .await
        .expect("a non-terminal row accepts an unmark at any time");
    assert!(matches!(
        changed,
        crate::orchestration::operator_command::OperatorOutcome::MarkSet {
            marked: false,
            ..
        }
    ));

    assert_eq!(harness.marks.marked_ids(), vec!["b".to_string()]);
    assert_eq!(
        harness.effects().await,
        admitted,
        "unmarking must not dequeue, cancel, unschedule, or restatus admitted work"
    );
}
