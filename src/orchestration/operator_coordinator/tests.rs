//! Application-transaction regressions.
//!
//! These cover the two properties the coordinator exists to guarantee, which no
//! amount of service-level testing can establish on its own:
//!
//! - a preparation failure leaves *nothing* behind, because preparation runs
//!   before any mutation;
//! - an accepted command's outcome is published *before* the scheduler activity
//!   that command enabled can emit anything.
//!
//! Unit-scoped: the scheduler is the shared recording double, the queue is the
//! in-memory `DynamicQueue`, and no process, repository, or network is touched.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;

use super::*;
use crate::events::{EventSink, ExecutionEvent, OutcomeRevisions};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandError, OperatorCommandService,
    ParallelRuntime, QueuePort, TerminationWaiter,
};
use crate::orchestration::run_control::testing::{RecordingScheduler, SchedulerCall};
use crate::orchestration::run_control::{ResolveReservations, RunCommandKind, SchedulerEffect};
use crate::orchestration::state::OrchestratorState;
use crate::tui::queue::DynamicQueue;

// ============================================================================
// Harness
// ============================================================================

/// The single ordered log every observable moment is written to.
///
/// Order is the assertion in most of this file, so one timeline carries both
/// dispatched events and scheduler activations. Keeping them in separate logs
/// would let "the outcome was published" pass while "the outcome was published
/// *first*" quietly stopped being true — and only the second claim matters.
#[derive(Default)]
pub(super) struct Timeline {
    entries: Mutex<Vec<String>>,
}

/// The timeline entry a scheduler activation writes.
pub(super) const SCHEDULER_ACTIVATED: &str = "scheduler:activated";

impl Timeline {
    /// Record an observation from a sink in another test module.
    pub(super) fn push_public(&self, entry: impl Into<String>) {
        self.push(entry);
    }

    fn push(&self, entry: impl Into<String>) {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(entry.into());
    }

    /// Everything observed so far, in order.
    pub(super) fn entries(&self) -> Vec<String> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Only the dispatched-event names, in order.
    pub(super) fn events(&self) -> Vec<String> {
        self.entries()
            .into_iter()
            .filter(|entry| entry != SCHEDULER_ACTIVATED)
            .collect()
    }
}

/// A sink that records the authoritative dispatch order onto the timeline.
pub(super) struct RecordingSink {
    timeline: Arc<Timeline>,
}

#[async_trait]
impl EventSink for RecordingSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        self.timeline.push(crate::events::classify_event(event).0);
    }

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

/// The production wiring, with only the scheduler and the frontend replaced.
pub(super) struct Harness {
    pub(super) state: Arc<tokio::sync::RwLock<OrchestratorState>>,
    pub(super) marks: Arc<ExecutionMarkStore>,
    pub(super) parallel: Arc<ParallelRuntime>,
    pub(super) resolves: Arc<ResolveReservations>,
    pub(super) scheduler: Arc<RecordingScheduler>,
    pub(super) core: Arc<CoreMode>,
    pub(super) timeline: Arc<Timeline>,
    pub(super) dispatcher: Arc<crate::events::EventDispatcher>,
    pub(super) application: Arc<OperatorApplication>,
}

