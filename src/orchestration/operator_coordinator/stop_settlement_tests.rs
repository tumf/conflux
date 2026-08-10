//! Phase-aware stop-and-dequeue settlement.
//!
//! The incident these exist for: a stop-and-dequeue succeeded, the command
//! record said only "cancelled and dequeued", and the display status still read
//! `applying` — so the caller concluded no Apply commit existed and made a
//! duplicate one. Apply had in fact finished, and Acceptance was what got
//! cancelled.
//!
//! Everything below is driven by channels and typed events, never by sleeping or
//! by a wall-clock threshold. The race case is deterministic by construction: the
//! termination token is cancelled *only* after the Apply completion and the
//! Acceptance transition have been dispatched, so "the commit landed before
//! settlement" is an ordering the test establishes rather than one it hopes for.
//!
//! Unit-scoped: the reducer, the queue, the scheduler, the facts store, and the
//! Git evidence port are all in-memory doubles. No process, repository, or
//! network is touched.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::events::{EventDispatcher, ExecutionEvent};
use crate::orchestration::apply_commit_evidence::{ApplyCommitEvidence, ApplyCommitEvidencePort};
use crate::orchestration::execution_facts::{ExecutionFactsStore, ExecutionPhase};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, ParallelRuntime, QueuePort,
    StopSettlement, TerminationWaiter,
};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::OrchestratorState;

/// The OID a fake Apply worker publishes as its final commit.
const APPLY_OID: &str = "9f1c0de0c0ffee0000000000000000000000abcd";

/// Cancellation port that reports when cancellation was issued and completes
/// termination only when the test says so.
struct SignallingQueue {
    token: CancellationToken,
    issued: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// `None` makes the target look already-terminated.
    waiter_is_live: bool,
}

impl SignallingQueue {
    fn live(token: CancellationToken, issued: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            token,
            issued: Mutex::new(Some(issued)),
            waiter_is_live: true,
        }
    }

    fn already_terminated() -> Self {
        Self {
            token: CancellationToken::new(),
            issued: Mutex::new(None),
            waiter_is_live: false,
        }
    }
}

#[async_trait]
impl QueuePort for SignallingQueue {
    async fn add(&self, _change_id: &str) -> bool {
        true
    }

    async fn remove(&self, _change_id: &str) -> bool {
        true
    }

    async fn request_cancellation(
        &self,
        _change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        if let Some(issued) = self
            .issued
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = issued.send(());
        }
        if self.waiter_is_live {
            Ok(Some(TerminationWaiter::new(self.token.clone())))
        } else {
            Ok(Some(TerminationWaiter::already_terminated()))
        }
    }

    async fn notify_scheduler(&self) {}
}

/// Repository evidence double that proves whichever OID it is handed.
#[derive(Default)]
struct ProvingPort {
    calls: AtomicUsize,
}

#[async_trait]
impl ApplyCommitEvidencePort for ProvingPort {
    async fn observe(&self, _change_id: &str, oid: &str) -> ApplyCommitEvidence {
        self.calls.fetch_add(1, Ordering::SeqCst);
        ApplyCommitEvidence::proven(oid)
    }
}

/// Repository evidence double that can never read the worktree.
struct UnreadablePort;

#[async_trait]
impl ApplyCommitEvidencePort for UnreadablePort {
    async fn observe(&self, _change_id: &str, _oid: &str) -> ApplyCommitEvidence {
        ApplyCommitEvidence::unknown()
    }
}

/// Everything one settlement test needs, wired the way production wires it.
struct Fixture {
    application: Arc<OperatorApplication>,
    dispatcher: Arc<EventDispatcher>,
    state: Arc<tokio::sync::RwLock<OrchestratorState>>,
    facts: Arc<ExecutionFactsStore>,
}

