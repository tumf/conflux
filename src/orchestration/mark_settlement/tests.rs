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
    ExecutionMarkStore, OperatorCommandService, ParallelEligibility, ParallelRuntime, QueuePort,
    TerminationWaiter,
};
use crate::orchestration::operator_coordinator::{
    bind_mark_settlement, CoreMode, OperatorApplication, OperatorIntent,
};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::{OrchestratorState, QueueIntent, ReduceOutcome, ReducerCommand};

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

/// Queue hooks that record every dispatch, in order.
#[derive(Debug, Default)]
struct RecordingHooks {
    calls: Mutex<Vec<HookCall>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookCall {
    Add(String),
    Remove(String),
}

impl RecordingHooks {
    fn calls(&self) -> Vec<HookCall> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl crate::orchestration::operator_command::QueueHookPort for RecordingHooks {
    async fn on_queue_add(&self, change_id: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(HookCall::Add(change_id.to_string()));
    }

    async fn on_queue_remove(&self, change_id: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(HookCall::Remove(change_id.to_string()));
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
    batches: Mutex<Vec<Vec<String>>>,
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

    /// The batch scope each completed pass was handed, in order.
    fn batches(&self) -> Vec<Vec<String>> {
        self.batches.lock().unwrap().clone()
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

    async fn settle_marks(&self, targets: Vec<String>) -> MarkSettlementPlan {
        self.batches.lock().unwrap().push(targets.clone());
        // Reads the marks that exist *now*, exactly as the production runtime
        // does, so a revocation during the deadline is visible in the pass — and
        // only within the batch's scope, which is what the batch is for.
        let marks = self.marks.lock().unwrap().clone();
        let mut plan = MarkSettlementPlan::default();
        for change_id in targets {
            if marks.contains(&change_id) {
                plan.additions.push(change_id);
            } else {
                plan.removals.push(change_id);
            }
        }
        self.settlements
            .lock()
            .unwrap()
            .push(plan.additions.clone());
        plan
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
    hooks: Arc<RecordingHooks>,
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
        let hooks = Arc::new(RecordingHooks::default());
        let sink = Arc::new(RecordingSink::default());
        let operator = Arc::new(
            OperatorCommandService::new(state.clone(), queue.clone(), hooks.clone(), marks.clone())
                .with_parallel(parallel.clone()),
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
            hooks,
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

    /// Authoritative reducer queue intent, independent of display status.
    ///
    /// `display_status` hides intent behind activity and wait words, so a test
    /// that asks "did the removal really not apply while the row was active"
    /// has to read the intent itself.
    async fn queue_intent(&self, change_id: &str) -> QueueIntent {
        self.state
            .read()
            .await
            .change_runtime(change_id)
            .expect("the reducer tracks the change")
            .queue_intent
            .clone()
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

/// A tracked, eligible, currently *marked* row.
fn row<'a>(change_id: &'a str, display_status: &'a str) -> MarkSettlementRow<'a> {
    MarkSettlementRow {
        change_id,
        display_status,
        tracked: true,
        parallel_eligible: true,
        marked: true,
    }
}

/// The same row with its mark withdrawn.
fn unmarked_row<'a>(change_id: &'a str, display_status: &'a str) -> MarkSettlementRow<'a> {
    MarkSettlementRow {
        marked: false,
        ..row(change_id, display_status)
    }
}

#[test]
fn running_mark_reanalysis_plans_only_ordinary_not_queued_rows() {
    let plan = plan_mark_settlement(&[row("fresh", "not queued")]);
    assert_eq!(plan.additions, vec!["fresh".to_string()]);
    assert!(plan.excluded.is_empty());
}

#[test]
fn bidirectional_mark_reconciliation_classification_unmark_removes_ordinary_pending() {
    let plan = plan_mark_settlement(&[unmarked_row("settled", "queued")]);
    assert_eq!(plan.removals, vec!["settled".to_string()]);
    assert!(plan.additions.is_empty());
    assert!(plan.excluded.is_empty());
}

#[test]
fn bidirectional_mark_reconciliation_classification_aligned_rows_are_no_ops() {
    // Both already-aligned directions are reasoned skips, not mutations: a
    // marked queued row must not be re-added and an unmarked idle row must not
    // be re-removed, or every settled batch would churn the queue.
    let plan = plan_mark_settlement(&[row("held", "queued"), unmarked_row("cold", "not queued")]);
    assert!(plan.is_empty());
    assert_eq!(
        plan.excluded,
        vec![
            ("held".to_string(), MarkSettlementExclusion::AlreadyQueued),
            (
                "cold".to_string(),
                MarkSettlementExclusion::AlreadyNotQueued
            ),
        ]
    );
}

#[test]
fn bidirectional_mark_reconciliation_classification_excludes_every_ineligible_unmarked_status() {
    // The removal direction's own table. Every lifecycle word that is not the
    // idle ordinary `queued` state must fail closed with a stable reason, so an
    // unmark can never reach active, waiting, or terminal work.
    let cases: &[(&str, MarkSettlementExclusion)] = &[
        ("not queued", MarkSettlementExclusion::AlreadyNotQueued),
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
        ("some future word", MarkSettlementExclusion::Waiting),
    ];
    for (display_status, expected) in cases {
        let plan = plan_mark_settlement(&[unmarked_row("target", display_status)]);
        assert!(
            plan.removals.is_empty(),
            "'{display_status}' must never become a queue removal"
        );
        assert_eq!(
            plan.excluded,
            vec![("target".to_string(), *expected)],
            "'{display_status}' reported the wrong exclusion"
        );
    }
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
        assert!(
            plan.removals.is_empty(),
            "'{display_status}' is marked, so it must never become a queue removal"
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
            tracked: false,
            ..row("unknown", "not queued")
        },
        MarkSettlementRow {
            parallel_eligible: false,
            ..row("uncommitted", "not queued")
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
async fn mark_settlement_changed_targets_later_write_restarts_one_accumulating_deadline() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    harness.arm(&["alpha"]);
    advance_within_window().await;

    // A second accepted operator write, naming a *different* target, before the
    // first deadline expires.
    harness.runtime.set_marks(&["alpha", "beta"]);
    harness.arm(&["beta"]);
    assert_eq!(
        harness.coordinator.pending_snapshot(),
        Some(vec!["alpha".to_string(), "beta".to_string()]),
        "the batch accumulates: restarting the deadline must not drop the write it was already waiting on"
    );

    advance_within_window().await;
    assert!(
        harness.runtime.batches().is_empty(),
        "the superseded deadline must not fire on the original schedule"
    );

    settle(&harness.coordinator).await;
    assert_eq!(
        harness.runtime.batches(),
        vec![vec!["alpha".to_string(), "beta".to_string()]],
        "exactly one settlement runs, over the whole accumulated batch"
    );
    assert_eq!(harness.coordinator.settled_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_changed_targets_repeated_target_enters_the_batch_once() {
    let harness = TimerHarness::new(ScriptedRuntime::live(&["alpha"]));
    // Marked, unmarked, marked again inside one window: three accepted writes,
    // one target to reconcile.
    harness.arm(&["alpha"]);
    harness.arm(&["alpha"]);
    harness.arm(&["alpha"]);

    settle(&harness.coordinator).await;

    assert_eq!(harness.runtime.batches(), vec![vec!["alpha".to_string()]]);
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
        harness.runtime.batches(),
        vec![vec!["alpha".to_string(), "beta".to_string()]],
        "the batch scope is what the accepted writes named"
    );
    assert_eq!(
        harness.runtime.settlements(),
        vec![vec!["alpha".to_string()]],
        "settlement classifies the marks that exist at expiry, not the ones the batch was recorded under"
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

    // `alpha` is unmarked and marked again inside one window, so the next batch
    // really does name it and the pass re-classifies a row it already queued.
    // Marking `beta` gives that pass genuine work, which is what makes "the
    // duplicate produced nothing" distinguishable from "the pass did nothing at
    // all".
    harness.operator.apply_execution_mark("alpha", false).await;
    harness.operator.apply_execution_mark("alpha", true).await;
    harness.operator.apply_execution_mark("beta", true).await;
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.added(),
        vec!["alpha".to_string(), "beta".to_string()],
        "an already-queued marked row must produce no second addition"
    );
    let plan = harness.coordinator().last_plan().expect("a pass ran");
    assert_eq!(plan.additions, vec!["beta".to_string()]);
    assert!(plan.removals.is_empty());
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
async fn mark_settlement_active_unmark_is_mark_only() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;

    // The change is dispatched and running by the time the operator unmarks it.
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "echo".to_string(),
        });
    assert_eq!(harness.status("alpha").await, "applying");
    let after_mark = harness.queue.calls();

    // Unmarking active work changes next-run selection and nothing else.
    assert!(harness.operator.apply_execution_mark("alpha", false).await);
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.calls(),
        after_mark,
        "unmarking active work must emit no remove, cancel, retry, or stop: {:?}",
        harness.queue.calls()
    );
    assert_eq!(harness.status("alpha").await, "applying");
    assert_eq!(
        harness.queue_intent("alpha").await,
        QueueIntent::Queued,
        "active lifecycle evidence must survive the unmark"
    );
    assert_eq!(
        harness
            .coordinator()
            .last_plan()
            .expect("a pass ran")
            .excluded,
        vec![("alpha".to_string(), MarkSettlementExclusion::Active)]
    );
    assert!(!harness.marks.is_marked("alpha"), "the mark itself is gone");
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

// ============================================================================
// Bidirectional reconciliation through the guarded queue path
// ============================================================================

#[tokio::test(start_paused = true)]
async fn bidirectional_mark_reconciliation_projection_unmark_publishes_not_queued() {
    let harness = Harness::running(&["alpha"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;
    assert_eq!(harness.status("alpha").await, "queued");

    assert!(harness.operator.apply_execution_mark("alpha", false).await);
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue_intent("alpha").await,
        QueueIntent::NotQueued,
        "the reducer is the authority the TUI projects from"
    );
    assert_eq!(harness.status("alpha").await, "not queued");
    assert_eq!(
        harness
            .coordinator()
            .last_plan()
            .expect("a pass ran")
            .removals,
        vec!["alpha".to_string()]
    );
    assert!(
        !harness
            .queue
            .calls()
            .iter()
            .any(|call| matches!(call, QueueCall::Cancel(_))),
        "a removal is not a dequeue: {:?}",
        harness.queue.calls()
    );

    // The frontends learn about it through the same authoritative outcome an
    // explicit queue command publishes.
    let deltas: Vec<_> = harness
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ExecutionEvent::OperatorCommandApplied {
                effect: crate::events::OperatorCommandEffect::QueueDelta { change_id, queued },
            } => Some((change_id, queued)),
            _ => None,
        })
        .collect();
    assert_eq!(
        deltas,
        vec![("alpha".to_string(), true), ("alpha".to_string(), false)]
    );
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_delta_scope_reconciles_only_named_targets() {
    let harness = Harness::running(&["alpha", "beta"]);
    // `beta` is marked and queued from an earlier settled batch.
    harness.operator.apply_execution_mark("beta", true).await;
    settle(&harness.coordinator()).await;

    // A batch that names only `alpha` must leave `beta` exactly as it was, even
    // though `beta` is still marked and would classify identically.
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;

    let plan = harness.coordinator().last_plan().expect("a pass ran");
    assert_eq!(plan.additions, vec!["alpha".to_string()]);
    assert!(plan.removals.is_empty());
    assert!(
        plan.excluded.is_empty(),
        "an unnamed target is never read, so it is never even excluded: {:?}",
        plan.excluded
    );
    assert_eq!(harness.queue_intent("beta").await, QueueIntent::Queued);
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_preserves_unrelated_queue_intent() {
    let harness = Harness::running(&["explicitly_queued", "marked_but_removed", "trigger"]);

    // An unmarked change the operator queued explicitly.
    harness
        .application
        .apply(OperatorIntent::SetQueueIntent {
            change_id: "explicitly_queued".to_string(),
            queued: true,
        })
        .await;
    assert!(!harness.marks.is_marked("explicitly_queued"));

    // A marked change the operator then removed from the queue explicitly.
    harness
        .operator
        .apply_execution_mark("marked_but_removed", true)
        .await;
    settle(&harness.coordinator()).await;
    harness
        .application
        .apply(OperatorIntent::SetQueueIntent {
            change_id: "marked_but_removed".to_string(),
            queued: false,
        })
        .await;
    assert!(harness.marks.is_marked("marked_but_removed"));
    assert_eq!(
        harness.queue_intent("marked_but_removed").await,
        QueueIntent::NotQueued
    );

    // An unrelated mark settles.
    harness.operator.apply_execution_mark("trigger", true).await;
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue_intent("explicitly_queued").await,
        QueueIntent::Queued,
        "an explicit queue addition is not mark intent and must survive settlement"
    );
    assert_eq!(
        harness.queue_intent("marked_but_removed").await,
        QueueIntent::NotQueued,
        "an explicit queue removal must not be undone by an unrelated mark settling"
    );
    let plan = harness.coordinator().last_plan().expect("a pass ran");
    assert_eq!(plan.additions, vec!["trigger".to_string()]);
    assert!(plan.removals.is_empty());
}

#[tokio::test(start_paused = true)]
async fn bidirectional_mark_reconciliation_entrypoints_agree() {
    // Individual (TUI), API (single-target command), and bulk `x` must produce
    // the same settled state from the same starting point, in both directions.
    let individual = Harness::running(&["alpha"]);
    individual
        .operator
        .apply_execution_mark("alpha", true)
        .await;
    settle(&individual.coordinator()).await;
    individual
        .operator
        .apply_execution_mark("alpha", false)
        .await;
    settle(&individual.coordinator()).await;

    let api = Harness::running(&["alpha"]);
    api.operator
        .set_execution_mark("alpha", true)
        .await
        .expect("a fresh row accepts a mark");
    settle(&api.coordinator()).await;
    api.operator
        .set_execution_mark("alpha", false)
        .await
        .expect("a queued row accepts an unmark");
    settle(&api.coordinator()).await;

    let bulk = Harness::running(&["alpha"]);
    bulk.operator
        .set_all_execution_marks()
        .await
        .expect("visible rows accept a bulk mark");
    settle(&bulk.coordinator()).await;
    bulk.operator
        .set_all_execution_marks()
        .await
        .expect("a fully marked set toggles off");
    settle(&bulk.coordinator()).await;

    for (name, harness) in [("individual", &individual), ("api", &api), ("bulk", &bulk)] {
        assert_eq!(
            harness.queue.calls(),
            vec![
                QueueCall::Add("alpha".to_string()),
                QueueCall::Notify,
                QueueCall::Remove("alpha".to_string()),
                QueueCall::Notify,
            ],
            "the {name} entry point produced a different settlement"
        );
        assert_eq!(harness.status("alpha").await, "not queued");
        assert_eq!(
            harness.hooks.calls(),
            vec![
                HookCall::Add("alpha".to_string()),
                HookCall::Remove("alpha".to_string()),
            ],
            "the {name} entry point produced different hooks"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_hooks_and_notification_coalesce_per_batch() {
    let harness = Harness::running(&["alpha", "beta"]);
    harness.operator.apply_execution_mark("alpha", true).await;
    settle(&harness.coordinator()).await;
    assert_eq!(
        harness.queue.calls(),
        vec![QueueCall::Add("alpha".to_string()), QueueCall::Notify]
    );

    // One batch carrying both directions: `alpha` unmarked, `beta` marked.
    harness.operator.apply_execution_mark("alpha", false).await;
    harness.operator.apply_execution_mark("beta", true).await;
    settle(&harness.coordinator()).await;

    assert_eq!(
        harness.queue.calls(),
        vec![
            QueueCall::Add("alpha".to_string()),
            QueueCall::Notify,
            QueueCall::Add("beta".to_string()),
            QueueCall::Remove("alpha".to_string()),
            QueueCall::Notify,
        ],
        "one batch, one scheduler wake, after every mutation: {:?}",
        harness.queue.calls()
    );
    assert_eq!(
        harness.hooks.calls(),
        vec![
            HookCall::Add("alpha".to_string()),
            HookCall::Add("beta".to_string()),
            HookCall::Remove("alpha".to_string()),
        ],
        "queue hooks stay exactly once per successful target mutation"
    );
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_hooks_and_notification_no_op_batch_never_wakes_the_scheduler() {
    let harness = Harness::running(&["alpha"]);
    // Marked, but worktree execution refuses the row, so the batch applies
    // nothing at all.
    harness.parallel.set_parallel_ineligible([(
        "alpha".to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);
    harness.operator.apply_execution_mark("alpha", true).await;

    settle(&harness.coordinator()).await;

    assert!(
        harness.queue.calls().is_empty(),
        "a no-op batch emits no wake and no mutation: {:?}",
        harness.queue.calls()
    );
    assert!(harness.hooks.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_application_guard_refuses_every_raced_lifecycle() {
    // The guard's own table, driven directly: classification and application are
    // two instants, and this is the one that has the reducer write boundary.
    let cases: &[(&str, ExecutionEvent, MarkSettlementExclusion)] = &[
        (
            "raced_active",
            ExecutionEvent::ApplyStarted {
                change_id: "raced_active".to_string(),
                command: "echo".to_string(),
            },
            MarkSettlementExclusion::Active,
        ),
        (
            "raced_error",
            ExecutionEvent::ApplyFailed {
                change_id: "raced_error".to_string(),
                error: "boom".to_string(),
            },
            MarkSettlementExclusion::Terminal,
        ),
    ];

    for (change_id, event, expected) in cases {
        for action in [MarkSettlementAction::Add, MarkSettlementAction::Remove] {
            let harness = Harness::running(&[change_id]);
            harness.state.write().await.apply_execution_event(event);

            let application = harness
                .operator
                .apply_settlement_queue_intent(change_id, action)
                .await;

            assert_eq!(
                application.skipped,
                Some(*expected),
                "'{change_id}' with {action:?} reported the wrong guard reason"
            );
            assert!(!application.applied());
            assert!(
                harness.queue.calls().is_empty(),
                "a refused mutation touches no runtime: {:?}",
                harness.queue.calls()
            );
            assert!(harness.hooks.calls().is_empty());
        }
    }
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_removal_race_preserves_active() {
    let harness = Harness::running(&["alpha"]);
    harness
        .reduce(ReducerCommand::AddToQueue("alpha".to_string()))
        .await;
    // Dispatch wins the race: the row is active but its queue intent is intact.
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "echo".to_string(),
        });

    let application = harness
        .operator
        .apply_settlement_queue_intent("alpha", MarkSettlementAction::Remove)
        .await;

    assert_eq!(
        application.skipped,
        Some(MarkSettlementExclusion::Active),
        "a removal that lost the race to dispatch is a reasoned no-op"
    );
    assert!(!application.applied());
    assert_eq!(
        harness.queue_intent("alpha").await,
        QueueIntent::Queued,
        "active lifecycle evidence must not be cleared"
    );
    assert_eq!(harness.status("alpha").await, "applying");
    assert!(harness.queue.calls().is_empty());
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_addition_race_never_retries() {
    let harness = Harness::running(&["alpha"]);
    // The row failed terminally between classification and application.
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        });
    assert_eq!(harness.status("alpha").await, "error");

    let application = harness
        .operator
        .apply_settlement_queue_intent("alpha", MarkSettlementAction::Add)
        .await;

    assert_eq!(
        application.skipped,
        Some(MarkSettlementExclusion::Terminal),
        "a terminal-error row is excluded, never retried"
    );
    assert!(!application.applied());
    assert!(
        harness.queue.calls().is_empty(),
        "no explicit-retry edge, no queue mutation: {:?}",
        harness.queue.calls()
    );
    assert_eq!(
        harness.status("alpha").await,
        "error",
        "the retained error must survive"
    );
}

#[tokio::test(start_paused = true)]
async fn mark_settlement_restart_is_process_local() {
    // A restart is a fresh store and a fresh coordinator, by construction.
    // Nothing an unsettled batch recorded can reach the new process.
    let before = Harness::running(&["alpha"]);
    before
        .reduce(ReducerCommand::AddToQueue("alpha".to_string()))
        .await;
    before.operator.apply_execution_mark("alpha", true).await;
    before.operator.apply_execution_mark("alpha", false).await;
    assert!(before.coordinator().is_armed());

    let after = Harness::running(&["alpha"]);
    after
        .reduce(ReducerCommand::AddToQueue("alpha".to_string()))
        .await;
    assert!(!after.marks.is_marked("alpha"));
    assert!(!after.coordinator().is_armed());
    assert_eq!(after.coordinator().pending_snapshot(), None);

    settle_expect_no_pass(&after.coordinator()).await;
    assert!(
        after.queue.calls().is_empty(),
        "a restarted process performs no delayed queue mutation: {:?}",
        after.queue.calls()
    );
    assert_eq!(
        after.queue_intent("alpha").await,
        QueueIntent::Queued,
        "workspace-derived queue intent is untouched by the lost batch"
    );
}