impl Harness {
    pub(super) fn new(change_ids: &[&str]) -> Self {
        let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let resolves = Arc::new(ResolveReservations::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        let operator = Arc::new(
            OperatorCommandService::new(
                state.clone(),
                Arc::new(DynamicQueue::new()),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            state.clone(),
            operator,
            scheduler.clone(),
            resolves.clone(),
            parallel.clone(),
        ));
        let core = Arc::new(CoreMode::new());
        let timeline = Arc::new(Timeline::default());
        let dispatcher = Arc::new(
            crate::events::EventDispatcher::new(
                state.clone(),
                vec![Arc::new(RecordingSink {
                    timeline: timeline.clone(),
                })],
            )
            .with_core_mode(Some(core.clone())),
        );
        // Every activation writes to the same timeline the sink does, so
        // "published before activated" is a single ordered comparison.
        {
            let timeline = timeline.clone();
            scheduler.on_activate(Arc::new(move |_, _| {
                timeline.push(SCHEDULER_ACTIVATED);
            }));
        }
        let application = Arc::new(OperatorApplication::new(
            core.clone(),
            run_control,
            dispatcher.clone(),
        ));
        Self {
            state,
            marks,
            parallel,
            resolves,
            scheduler,
            core,
            timeline,
            dispatcher,
            application,
        }
    }

    pub(super) async fn apply(&self, intent: OperatorIntent) -> ApplicationResult {
        self.application.apply(intent).await
    }

    pub(super) async fn status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    /// Every mutable axis a failed command must leave untouched.
    ///
    /// Compared as one value so a rollback that restored eight of nine axes
    /// still fails: a partially preserved command is exactly the "partial
    /// committed decision state" this transaction exists to make impossible.
    pub(super) async fn axes(&self, change_ids: &[&str]) -> Axes {
        let guard = self.state.read().await;
        Axes {
            statuses: change_ids
                .iter()
                .map(|id| ((*id).to_string(), guard.display_status(id).to_string()))
                .collect(),
            marks: self.marks.marked_ids(),
            active_resolver: self.resolves.active(),
            queued_resolves: self.resolves.waiting(),
            mode: self.core.get(),
            scheduler: self.scheduler.calls(),
            events: self.timeline.entries(),
        }
    }
}

/// The comparable state of every axis an ordinary failure must preserve.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Axes {
    statuses: Vec<(String, String)>,
    marks: Vec<String>,
    active_resolver: Option<String>,
    queued_resolves: Vec<String>,
    mode: OperatorMode,
    scheduler: Vec<SchedulerCall>,
    events: Vec<String>,
}

/// Arrange `change_id` in a reducer-visible merge wait.
pub(super) async fn merge_wait(harness: &Harness, change_id: &str) {
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        });
}

/// Arrange `change_id` as a terminal error.
pub(super) async fn terminal_error(harness: &Harness, change_id: &str) {
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ProcessingError {
            id: change_id.to_string(),
            error: "boom".to_string(),
        });
    // Core is deliberately *not* touched. `ProcessingError` is change-scoped:
    // in production it leaves the process mode exactly where it was, so an
    // arrangement that pushed Core into Error here would be describing a
    // process state this event cannot produce.
}

// ============================================================================
// The ordinary transaction
// ============================================================================

/// Start consumes the authoritative marked set, commits queue intent, projects
/// Running, and activates the scheduler exactly once.
#[tokio::test]
async fn accepted_operator_command_transaction_start_commits_and_projects_once() {
    let harness = Harness::new(&["c1", "c2"]);
    harness.marks.set("c1", true);

    let result = harness.apply(OperatorIntent::Start).await;

    let Ok(ApplicationOutcome::Run(RunControlOutcome::RunDispatched {
        change_ids,
        explicit_retry,
        scheduler,
        ..
    })) = result.outcome
    else {
        panic!("a marked, startable target must dispatch a run: {result:?}");
    };
    assert_eq!(change_ids, vec!["c1".to_string()]);
    assert!(
        !explicit_retry,
        "an ordinary start is not an explicit retry"
    );
    assert_eq!(scheduler, SchedulerEffect::Started);

    assert_eq!(
        harness.core.get(),
        OperatorMode::Running,
        "the accepted outcome must move the one process mode"
    );
    assert_eq!(harness.status("c1").await, "queued");
    assert_eq!(
        harness.status("c2").await,
        "not queued",
        "an unmarked change must not be swept into the run"
    );
    assert_eq!(
        harness.timeline.events(),
        vec!["OperatorCommandApplied"],
        "exactly one authoritative outcome is published"
    );
    assert_eq!(
        harness.scheduler.started_targets(),
        vec![vec!["c1".to_string()]],
        "the scheduler activates once, over the accepted targets"
    );
}

