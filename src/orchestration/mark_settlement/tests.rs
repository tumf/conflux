//! Mark-stability regressions for the live-scheduler settlement policy.
//!
//! Unit-scoped by construction. The reducer is a plain `OrchestratorState`, the
//! scheduler is the shared recording double, and the queue is an in-memory port
//! that records every call it receives. No process, repository, network, or
//! wall-clock threshold is involved: every test runs under paused Tokio time and
//! waits on the coordinator's completed-pass transition rather than on a delay.
//!
//! Naming: every test in this file is `running_mark_reanalysis_*` so the focused
//! `make test-running-mark-reanalysis` target selects exactly this contract.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::*;
use crate::events::{EventSink, ExecutionEvent};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, ParallelEligibility,
    ParallelRuntime, QueuePort, TerminationWaiter,
};
use crate::orchestration::operator_coordinator::{
    bind_mark_settlement, CoreMode, OperatorApplication, OperatorIntent,
};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::{OrchestratorState, ReduceOutcome, ReducerCommand};

// ============================================================================
// Doubles
// ============================================================================

/// Every runtime call settlement could possibly make, in order.
///
/// One log rather than several counters: "settlement never emits cancellation"
/// is then a property of the whole recorded sequence instead of a set of
/// assertions that can silently miss the one call nobody thought to count.
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueueCall {
    Add(String),
    Remove(String),
    Cancel(String),
    Notify,
    ExplicitRetry(String),
}

#[derive(Debug, Default)]
struct RecordingQueue {
    entries: Mutex<Vec<String>>,
    calls: Mutex<Vec<QueueCall>>,
}

impl RecordingQueue {
    fn calls(&self) -> Vec<QueueCall> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, call: QueueCall) {
        self.calls.lock().unwrap().push(call);
    }

    fn added(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                QueueCall::Add(id) => Some(id),
                _ => None,
            })
            .collect()
    }
}

#[async_trait]
impl QueuePort for RecordingQueue {
    async fn add(&self, change_id: &str) -> bool {
        self.record(QueueCall::Add(change_id.to_string()));
        let mut guard = self.entries.lock().unwrap();
        if guard.iter().any(|id| id == change_id) {
            return false;
        }
        guard.push(change_id.to_string());
        true
    }

    async fn remove(&self, change_id: &str) -> bool {
        self.record(QueueCall::Remove(change_id.to_string()));
        let mut guard = self.entries.lock().unwrap();
        let before = guard.len();
        guard.retain(|id| id != change_id);
        guard.len() != before
    }

    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        self.record(QueueCall::Cancel(change_id.to_string()));
        Ok(None)
    }

    async fn notify_scheduler(&self) {
        self.record(QueueCall::Notify);
    }

    async fn publish_explicit_retry(&self, change_id: &str) {
        self.record(QueueCall::ExplicitRetry(change_id.to_string()));
    }
}

/// Sink that records the authoritative dispatch order.
#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<ExecutionEvent>>,
}

#[async_trait]
impl EventSink for RecordingSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        self.events.lock().unwrap().push(event.clone());
    }

    async fn on_state_changed(&self, _state: &OrchestratorState) {}
}

/// A settlement runtime with no application behind it.
///
/// Used where the property under test is the *timer* — arming, reset, final
/// snapshot, abandonment — so it can be observed without a reducer or a queue at
/// all.
#[derive(Default)]
struct ScriptedRuntime {
    live: std::sync::atomic::AtomicBool,
    settlements: Mutex<Vec<Vec<String>>>,
    abandonments: Mutex<Vec<Vec<String>>>,
    marks: Mutex<Vec<String>>,
}

impl ScriptedRuntime {
    fn live(marks: &[&str]) -> Self {
        let runtime = Self::default();
        runtime
            .live
            .store(true, std::sync::atomic::Ordering::SeqCst);
        *runtime.marks.lock().unwrap() = marks.iter().map(|id| (*id).to_string()).collect();
        runtime
    }

    fn set_live(&self, live: bool) {
        self.live.store(live, std::sync::atomic::Ordering::SeqCst);
    }

    fn set_marks(&self, marks: &[&str]) {
        *self.marks.lock().unwrap() = marks.iter().map(|id| (*id).to_string()).collect();
    }

    fn settlements(&self) -> Vec<Vec<String>> {
        self.settlements.lock().unwrap().clone()
    }