fn fixture(
    queue: Arc<SignallingQueue>,
    evidence: Option<Arc<dyn ApplyCommitEvidencePort>>,
    bind_facts: bool,
) -> Fixture {
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["c1".to_string()],
        10,
    )));
    let facts = Arc::new(ExecutionFactsStore::new());
    let parallel = Arc::new(ParallelRuntime::new());
    let mut operator = OperatorCommandService::new(
        state.clone(),
        queue,
        Arc::new(NoopQueueHooks),
        Arc::new(ExecutionMarkStore::new()),
    )
    .with_parallel(parallel.clone())
    // A generous safety valve against a hang, never a correctness assertion:
    // every ordering below is established by the channel, not by this bound.
    .with_cancellation_timeout(Duration::from_secs(30));
    if bind_facts {
        operator = operator.with_execution_facts(facts.clone());
    }
    let run_control = Arc::new(RunControlService::new(
        state.clone(),
        Arc::new(operator),
        Arc::new(RecordingScheduler::new()),
        Arc::new(ResolveReservations::new()),
        parallel,
    ));
    let dispatcher = Arc::new(
        EventDispatcher::new(state.clone(), crate::events::cli_event_sinks())
            .with_execution_facts(bind_facts.then(|| facts.clone())),
    );
    let application = Arc::new(
        OperatorApplication::new(Arc::new(CoreMode::new()), run_control, dispatcher.clone())
            .with_apply_commit_evidence(evidence),
    );
    Fixture {
        application,
        dispatcher,
        state,
        facts,
    }
}

fn apply_started() -> ExecutionEvent {
    ExecutionEvent::ApplyStarted {
        change_id: "c1".to_string(),
        command: "agent apply".to_string(),
    }
}

fn settlement_of(result: ApplicationResult) -> StopSettlement {
    match result.outcome {
        Ok(ApplicationOutcome::Operator(OperatorOutcome::Dequeued { settlement, .. })) => {
            settlement
        }
        other => panic!("expected a successful dequeue, got {other:?}"),
    }
}

/// The regression. Apply's final commit lands and Acceptance starts *while*
/// cancellation is in flight; settlement must name Acceptance as what was
/// cancelled, Apply as what completed, and the exact retained commit OID.
#[tokio::test]
async fn agent_execution_observability_apply_commit_stop_race() {
    let token = CancellationToken::new();
    let (issued_tx, issued_rx) = tokio::sync::oneshot::channel();
    let queue = Arc::new(SignallingQueue::live(token.clone(), issued_tx));
    let port = Arc::new(ProvingPort::default());
    let fixture = fixture(queue, Some(port.clone()), true);

    // The change is applying when the operator intervenes.
    fixture.dispatcher.dispatch(apply_started()).await;

    let pending = fixture
        .application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("phase one issues cancellation for a live handle");

    // The fake Apply worker: it only reacts *after* cancellation was issued, and
    // it confirms termination *only* after its final commit and the Acceptance
    // transition are both visible. That ordering is the whole test.
    let worker_dispatcher = fixture.dispatcher.clone();
    let worker = tokio::spawn(async move {
        issued_rx.await.expect("cancellation must be issued first");
        worker_dispatcher
            .dispatch(ExecutionEvent::ApplyCompleted {
                change_id: "c1".to_string(),
                revision: APPLY_OID.to_string(),
            })
            .await;
        worker_dispatcher
            .dispatch(ExecutionEvent::AcceptanceStarted {
                change_id: "c1".to_string(),
                command: "agent accept".to_string(),
            })
            .await;
        token.cancel();
    });

    let result = fixture.application.settle_stop_and_dequeue(pending).await;
    worker.await.expect("worker task");

    let settlement = settlement_of(result);
    assert_eq!(
        settlement.cancelled_phase,
        ExecutionPhase::Acceptance,
        "the phase active at settlement is what was cancelled, not the one \
         observed when the command was admitted"
    );
    assert_eq!(settlement.last_completed_phase, Some(ExecutionPhase::Apply));
    assert_eq!(settlement.apply_commit_present, Some(true));
    assert_eq!(settlement.apply_commit_oid.as_deref(), Some(APPLY_OID));
    assert_eq!(
        port.calls.load(Ordering::SeqCst),
        1,
        "repository evidence is observed exactly once, after termination"
    );
    assert_eq!(
        fixture.state.read().await.display_status("c1"),
        "not queued",
        "the dequeue itself still committed"
    );

    let detail = settlement.describe("c1");
    assert!(detail.contains("acceptance"), "{detail}");
    assert!(detail.contains(APPLY_OID), "{detail}");
    assert!(
        detail.contains("not rolled back"),
        "the detail must deny rollback: {detail}"
    );
}