/// A command that is invalid for the current mode changes nothing at all.
#[tokio::test]
async fn accepted_operator_command_transaction_invalid_mode_has_no_effect() {
    for (mode, intent) in [
        // `Running` is deliberately absent: a live run refuses Start for the
        // *target* rather than for the mode, because a marked change-local error
        // stays retryable while unrelated work continues.
        (OperatorMode::Stopping, OperatorIntent::Start),
        (OperatorMode::Select, OperatorIntent::Stop),
        (OperatorMode::Running, OperatorIntent::CancelStop),
        (OperatorMode::Select, OperatorIntent::ForceStop),
        (OperatorMode::Stopped, OperatorIntent::ForceStop),
    ] {
        let harness = Harness::new(&["c1"]);
        harness.marks.set("c1", true);
        harness.core.set(mode);
        let before = harness.axes(&["c1"]).await;

        let result = harness.apply(intent.clone()).await;

        assert!(
            matches!(result.outcome, Err(RunControlError::InvalidMode { .. })),
            "{intent:?} in {mode:?} must be refused for the mode: {result:?}"
        );
        assert_eq!(
            result.revision, None,
            "{intent:?} in {mode:?} dispatched no outcome, so it settles at the admitted revision"
        );
        assert_eq!(
            harness.axes(&["c1"]).await,
            before,
            "{intent:?} in {mode:?} must leave every axis untouched"
        );
    }
}

/// A live run refuses Start for the target, and still changes nothing.
///
/// The mode guard used to answer this case. It cannot any more — a marked
/// change-local error is retryable in `Running` — so what has to hold is that
/// the *target* refusal is just as complete.
#[tokio::test]
async fn accepted_operator_command_transaction_running_start_without_retry_evidence_has_no_effect()
{
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);
    harness.core.set(OperatorMode::Running);
    let before = harness.axes(&["c1"]).await;

    let result = harness.apply(OperatorIntent::Start).await;

    assert!(
        matches!(
            result.outcome,
            Err(RunControlError::NoEligibleTarget { .. })
        ),
        "an ordinary mark under a live run is a target refusal: {result:?}"
    );
    assert_eq!(result.revision, None);
    assert_eq!(
        harness.axes(&["c1"]).await,
        before,
        "a refused Start must leave every axis untouched"
    );
}

/// Start with no eligible target is a refusal, not a false success.
#[tokio::test]
async fn accepted_operator_command_transaction_no_target_start_is_refused() {
    let harness = Harness::new(&["c1"]);
    let before = harness.axes(&["c1"]).await;

    let result = harness.apply(OperatorIntent::Start).await;

    assert!(
        matches!(
            result.outcome,
            Err(RunControlError::NoEligibleTarget {
                command: RunCommandKind::Start,
                ..
            })
        ),
        "an unmarked process has nothing to start: {result:?}"
    );
    assert_eq!(harness.axes(&["c1"]).await, before);
}

/// An accepted resolve projects the reservation, `Running`, and one dispatch;
/// a queued one takes neither the mode nor the scheduler.
#[tokio::test]
async fn accepted_operator_command_transaction_resolve_projects_only_the_active_reservation() {
    let harness = Harness::new(&["c1", "c2"]);
    merge_wait(&harness, "c1").await;
    merge_wait(&harness, "c2").await;

    let first = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c1".to_string(),
        })
        .await;
    assert!(
        matches!(
            &first.outcome,
            Ok(ApplicationOutcome::Run(
                RunControlOutcome::ResolveReserved {
                    reservation: ResolveReservation::Active,
                    scheduler: SchedulerEffect::Started,
                    ..
                }
            ))
        ),
        "the first valid resolve becomes the active resolver: {first:?}"
    );
    assert_eq!(harness.core.get(), OperatorMode::Running);
    assert_eq!(harness.resolves.active().as_deref(), Some("c1"));

    let queued = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c2".to_string(),
        })
        .await;
    assert!(
        matches!(
            &queued.outcome,
            Ok(ApplicationOutcome::Run(
                RunControlOutcome::ResolveReserved {
                    reservation: ResolveReservation::Queued { position: 1 },
                    scheduler: SchedulerEffect::None,
                    ..
                }
            ))
        ),
        "a second resolve waits in FIFO order without dispatching: {queued:?}"
    );
    assert_eq!(harness.resolves.waiting(), vec!["c2".to_string()]);
    assert_eq!(
        harness
            .scheduler
            .calls()
            .iter()
            .filter(|call| matches!(call, SchedulerCall::Started { .. }))
            .count(),
        1,
        "only the active reservation dispatches scheduling"
    );

    // A duplicate reservation is revision-idempotent: no second queue entry, no
    // second dispatch, and nothing published.
    let events_before = harness.timeline.events().len();
    let duplicate = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c2".to_string(),
        })
        .await;
    assert!(
        matches!(
            &duplicate.outcome,
            Ok(ApplicationOutcome::Run(RunControlOutcome::NoOp {
                reason: RunNoOpReason::ResolveAlreadyReserved { .. }
            }))
        ),
        "a duplicate reservation is a no-op: {duplicate:?}"
    );
    assert_eq!(duplicate.revision, None, "a no-op advances no revision");
    assert_eq!(harness.resolves.waiting(), vec!["c2".to_string()]);
    assert_eq!(harness.timeline.events().len(), events_before);
}