    fn abandonments(&self) -> Vec<Vec<String>> {
        self.abandonments.lock().unwrap().clone()
    }
}

#[async_trait]
impl MarkSettlementRuntime for ScriptedRuntime {
    fn admits_dynamic_queue(&self) -> bool {
        self.live.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn settle_marks(&self) -> MarkSettlementPlan {
        // Reads the marks that exist *now*, exactly as the production runtime
        // does, so a revocation during the deadline is visible in the pass.
        let additions = self.marks.lock().unwrap().clone();
        self.settlements.lock().unwrap().push(additions.clone());
        MarkSettlementPlan {
            additions,
            excluded: Vec::new(),
        }
    }

    async fn report_abandoned_settlement(&self, pending: Vec<String>) {
        self.abandonments.lock().unwrap().push(pending);
    }
}

// ============================================================================
// Harness
// ============================================================================

/// A coordinator wired to a scripted runtime, with nothing else in the process.
struct TimerHarness {
    coordinator: Arc<MarkSettlementCoordinator>,
    runtime: Arc<ScriptedRuntime>,
}

impl TimerHarness {
    fn new(runtime: ScriptedRuntime) -> Self {
        let runtime = Arc::new(runtime);
        let coordinator = Arc::new(MarkSettlementCoordinator::new());
        let weak: std::sync::Weak<ScriptedRuntime> = Arc::downgrade(&runtime);
        coordinator.bind_runtime(weak);
        Self {
            coordinator,
            runtime,
        }
    }

    fn arm(&self, snapshot: &[&str]) -> bool {
        self.coordinator
            .notify(snapshot.iter().map(|id| (*id).to_string()).collect())
    }
}

/// The production wiring, with only the scheduler and the frontend replaced.
struct Harness {
    state: Arc<RwLock<OrchestratorState>>,
    marks: Arc<ExecutionMarkStore>,
    parallel: Arc<ParallelRuntime>,
    scheduler: Arc<RecordingScheduler>,
    queue: Arc<RecordingQueue>,
    sink: Arc<RecordingSink>,
    operator: Arc<OperatorCommandService>,
    application: Arc<OperatorApplication>,
}

impl Harness {
    fn new(change_ids: &[&str]) -> Self {
        let state = Arc::new(RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        let queue = Arc::new(RecordingQueue::default());
        let sink = Arc::new(RecordingSink::default());
        let operator = Arc::new(
            OperatorCommandService::new(
                state.clone(),
                queue.clone(),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone())
            .with_run_boundary(scheduler.clone()),
        );
        let run_control = Arc::new(RunControlService::new(
            state.clone(),
            operator.clone(),
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel.clone(),
        ));
        let core = Arc::new(CoreMode::new());
        let dispatcher = Arc::new(
            crate::events::EventDispatcher::new(state.clone(), vec![sink.clone()])
                .with_core_mode(Some(core.clone())),
        );
        let application = Arc::new(OperatorApplication::new(core, run_control, dispatcher));
        bind_mark_settlement(&application);
        Self {
            state,
            marks,
            parallel,
            scheduler,
            queue,
            sink,
            operator,
            application,
        }
    }

    /// A harness whose scheduler is already live and admitting dynamic work.
    fn running(change_ids: &[&str]) -> Self {
        let harness = Self::new(change_ids);
        harness.scheduler.set_running(true);
        harness
    }

    fn coordinator(&self) -> Arc<MarkSettlementCoordinator> {
        self.marks.settlement()
    }

    async fn status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    async fn reduce(&self, command: ReducerCommand) -> ReduceOutcome {
        self.state.write().await.apply_command(command)
    }

    fn events(&self) -> Vec<ExecutionEvent> {
        self.sink.events.lock().unwrap().clone()
    }

    fn logs(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                ExecutionEvent::Log(entry) => Some(entry.message),
                _ => None,
            })
            .collect()
    }
}

/// Advance paused time past the stability deadline and wait for the pass.
///
/// The wait is on the coordinator's completed-pass transition, not on a
/// duration: a settlement task that never ran would hang the test rather than
/// pass it, which is the point.
async fn settle(coordinator: &Arc<MarkSettlementCoordinator>) {
    let mut passes = coordinator.passes();
    let target = *passes.borrow() + 1;
    tokio::time::advance(MARK_STABILITY_WINDOW + std::time::Duration::from_millis(1)).await;
    passes
        .wait_for(|observed| *observed >= target)
        .await
        .expect("the coordinator outlives its watchers");
}