/// Stopping before the final Apply commit must not claim one exists.
#[tokio::test]
async fn agent_execution_observability_stop_result_before_apply_commit() {
    let token = CancellationToken::new();
    let (issued_tx, issued_rx) = tokio::sync::oneshot::channel();
    let queue = Arc::new(SignallingQueue::live(token.clone(), issued_tx));
    let fixture = fixture(queue, Some(Arc::new(ProvingPort::default())), true);

    fixture.dispatcher.dispatch(apply_started()).await;
    let pending = fixture
        .application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("cancellation issued");

    let worker = tokio::spawn(async move {
        issued_rx.await.expect("cancellation issued");
        // The worker dies mid-Apply: no completion, no commit.
        token.cancel();
    });

    let result = fixture.application.settle_stop_and_dequeue(pending).await;
    worker.await.expect("worker task");

    let settlement = settlement_of(result);
    assert_eq!(settlement.cancelled_phase, ExecutionPhase::Apply);
    assert_eq!(settlement.last_completed_phase, None);
    assert_eq!(
        settlement.apply_commit_present, None,
        "no retained completion fact means unknown, never 'definitely absent'"
    );
    assert_eq!(settlement.apply_commit_oid, None);
    let detail = settlement.describe("c1");
    assert!(detail.contains("could not be proven"), "{detail}");
    assert!(detail.contains("not rolled back"), "{detail}");
}

/// The phase named is whichever one the reducer actually held at settlement.
#[tokio::test]
async fn agent_execution_observability_stop_result_names_the_settlement_phase() {
    let cases: Vec<(&str, ExecutionEvent, ExecutionPhase)> = vec![
        (
            "archive",
            ExecutionEvent::ArchiveStarted {
                change_id: "c1".to_string(),
                command: "agent archive".to_string(),
            },
            ExecutionPhase::Archive,
        ),
        (
            "resolve",
            ExecutionEvent::ResolveStarted {
                change_id: "c1".to_string(),
                command: "agent resolve".to_string(),
            },
            ExecutionPhase::Resolve,
        ),
        (
            "preparing",
            ExecutionEvent::WorkspacePreparationStarted {
                change_id: "c1".to_string(),
            },
            ExecutionPhase::Preparing,
        ),
    ];

    for (label, start, expected) in cases {
        let token = CancellationToken::new();
        let (issued_tx, issued_rx) = tokio::sync::oneshot::channel();
        let queue = Arc::new(SignallingQueue::live(token.clone(), issued_tx));
        let fixture = fixture(queue, Some(Arc::new(ProvingPort::default())), true);

        fixture.dispatcher.dispatch(start).await;
        let pending = fixture
            .application
            .begin_stop_and_dequeue("c1")
            .await
            .expect("cancellation issued");
        let worker = tokio::spawn(async move {
            issued_rx.await.expect("cancellation issued");
            token.cancel();
        });

        let settlement = settlement_of(fixture.application.settle_stop_and_dequeue(pending).await);
        worker.await.expect("worker task");

        assert_eq!(
            settlement.cancelled_phase, expected,
            "{label} must be named as the cancelled phase"
        );
    }
}