/// Explicit retry routes an errored target, commits its intent, and dispatches.
///
/// `Start` in Error mode *is* the retry-every-marked-target route, so the
/// process mode is arranged from the one event that produces it: a typed global
/// fatal error. The change's own terminal state comes from `ProcessingError`,
/// which is change-scoped and deliberately leaves the process mode alone.
#[tokio::test]
async fn accepted_operator_command_transaction_retry_commits_and_dispatches() {
    let harness = Harness::new(&["c1"]);
    terminal_error(&harness, "c1").await;
    harness
        .dispatcher
        .dispatch(ExecutionEvent::Error {
            message: "fatal".to_string(),
        })
        .await;
    assert_eq!(harness.core.get(), OperatorMode::Error);
    harness.marks.set("c1", true);

    let result = harness.apply(OperatorIntent::Start).await;

    let Ok(ApplicationOutcome::Run(RunControlOutcome::RunDispatched {
        change_ids,
        explicit_retry,
        ..
    })) = result.outcome
    else {
        panic!("a marked error row must route through retry: {result:?}");
    };
    assert_eq!(change_ids, vec!["c1".to_string()]);
    assert!(explicit_retry, "retry must carry explicit-retry semantics");
    assert_eq!(harness.core.get(), OperatorMode::Running);
    assert_eq!(
        harness.timeline.events(),
        vec!["Error", "OperatorCommandApplied"],
        "the arranged fatal error, then exactly one accepted outcome"
    );
}

/// Retry with no retryable evidence is rejected with no scheduler effect.
///
/// A mark is next-run intent only, so an idle row may carry one in Error mode.
/// Admission is where that intent meets current evidence: with no retryable
/// target left, the request is refused and the excluded row is named, rather
/// than settling as a silent no-op the operator cannot act on.
#[tokio::test]
async fn accepted_operator_command_transaction_retry_without_evidence_is_rejected() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);
    harness.core.set(OperatorMode::Error);
    let before = harness.axes(&["c1"]).await;

    let result = harness.apply(OperatorIntent::Start).await;

    let Err(RunControlError::NoEligibleTarget {
        command: RunCommandKind::Start,
        ref detail,
    }) = result.outcome
    else {
        panic!("an idle marked row carries no retryable evidence: {result:?}");
    };
    assert!(
        detail.contains("c1"),
        "the excluded target must be named: {detail}"
    );
    assert_eq!(result.revision, None);
    assert_eq!(harness.axes(&["c1"]).await, before);
}

// ============================================================================
// Core mode
// ============================================================================

/// Authoritative lifecycle events transition the same Core mode, so natural
/// completion readmits a later Start instead of wedging admission.
#[tokio::test]
async fn accepted_operator_command_transaction_lifecycle_events_keep_core_admissible() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);

    harness.apply(OperatorIntent::Start).await;
    assert_eq!(harness.core.get(), OperatorMode::Running);

    harness.dispatcher.dispatch(ExecutionEvent::Stopping).await;
    assert_eq!(harness.core.get(), OperatorMode::Stopping);

    harness.dispatcher.dispatch(ExecutionEvent::Stopped).await;
    assert_eq!(harness.core.get(), OperatorMode::Stopped);

    // A late completion must not overwrite a retained terminal mode.
    harness
        .dispatcher
        .dispatch(ExecutionEvent::AllCompleted)
        .await;
    assert_eq!(
        harness.core.get(),
        OperatorMode::Stopped,
        "AllCompleted must not overwrite an operator stop"
    );

    // Natural completion from a live run returns to Select and readmits Start.
    harness.core.set(OperatorMode::Running);
    harness
        .dispatcher
        .dispatch(ExecutionEvent::AllCompleted)
        .await;
    assert_eq!(harness.core.get(), OperatorMode::Select);

    harness.marks.set("c1", true);
    let restart = harness.apply(OperatorIntent::Start).await;
    assert!(
        matches!(
            restart.outcome,
            Ok(ApplicationOutcome::Run(
                RunControlOutcome::RunDispatched { .. }
            ))
        ),
        "natural completion must readmit a later Start without frontend repair: {restart:?}"
    );

    harness
        .dispatcher
        .dispatch(ExecutionEvent::Error {
            message: "fatal".to_string(),
        })
        .await;
    assert_eq!(harness.core.get(), OperatorMode::Error);
    harness
        .dispatcher
        .dispatch(ExecutionEvent::AllCompleted)
        .await;
    assert_eq!(
        harness.core.get(),
        OperatorMode::Error,
        "AllCompleted must not overwrite a fatal error"
    );
}