/// Advance paused time to just *before* the deadline.
async fn advance_within_window() {
    tokio::time::advance(MARK_STABILITY_WINDOW - std::time::Duration::from_millis(1)).await;
}

// ============================================================================
// Classification (pure)
// ============================================================================

fn row<'a>(change_id: &'a str, display_status: &'a str) -> MarkSettlementRow<'a> {
    MarkSettlementRow {
        change_id,
        display_status,
        tracked: true,
        parallel_eligible: true,
    }
}

#[test]
fn running_mark_reanalysis_plans_only_ordinary_not_queued_rows() {
    let plan = plan_mark_settlement(&[row("fresh", "not queued")]);
    assert_eq!(plan.additions, vec!["fresh".to_string()]);
    assert!(plan.excluded.is_empty());
}

#[test]
fn running_mark_reanalysis_excludes_every_ineligible_status() {
    // One table rather than one test per word: a new display status that nobody
    // classified must show up as a missing row here, not as a silent admission.
    let cases: &[(&str, MarkSettlementExclusion)] = &[
        ("queued", MarkSettlementExclusion::AlreadyQueued),
        ("preparing", MarkSettlementExclusion::Active),
        ("applying", MarkSettlementExclusion::Active),
        ("accepting", MarkSettlementExclusion::Active),
        ("rejecting", MarkSettlementExclusion::Active),
        ("archiving", MarkSettlementExclusion::Active),
        ("resolving", MarkSettlementExclusion::Active),
        ("merge wait", MarkSettlementExclusion::Waiting),
        ("resolve pending", MarkSettlementExclusion::Waiting),
        ("reject pending", MarkSettlementExclusion::Waiting),
        ("blocked", MarkSettlementExclusion::Waiting),
        ("stalled", MarkSettlementExclusion::Waiting),
        ("error", MarkSettlementExclusion::Terminal),
        ("stopped", MarkSettlementExclusion::Terminal),
        ("archived", MarkSettlementExclusion::Terminal),
        ("merged", MarkSettlementExclusion::Terminal),
        ("pushed", MarkSettlementExclusion::Terminal),
        ("rejected", MarkSettlementExclusion::Terminal),
    ];
    for (display_status, expected) in cases {
        let plan = plan_mark_settlement(&[row("target", display_status)]);
        assert!(
            plan.additions.is_empty(),
            "'{display_status}' must never become a queue addition"
        );
        assert_eq!(
            plan.excluded,
            vec![("target".to_string(), *expected)],
            "'{display_status}' reported the wrong exclusion"
        );
    }
}

#[test]
fn running_mark_reanalysis_excludes_untracked_and_ineligible_rows() {
    let plan = plan_mark_settlement(&[
        MarkSettlementRow {
            change_id: "unknown",
            display_status: "not queued",
            tracked: false,
            parallel_eligible: true,
        },
        MarkSettlementRow {
            change_id: "uncommitted",
            display_status: "not queued",
            tracked: true,
            parallel_eligible: false,
        },
    ]);
    assert!(plan.additions.is_empty());
    assert_eq!(
        plan.excluded,
        vec![
            ("unknown".to_string(), MarkSettlementExclusion::NotLoadable),
            (
                "uncommitted".to_string(),
                MarkSettlementExclusion::Unavailable
            ),
        ]
    );
}

#[test]
fn running_mark_reanalysis_unrecognised_status_fails_closed() {
    // A display status this classifier has never seen is not evidence that the
    // row may be admitted, so it must not become an addition by default.
    let plan = plan_mark_settlement(&[row("future", "some future word")]);
    assert!(plan.additions.is_empty());
    assert_eq!(
        plan.excluded,
        vec![("future".to_string(), MarkSettlementExclusion::Waiting)]
    );
}