/// An already-terminated target cancelled nothing, and says so.
#[tokio::test]
async fn agent_execution_observability_stop_result_already_terminated_is_none() {
    let queue = Arc::new(SignallingQueue::already_terminated());
    let fixture = fixture(queue, Some(Arc::new(ProvingPort::default())), true);
    // Queued but idle: nothing is executing, so there is no phase to cancel.
    fixture.state.write().await.apply_command(
        crate::orchestration::state::ReducerCommand::AddToQueue("c1".to_string()),
    );

    let pending = fixture
        .application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("an idle row has nothing to terminate");
    let settlement = settlement_of(fixture.application.settle_stop_and_dequeue(pending).await);

    assert_eq!(settlement.cancelled_phase, ExecutionPhase::None);
    assert_eq!(settlement.last_completed_phase, None);
    assert_eq!(settlement.apply_commit_present, None);
    let detail = settlement.describe("c1");
    assert!(detail.contains("already terminated"), "{detail}");
    assert!(detail.contains("not rolled back"), "{detail}");
}

/// Unreadable repository evidence stays unknown: the dequeue still succeeds, and
/// the uncertainty is published rather than resolved by guessing.
#[tokio::test]
async fn agent_execution_observability_stop_result_unreadable_evidence_is_unknown() {
    let token = CancellationToken::new();
    let (issued_tx, issued_rx) = tokio::sync::oneshot::channel();
    let queue = Arc::new(SignallingQueue::live(token.clone(), issued_tx));
    let fixture = fixture(queue, Some(Arc::new(UnreadablePort)), true);

    fixture.dispatcher.dispatch(apply_started()).await;
    let pending = fixture
        .application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("cancellation issued");
    let dispatcher = fixture.dispatcher.clone();
    let worker = tokio::spawn(async move {
        issued_rx.await.expect("cancellation issued");
        dispatcher
            .dispatch(ExecutionEvent::ApplyCompleted {
                change_id: "c1".to_string(),
                revision: APPLY_OID.to_string(),
            })
            .await;
        token.cancel();
    });

    let settlement = settlement_of(fixture.application.settle_stop_and_dequeue(pending).await);
    worker.await.expect("worker task");

    assert_eq!(
        fixture.facts.apply_commit_oid("c1").as_deref(),
        Some(APPLY_OID),
        "the typed fact was retained; only the repository proof failed"
    );
    assert_eq!(settlement.last_completed_phase, Some(ExecutionPhase::Apply));
    assert_eq!(settlement.apply_commit_present, None);
    assert_eq!(settlement.apply_commit_oid, None);
    assert_eq!(
        fixture.state.read().await.display_status("c1"),
        "not queued",
        "explanatory uncertainty must not gate the dequeue itself"
    );
}

/// A process with no observability store or no evidence port reports unknown
/// rather than inventing either half.
#[tokio::test]
async fn agent_execution_observability_stop_result_unbound_evidence_is_unknown() {
    let token = CancellationToken::new();
    let (issued_tx, issued_rx) = tokio::sync::oneshot::channel();
    let queue = Arc::new(SignallingQueue::live(token.clone(), issued_tx));
    let fixture = fixture(queue, None, false);

    fixture.dispatcher.dispatch(apply_started()).await;
    let pending = fixture
        .application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("cancellation issued");
    let worker = tokio::spawn(async move {
        issued_rx.await.expect("cancellation issued");
        token.cancel();
    });

    let settlement = settlement_of(fixture.application.settle_stop_and_dequeue(pending).await);
    worker.await.expect("worker task");

    assert_eq!(
        settlement.cancelled_phase,
        ExecutionPhase::Apply,
        "the reducer is the phase authority, so it answers with or without a \
         facts store"
    );
    assert_eq!(settlement.last_completed_phase, None);
    assert_eq!(settlement.apply_commit_present, None);
}

/// An unclassifiable phase is reported as unknown rather than as "nothing was
/// running".
///
/// A successful dequeue cannot reach this state — the reducer refuses to dequeue
/// a change it does not track, so the command settles as a no-op instead — which
/// is why the projection is asserted directly here.
#[test]
fn agent_execution_observability_stop_result_unknown_phase_is_explicit() {
    let settlement = StopSettlement {
        cancelled_phase: ExecutionPhase::Unknown,
        ..StopSettlement::none()
    };
    let detail = settlement.describe("c1");
    assert!(detail.contains("could not be determined"), "{detail}");
    assert!(detail.contains("not rolled back"), "{detail}");
}