/// Reconstruction discards every process-local coordination value.
///
/// The constitutional property: nothing here may become durable workflow
/// authority, so a new process starts in `Select` with no marks and no
/// reservations no matter what the previous one was doing.
#[tokio::test]
async fn accepted_operator_command_transaction_reconstruction_starts_fresh() {
    let first = Harness::new(&["c1"]);
    first.marks.set("c1", true);
    first.apply(OperatorIntent::Start).await;
    assert_eq!(first.core.get(), OperatorMode::Running);

    let reconstructed = Harness::new(&["c1"]);
    assert_eq!(reconstructed.core.get(), OperatorMode::Select);
    assert!(reconstructed.marks.marked_ids().is_empty());
    assert_eq!(reconstructed.resolves.active(), None);
    assert!(reconstructed.resolves.waiting().is_empty());
    assert!(reconstructed.timeline.events().is_empty());
}

// ============================================================================
// Scheduler ordering
// ============================================================================

/// Scheduler progress enabled by a command cannot precede that command's own
/// accepted outcome.
///
/// The recording scheduler publishes its first progress event *at activation*,
/// which is the earliest moment a real one could. If activation happened inside
/// the transaction, `ProcessingStarted` would land first and a frontend would
/// see the run before the Start that caused it.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_outcome_precedes_progress() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);

    harness.apply(OperatorIntent::Start).await;

    assert_eq!(
        harness.timeline.entries(),
        vec![
            "OperatorCommandApplied".to_string(),
            SCHEDULER_ACTIVATED.to_string()
        ],
        "the accepted command effect must be published before the scheduler that \
         command enabled is allowed to exist"
    );
}

/// A live scheduler is woken rather than started a second time.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_live_scheduler_is_woken() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);
    harness.scheduler.set_running(true);

    let result = harness.apply(OperatorIntent::Start).await;

    assert!(
        matches!(
            &result.outcome,
            Ok(ApplicationOutcome::Run(RunControlOutcome::RunDispatched {
                scheduler: SchedulerEffect::Notified,
                ..
            }))
        ),
        "a live scheduler is woken, not restarted: {result:?}"
    );
    assert_eq!(harness.scheduler.calls(), vec![SchedulerCall::Notified]);
}

/// A preparation failure leaves every axis exactly as it was.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_preparation_failure_rolls_back() {
    let harness = Harness::new(&["c1", "c2"]);
    harness.marks.set("c1", true);
    let before = harness.axes(&["c1", "c2"]).await;
    harness.scheduler.fail_launch("runtime refused the launch");

    let result = harness.apply(OperatorIntent::Start).await;

    assert!(
        matches!(result.outcome, Err(RunControlError::DispatchFailed { .. })),
        "a runtime that refuses the launch must fail the command: {result:?}"
    );
    assert_eq!(
        result.revision, None,
        "a failure with no effect settles at the unchanged admitted revision"
    );
    assert_eq!(
        harness.axes(&["c1", "c2"]).await,
        before,
        "reducer status, marks, queue, resolves, mode, scheduler calls, and events \
         must all equal their pre-command values"
    );
}