// ============================================================================
// Deadline behaviour
// ============================================================================

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_does_not_settle_before_the_window_elapses() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    assert!(harness.arm(&["alpha"]));

    advance_within_window().await;

    assert!(
        harness.runtime.settlements().is_empty(),
        "a mark must not mutate queue intent during the stability window"
    );
    assert!(harness.coordinator.is_armed());
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_settles_once_after_the_window() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    harness.arm(&["alpha"]);

    settle(&harness.coordinator).await;

    assert_eq!(
        harness.runtime.settlements(),
        vec![vec!["alpha".to_string()]]
    );
    assert_eq!(harness.coordinator.settled_count(), 1);
    assert!(!harness.coordinator.is_armed());
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_later_operator_mark_restarts_the_deadline() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    harness.arm(&["alpha"]);
    advance_within_window().await;

    // A second accepted operator write before the first deadline expires.
    harness.runtime.set_marks(&["alpha", "beta"]);
    harness.arm(&["alpha", "beta"]);
    assert_eq!(
        harness.coordinator.pending_snapshot(),
        Some(vec!["alpha".to_string(), "beta".to_string()]),
        "the pending snapshot must be replaced, not accumulated"
    );

    advance_within_window().await;
    assert!(
        harness.runtime.settlements().is_empty(),
        "the superseded deadline must not fire on the original schedule"
    );

    settle(&harness.coordinator).await;
    assert_eq!(
        harness.runtime.settlements(),
        vec![vec!["alpha".to_string(), "beta".to_string()]],
        "exactly one settlement runs, over the final snapshot"
    );
    assert_eq!(harness.coordinator.settled_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_system_revocation_does_not_restart_the_deadline() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha", "beta"]));
    harness.arm(&["alpha", "beta"]);
    advance_within_window().await;

    // Lifecycle reconciliation revokes `beta` by writing the store directly. It
    // never calls `notify`, so the deadline armed by the operator stands.
    harness.runtime.set_marks(&["alpha"]);

    settle(&harness.coordinator).await;
    assert_eq!(
        harness.runtime.settlements(),
        vec![vec!["alpha".to_string()]],
        "settlement classifies the current mark set, not the armed snapshot"
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_without_a_live_scheduler_is_mark_only() {
    let runtime = ScriptedRuntime::live(&["alpha"]);
    runtime.set_live(false);
    let harness = TimerHarness::new(runtime);

    assert!(
        !harness.arm(&["alpha"]),
        "a process with no live dynamic-queue scheduler must not arm settlement"
    );
    assert!(!harness.coordinator.is_armed());

    settle_expect_no_pass(&harness.coordinator).await;
    assert!(harness.runtime.settlements().is_empty());
    assert!(harness.runtime.abandonments().is_empty());
}

/// Advance past the window and assert no pass ran at all.
///
/// Deliberately not the `settle` helper: here the claim is that *nothing* was
/// scheduled, so waiting on a pass that must never happen would hang.
async fn settle_expect_no_pass(coordinator: &Arc<MarkSettlementCoordinator>) {
    let before = *coordinator.passes().borrow();
    tokio::time::advance(MARK_STABILITY_WINDOW * 2).await;
    tokio::task::yield_now().await;
    assert_eq!(
        *coordinator.passes().borrow(),
        before,
        "no settlement pass may run without a live scheduler"
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_finite_run_discards_its_unsettled_snapshot() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    harness.arm(&["alpha"]);
    advance_within_window().await;

    // The finite scheduler exits before the deadline expires.
    harness.runtime.set_live(false);

    settle(&harness.coordinator).await;

    assert!(
        harness.runtime.settlements().is_empty(),
        "an ended scheduler must produce no queue addition"
    );
    assert_eq!(
        harness.runtime.abandonments(),
        vec![vec!["alpha".to_string()]],
        "abandonment is reported exactly once, naming the discarded snapshot"
    );
    assert_eq!(harness.coordinator.abandoned_count(), 1);
    assert!(!harness.coordinator.is_armed());
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_persistent_idle_scheduler_still_settles() {
    // A persistent scheduler parked in Select is still live, so the deadline it
    // was armed under remains valid: presentation mode is never the authority.
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    harness.arm(&["alpha"]);
    advance_within_window().await;

    settle(&harness.coordinator).await;
    assert_eq!(
        harness.runtime.settlements(),
        vec![vec!["alpha".to_string()]]
    );
}

#[test]
fn running_mark_reanalysis_restart_begins_with_empty_stability_state() {
    let coordinator = MarkSettlementCoordinator::new();
    assert!(!coordinator.is_armed());
    assert_eq!(coordinator.pending_snapshot(), None);
    assert_eq!(coordinator.settled_count(), 0);
    assert_eq!(coordinator.abandoned_count(), 0);
    assert_eq!(coordinator.last_plan(), None);
    assert_eq!(coordinator.window(), MARK_STABILITY_WINDOW);
}

// ============================================================================
// Service entry points
// ============================================================================

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_tui_mark_entry_point_arms_settlement() {
    let harness = Harness::running(&["alpha"]);

    // The TUI Space/bulk-`x` write path.
    assert!(harness.operator.apply_execution_mark("alpha", true).await);
    assert!(harness.coordinator().is_armed());

    settle(&harness.coordinator()).await;
    assert_eq!(harness.queue.added(), vec!["alpha".to_string()]);
    assert_eq!(harness.status("alpha").await, "queued");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_api_mark_entry_point_arms_settlement() {
    let harness = Harness::running(&["alpha"]);

    // The `/api/v2` single-target command path.
    harness
        .operator
        .set_execution_mark("alpha", true)
        .await
        .expect("a fresh row accepts a mark");
    assert!(harness.coordinator().is_armed());

    settle(&harness.coordinator()).await;
    assert_eq!(harness.queue.added(), vec!["alpha".to_string()]);
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_bulk_mark_entry_point_arms_one_settlement() {
    let harness = Harness::running(&["alpha", "beta"]);

    harness
        .operator
        .set_all_execution_marks()
        .await
        .expect("visible rows accept a bulk mark");
    assert_eq!(
        harness.coordinator().pending_snapshot(),
        Some(vec!["alpha".to_string(), "beta".to_string()]),
        "one deadline describes the whole bulk mark set"
    );

    settle(&harness.coordinator()).await;
    assert_eq!(
        harness.queue.added(),
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert_eq!(harness.coordinator().settled_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_unchanged_mark_command_does_not_arm() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;

    // A repeat of the same value is a no-op: nothing changed, so nothing arms.
    assert!(!harness.operator.apply_execution_mark("alpha", true).await);
    assert!(!harness.coordinator().is_armed());
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_start_admission_marks_do_not_arm() {
    let harness = Harness::running(&["alpha"]);

    // The write Start admission performs for an explicitly selected target.
    assert!(
        harness
            .operator
            .apply_admission_execution_mark("alpha", true)
            .await
    );
    assert!(
        !harness.coordinator().is_armed(),
        "a Start-admission mark write must never arm delayed settlement"
    );

    settle_expect_no_pass(&harness.coordinator()).await;
    assert!(
        harness.queue.calls().is_empty(),
        "a rejected Start must leave no delayed queue effect: {:?}",
        harness.queue.calls()
    );
}

// ============================================================================
// Settlement through the shared queue command path
// ============================================================================

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_addition_produces_one_reducer_and_queue_mutation() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;

    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.calls(),
        vec![QueueCall::Add("alpha".to_string()), QueueCall::Notify,],
        "one dynamic mutation and one scheduler wake, and nothing else"
    );
    assert_eq!(harness.status("alpha").await, "queued");

    let queue_deltas: Vec<_> = harness
        .events()
        .into_iter()
        .filter(|event| {
            matches!(
                event,
                ExecutionEvent::OperatorCommandApplied {
                    effect: crate::events::OperatorCommandEffect::QueueDelta { .. },
                }
            )
        })
        .collect();
    assert_eq!(
        queue_deltas.len(),
        1,
        "exactly one authoritative outcome describes the addition"
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_duplicate_addition_is_a_no_op() {
    let harness = Harness::running(&["alpha", "beta"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;
    assert_eq!(harness.queue.added(), vec!["alpha".to_string()]);

    // `alpha` is still marked, so the next pass re-classifies a row it already
    // queued. Marking `beta` gives that pass genuine work, which is what makes
    // "the duplicate produced nothing" distinguishable from "the pass did
    // nothing at all".
    harness.operator.apply_execution_mark("beta", true).await;
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.added(),
        vec!["alpha".to_string(), "beta".to_string()],
        "an already-queued marked row must produce no second addition"
    );
    let plan = harness.coordinator().last_plan().expect("a pass ran");
    assert_eq!(plan.additions, vec!["beta".to_string()]);
    assert_eq!(
        plan.excluded,
        vec![("alpha".to_string(), MarkSettlementExclusion::AlreadyQueued)]
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_empty_plan_produces_no_mutation() {
    let harness = Harness::running(&["alpha"]);
    // Marked, but worktree execution refuses the row.
    harness.parallel.set_parallel_ineligible([(
        "alpha".to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);
    harness.operator.apply_execution_mark("alpha", true).await;

    settle(&harness.coordinator()).await;

    assert!(
        harness.queue.calls().is_empty(),
        "an empty plan touches no runtime at all: {:?}",
        harness.queue.calls()
    );
    assert_eq!(harness.status("alpha").await, "not queued");
    assert_eq!(
        harness
            .coordinator()
            .last_plan()
            .expect("a pass ran")
            .excluded,
        vec![("alpha".to_string(), MarkSettlementExclusion::Unavailable)]
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_unmark_never_dequeues_admitted_work() {
    let harness = Harness::running(&["alpha"]);
    harness
        .reduce(ReducerCommand::AddToQueue("alpha".to_string()))
        .await;
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;
    let after_mark = harness.queue.calls();

    // Unmarking is mark intent only. It arms settlement like any other accepted
    // standalone write, and settlement is additive-only.
    assert!(harness.operator.apply_execution_mark("alpha", false).await);
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.calls(),
        after_mark,
        "unmarking must emit no remove, cancel, retry, or stop: {:?}",
        harness.queue.calls()
    );
    assert_eq!(harness.status("alpha").await, "queued");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_terminal_error_row_never_reaches_the_queue_service() {
    let harness = Harness::running(&["alpha"]);
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        });
    assert_eq!(harness.status("alpha").await, "error");

    // Marking an error row is allowed; admitting it is not. The queue service's
    // terminal-error branch is an explicit retry, which no mark ever requested.
    harness.marks.set("alpha", true);
    harness.operator.apply_execution_mark("beta", true).await;

    settle(&harness.coordinator()).await;

    assert!(
        !harness
            .queue
            .calls()
            .iter()
            .any(|call| matches!(call, QueueCall::ExplicitRetry(_))),
        "settlement must never publish an explicit-retry edge: {:?}",
        harness.queue.calls()
    );
    assert_eq!(harness.status("alpha").await, "error");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_lifecycle_transition_racing_settlement_is_respected() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    advance_within_window().await;

    // The change starts executing while the deadline is still pending. Settlement
    // classifies the state it finds at expiry, not the one it was armed under.
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "echo".to_string(),
        });
    assert_eq!(harness.status("alpha").await, "applying");

    settle(&harness.coordinator()).await;

    assert!(
        harness.queue.calls().is_empty(),
        "an active row must gain no queue effect: {:?}",
        harness.queue.calls()
    );
    assert_eq!(
        harness
            .coordinator()
            .last_plan()
            .expect("a pass ran")
            .excluded,
        vec![("alpha".to_string(), MarkSettlementExclusion::Active)]
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_abandonment_reports_one_operator_visible_outcome() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    advance_within_window().await;

    // The finite scheduler ends before the deadline expires.
    harness.scheduler.set_running(false);

    settle(&harness.coordinator()).await;

    let abandonment: Vec<_> = harness
        .logs()
        .into_iter()
        .filter(|message| message.contains("Mark settlement abandoned"))
        .collect();
    assert_eq!(
        abandonment.len(),
        1,
        "abandonment is reported exactly once: {:?}",
        harness.logs()
    );
    assert!(abandonment[0].contains("alpha"));
    assert!(harness.queue.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_settlement_matches_an_explicit_queue_command() {
    // The whole point of routing settlement through the shared command path: a
    // settled addition and an operator's explicit queue command must be
    // indistinguishable in reducer state, runtime calls, and event cardinality.
    let settled = Harness::running(&["alpha"]);
    settled.operator.apply_execution_mark("alpha", true).await;
    settle(&settled.coordinator()).await;

    let explicit = Harness::running(&["alpha"]);
    explicit
        .application
        .apply(OperatorIntent::SetQueueIntent {
            change_id: "alpha".to_string(),
            queued: true,
        })
        .await;

    assert_eq!(settled.queue.calls(), explicit.queue.calls());
    assert_eq!(
        settled.status("alpha").await,
        explicit.status("alpha").await
    );
    assert_eq!(
        settled
            .events()
            .iter()
            .filter(|event| matches!(event, ExecutionEvent::OperatorCommandApplied { .. }))
            .count(),
        explicit
            .events()
            .iter()
            .filter(|event| matches!(event, ExecutionEvent::OperatorCommandApplied { .. }))
            .count(),
    );
}