/// A resolve whose reducer refuses the intent rolls its reserved launch back.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_refused_resolve_rolls_back_its_launch() {
    let harness = Harness::new(&["c1"]);
    // No merge wait was arranged, so the target is ineligible before anything is
    // reserved.
    let before = harness.axes(&["c1"]).await;

    let result = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c1".to_string(),
        })
        .await;

    assert!(
        matches!(
            result.outcome,
            Err(RunControlError::TargetIneligible {
                command: RunCommandKind::Resolve,
                ..
            })
        ),
        "a change that is not merge-waiting cannot reserve the resolver: {result:?}"
    );
    assert_eq!(harness.axes(&["c1"]).await, before);
}

/// Activation happens *inside* the gate, after the outcome dispatch.
///
/// Both halves matter and neither implies the other. Activating after the
/// dispatch is what keeps scheduler progress from preceding the command that
/// enabled it; activating before the release is what makes "admitted" and
/// "spawned" one indivisible interval, so a force stop cannot be admitted
/// against a run whose permit has not spawned yet, `cancel_run` nothing, and
/// leave the run to spawn immediately afterwards.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_activation_runs_under_the_gate() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);

    let gate_held_at_activation = Arc::new(AtomicBool::new(false));
    {
        let gate = harness.application.gate();
        let timeline = harness.timeline.clone();
        let observed = gate_held_at_activation.clone();
        harness.scheduler.on_activate(Arc::new(move |_, _| {
            // The transaction gate is the only thing that could be holding this
            // lock at activation time: nothing else in this test takes it.
            observed.store(gate.try_lock().is_err(), Ordering::SeqCst);
            timeline.push_public(SCHEDULER_ACTIVATED);
        }));
    }

    harness.apply(OperatorIntent::Start).await;

    assert!(
        gate_held_at_activation.load(Ordering::SeqCst),
        "the prepared permit must be activated while the application gate is \
         still held, so no command is admitted between the accepted run \
         decision and the spawn it authorized"
    );
    assert_eq!(
        harness.timeline.entries(),
        vec![
            "OperatorCommandApplied".to_string(),
            SCHEDULER_ACTIVATED.to_string()
        ],
        "activation is inside the gate but still strictly after the outcome"
    );
}

/// An immediate terminal completion cannot reorder itself before the outcome.
#[tokio::test]
async fn accepted_operator_command_scheduler_order_immediate_completion_follows_the_outcome() {
    let harness = Harness::new(&["c1"]);
    harness.marks.set("c1", true);

    let result = harness.apply(OperatorIntent::Start).await;
    // The completion a just-activated scheduler would publish immediately.
    harness
        .dispatcher
        .dispatch(ExecutionEvent::AllCompleted)
        .await;

    assert!(
        matches!(
            result.outcome,
            Ok(ApplicationOutcome::Run(
                RunControlOutcome::RunDispatched { .. }
            ))
        ),
        "the command still settles as dispatched: {result:?}"
    );
    assert_eq!(
        harness.timeline.entries(),
        vec![
            "OperatorCommandApplied".to_string(),
            SCHEDULER_ACTIVATED.to_string(),
            "AllCompleted".to_string()
        ],
        "even an instant completion is ordered after the accepted command effect"
    );
    assert_eq!(
        harness.core.get(),
        OperatorMode::Select,
        "the completion is still authoritative for Core mode once it arrives"
    );
}

// ============================================================================
// The two-phase transaction
// ============================================================================

/// A queue whose cancellation is issued but never confirmed.
#[derive(Default)]
struct StuckQueue {
    cancellations: Mutex<Vec<String>>,
}

impl StuckQueue {
    fn cancellations(&self) -> Vec<String> {
        self.cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait]
impl QueuePort for StuckQueue {
    async fn add(&self, _change_id: &str) -> bool {
        true
    }

    async fn remove(&self, _change_id: &str) -> bool {
        true
    }

    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        self.cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(change_id.to_string());
        Ok(Some(TerminationWaiter::never()))
    }

    async fn notify_scheduler(&self) {}
}

/// A projection double that reports whether the gate was held when it was read.
///
/// The contract under test is *where* a settlement revision is sampled, not
/// which number comes back, so the double answers that question directly
/// instead of the test trying to win a timing race against the coordinator.
struct GateWatchingRevisions {
    gate: Arc<tokio::sync::Mutex<()>>,
    revision: AtomicU64,
    gate_held_at_read: AtomicBool,
    reads: AtomicU64,
}

impl GateWatchingRevisions {
    fn new(gate: Arc<tokio::sync::Mutex<()>>) -> Self {
        Self {
            gate,
            revision: AtomicU64::new(0),
            gate_held_at_read: AtomicBool::new(false),
            reads: AtomicU64::new(0),
        }
    }

    fn set(&self, revision: u64) {
        self.revision.store(revision, Ordering::SeqCst);
    }
}

impl OutcomeRevisions for GateWatchingRevisions {
    fn revision_for_dispatch(&self, _dispatch_id: u64) -> Option<u64> {
        None
    }

    fn current_revision(&self) -> u64 {
        self.gate_held_at_read
            .store(self.gate.try_lock().is_err(), Ordering::SeqCst);
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.revision.load(Ordering::SeqCst)
    }
}

/// A timed-out stop-and-dequeue reads its settlement revision under the
/// reacquired gate, and commits nothing.
///
/// The delta permits no command to sample mutable global state after releasing
/// its boundary, and the timeout path is the one that leaves the boundary and
/// then still has to settle. Nothing else in this test holds the gate, so an
/// unlockable gate at read time can only mean the settlement is holding it.
#[tokio::test]
async fn accepted_operator_command_dequeue_liveness_timeout_settles_under_the_gate() {
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["c1".to_string()],
        10,
    )));
    let marks = Arc::new(ExecutionMarkStore::new());
    let parallel = Arc::new(ParallelRuntime::new());
    let queue = Arc::new(StuckQueue::default());
    let scheduler = Arc::new(RecordingScheduler::new());
    let timeline = Arc::new(Timeline::default());
    let operator = Arc::new(
        OperatorCommandService::new(
            state.clone(),
            queue.clone(),
            Arc::new(NoopQueueHooks),
            marks.clone(),
        )
        .with_parallel(parallel.clone())
        // A safety valve for the never-confirmed waiter, not a performance
        // assertion: the settlement's ordering is proven by the gate
        // observation below, never by how long this took.
        .with_cancellation_timeout(Duration::from_millis(20)),
    );
    let run_control = Arc::new(RunControlService::new(
        state.clone(),
        operator,
        scheduler,
        Arc::new(ResolveReservations::new()),
        parallel,
    ));
    let dispatcher = Arc::new(crate::events::EventDispatcher::new(
        state.clone(),
        vec![Arc::new(RecordingSink {
            timeline: timeline.clone(),
        })],
    ));
    let application = OperatorApplication::new(Arc::new(CoreMode::new()), run_control, dispatcher);
    let revisions = Arc::new(GateWatchingRevisions::new(application.gate()));
    let application = application.with_revisions(Some(revisions.clone()));

    let pending = application
        .begin_stop_and_dequeue("c1")
        .await
        .expect("phase one issues cancellation for a registered handle");
    // Unrelated progress during the wait: the settlement must report *this*,
    // read under the reacquired boundary, not the revision it was admitted at.
    revisions.set(41);

    let result = application.settle_stop_and_dequeue(pending).await;

    assert!(
        matches!(
            result.outcome,
            Err(RunControlError::Operator(
                OperatorCommandError::TerminationTimeout { .. }
            ))
        ),
        "an unconfirmed termination must fail rather than certify a dequeue: {result:?}"
    );
    assert!(
        revisions.gate_held_at_read.load(Ordering::SeqCst),
        "the settlement revision must be read under the reacquired application \
         gate, not sampled from global state after the boundary was released"
    );
    assert_eq!(
        result.revision,
        Some(41),
        "the explicit unchanged revision observed at settlement is what the \
         record settles with"
    );
    assert_eq!(
        queue.cancellations(),
        vec!["c1".to_string()],
        "the already-issued cancellation is neither repeated nor rolled back"
    );
    assert!(
        timeline.entries().is_empty(),
        "a timed-out dequeue publishes no outcome event: {:?}",
        timeline.entries()
    );
    assert_eq!(
        state.read().await.display_status("c1"),
        "not queued",
        "a timed-out dequeue commits no reducer mutation"
    );
}
