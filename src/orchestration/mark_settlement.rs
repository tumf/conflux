//! Process-local stability policy between execution marks and queue intent.
//!
//! Execution marks and reducer queue intent stay two separate projections. What
//! was missing between them was an operator path for adding newly selected work
//! to a run that is *already live*: a mark expressed intent that nothing ever
//! consumed until the next Start.
//!
//! This module is that bridge, and it is deliberately only a *policy* bridge —
//! it merges no projection and owns no queue state. One accepted standalone
//! operator mark write arms a single 10-second stability deadline. Every later
//! accepted operator write replaces the pending snapshot and restarts that one
//! deadline. When the deadline finally expires, settlement reads the marks that
//! exist *then*, classifies them against one coherent reducer/operator view, and
//! adds the eligible ordinary `not queued` rows through the same queue command
//! path a frontend would have used.
//!
//! Three properties are structural rather than rules to re-check:
//!
//! *Arming never touches the application transaction.* [`MarkSettlementCoordinator::notify`]
//! records timer state and spawns a task. It is called from inside the operator
//! mutation guard — including from a bulk write — so re-entering the reducer or
//! that guard here would deadlock the very command that armed it.
//!
//! *Settlement is additive-only.* The plan can only add. Unmarking changes mark
//! intent and nothing else, so no mark control can dequeue, cancel, stop, retry,
//! or resolve admitted work.
//!
//! *Nothing here is durable.* The deadline and the pending snapshot live in
//! memory for one process lifetime. Under `openspec/CONSTITUTION.md` a restart
//! recomputes the next action from workspace and Git evidence alone, so a
//! pending snapshot that never settled leaves no trace to route from.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use async_trait::async_trait;

use crate::orchestration::operator_command::{is_active_status, is_final_status};

/// How long an operator mark set must stay unchanged before it is admitted.
///
/// Fixed on purpose: the requested policy is one stable interval, not a tuning
/// surface. It coincides with the scheduler's queue-coalescing debounce today,
/// but it is an independent *pre-admission* interval — the scheduler's existing
/// explicit queue-addition bypass is what keeps the two from stacking.
pub const MARK_STABILITY_WINDOW: Duration = Duration::from_secs(10);

// ============================================================================
// Classification
// ============================================================================

/// Why one marked row produced no queue addition at settlement.
///
/// Every reason is a *reasoned skip*, never a refusal: settlement is a
/// background policy pass, so an ineligible row simply keeps its mark and gains
/// no side effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkSettlementExclusion {
    /// The reducer does not track the change, so there is nothing loadable to admit.
    NotLoadable,
    /// The change reached a terminal outcome, a terminal error, or a stop.
    ///
    /// The queue service must not be called for these at all: its terminal-error
    /// branch is an explicit *retry*, which is intent no mark ever expressed.
    Terminal,
    /// The change is actively executing.
    Active,
    /// The change is waiting — merge, resolve, reject, dependency, or stall.
    Waiting,
    /// The change already carries reducer queue intent.
    AlreadyQueued,
    /// Worktree execution refuses the change.
    Unavailable,
}

impl MarkSettlementExclusion {
    /// Stable machine-readable token, so a log line and a test name one word.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotLoadable => "not_loadable",
            Self::Terminal => "terminal",
            Self::Active => "active",
            Self::Waiting => "waiting",
            Self::AlreadyQueued => "already_queued",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One marked change as a single settlement observation saw it.
#[derive(Debug, Clone, Copy)]
pub struct MarkSettlementRow<'a> {
    /// Target change.
    pub change_id: &'a str,
    /// Reducer-derived display status at the observation instant.
    pub display_status: &'a str,
    /// Whether the reducer tracks the change at all.
    pub tracked: bool,
    /// Whether worktree execution admits the change.
    pub parallel_eligible: bool,
}

/// The additive plan one settlement pass derived.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MarkSettlementPlan {
    /// Changes to add through the shared queue command path, in observation order.
    pub additions: Vec<String>,
    /// Marked rows that gained no queue effect, each with its reason.
    pub excluded: Vec<(String, MarkSettlementExclusion)>,
}

impl MarkSettlementPlan {
    /// True when the plan would mutate nothing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty()
    }
}

/// Classify one marked row; `None` means it is an ordinary admission candidate.
///
/// The order is precedence, not convenience: a terminal row that also happens to
/// be ineligible must report `Terminal`, because that is the reason its mark can
/// never become queue intent again.
pub fn classify_mark_settlement_row(
    row: &MarkSettlementRow<'_>,
) -> Option<MarkSettlementExclusion> {
    if !row.tracked {
        return Some(MarkSettlementExclusion::NotLoadable);
    }
    if is_final_status(row.display_status) || matches!(row.display_status, "error" | "stopped") {
        return Some(MarkSettlementExclusion::Terminal);
    }
    if is_active_status(row.display_status) {
        return Some(MarkSettlementExclusion::Active);
    }
    if matches!(
        row.display_status,
        "merge wait" | "resolve pending" | "reject pending" | "blocked" | "stalled"
    ) {
        return Some(MarkSettlementExclusion::Waiting);
    }
    if row.display_status == "queued" {
        return Some(MarkSettlementExclusion::AlreadyQueued);
    }
    if row.display_status != "not queued" {
        // An unrecognised word is not evidence of admissibility. Fail closed so a
        // future display status cannot silently become a queue addition.
        return Some(MarkSettlementExclusion::Waiting);
    }
    if !row.parallel_eligible {
        return Some(MarkSettlementExclusion::Unavailable);
    }
    None
}

/// Derive the whole additive plan from one coherent observation.
pub fn plan_mark_settlement(rows: &[MarkSettlementRow<'_>]) -> MarkSettlementPlan {
    let mut plan = MarkSettlementPlan::default();
    for row in rows {
        match classify_mark_settlement_row(row) {
            Some(reason) => plan.excluded.push((row.change_id.to_string(), reason)),
            None => plan.additions.push(row.change_id.to_string()),
        }
    }
    plan
}

// ============================================================================
// Runtime port
// ============================================================================

/// What a settlement pass may do, isolated from the timer that schedules it.
///
/// The coordinator owns *when*; this owns *what*. Keeping them apart is what
/// lets the deadline behaviour be verified without a reducer, a queue, or a
/// scheduler, and what keeps the coordinator from needing to know that the real
/// implementation is the operator application transaction.
#[async_trait]
pub trait MarkSettlementRuntime: Send + Sync {
    /// True when a live scheduler capable of dynamic queue admission exists.
    ///
    /// Scheduler liveness, never presentation mode: a persistent scheduler
    /// reports `Select` while parked and is still perfectly able to admit work.
    fn admits_dynamic_queue(&self) -> bool;

    /// Read the current marks, classify them, and apply the additive plan.
    async fn settle_marks(&self) -> MarkSettlementPlan;

    /// Report that a pending snapshot was abandoned because the scheduler ended.
    ///
    /// Informational only. A finite run gets no new termination barrier, so the
    /// operator learns that the marks they made will not be admitted rather than
    /// discovering it from a run that silently did nothing.
    async fn report_abandoned_settlement(&self, pending: Vec<String>);
}

// ============================================================================
// Coordinator
// ============================================================================

/// The one process-local mark-settlement notifier.
///
/// Owned alongside [`crate::orchestration::operator_command::ExecutionMarkStore`],
/// because the store is the single point both frontend service paths already
/// write through. A second notifier would be a second answer to "when does this
/// mark set become queue intent".
pub struct MarkSettlementCoordinator {
    inner: Mutex<CoordinatorInner>,
    window: Duration,
    /// Completed passes, settled and abandoned alike.
    ///
    /// A pass runs on its own task, so an observer that needs to know one
    /// finished — a diagnostic, or a deterministic test — waits on this
    /// transition instead of polling or sleeping.
    passes: tokio::sync::watch::Sender<u64>,
}

#[derive(Default)]
struct CoordinatorInner {
    /// The settlement runtime, held weakly.
    ///
    /// The store this coordinator lives in is reachable *from* the runtime, so a
    /// strong handle here would be a cycle that never drops. A process whose
    /// runtime is gone has no live scheduler either, which is exactly the state
    /// in which arming must not happen.
    runtime: Option<Weak<dyn MarkSettlementRuntime>>,
    /// Monotonic arming identity. A task whose generation is stale exits.
    generation: u64,
    /// The mark set observed when the current deadline was armed.
    ///
    /// Evidence only — settlement deliberately re-reads the marks that exist at
    /// expiry, so a lifecycle-driven revocation lands in the final plan without
    /// extending the deadline.
    pending: Option<Vec<String>>,
    /// Completed settlement passes, for observability and deterministic tests.
    settled: u64,
    /// Passes abandoned because the scheduler ended before the deadline.
    abandoned: u64,
    /// The plan the most recent completed pass derived.
    last_plan: Option<MarkSettlementPlan>,
}

impl std::fmt::Debug for MarkSettlementCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = self.lock();
        f.debug_struct("MarkSettlementCoordinator")
            .field("armed", &guard.pending.is_some())
            .field("generation", &guard.generation)
            .field("settled", &guard.settled)
            .field("abandoned", &guard.abandoned)
            .finish()
    }
}

impl Default for MarkSettlementCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkSettlementCoordinator {
    /// A fresh, unarmed coordinator with the standard stability window.
    ///
    /// Fresh on every construction on purpose: a restart begins with no pending
    /// snapshot, which is what keeps the deadline from being workflow evidence.
    pub fn new() -> Self {
        Self::with_window(MARK_STABILITY_WINDOW)
    }

    /// A coordinator with an explicit stability window.
    ///
    /// Production always uses [`MARK_STABILITY_WINDOW`]; a test uses this to
    /// keep paused-time arithmetic readable.
    pub fn with_window(window: Duration) -> Self {
        Self {
            inner: Mutex::new(CoordinatorInner::default()),
            window,
            passes: tokio::sync::watch::channel(0).0,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CoordinatorInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Bind the settlement runtime for this process.
    ///
    /// Until this is bound — a CLI run, a unit test, a frontend built before the
    /// application transaction exists — every notification is mark-only.
    pub fn bind_runtime(&self, runtime: Weak<dyn MarkSettlementRuntime>) {
        self.lock().runtime = Some(runtime);
    }

    /// The stability window this coordinator arms.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn window(&self) -> Duration {
        self.window
    }

    /// Arm or restart the single stability deadline for `snapshot`.
    ///
    /// Returns true when a deadline is now pending. False means the process has
    /// no live scheduler capable of dynamic queue admission, so the mark write
    /// stays exactly what it was: mark-only.
    ///
    /// Lock discipline matters here. This is called with the operator mutation
    /// guard held, so it must never take the reducer, take that guard again, or
    /// await anything; it records timer state and spawns the settlement task.
    pub fn notify(self: &Arc<Self>, snapshot: Vec<String>) -> bool {
        let Some(runtime) = self.runtime() else {
            return false;
        };
        if !runtime.admits_dynamic_queue() {
            return false;
        }
        // No runtime handle means no task can be spawned to settle the deadline,
        // and an armed deadline that can never expire would be a lie.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return false;
        };

        let generation = {
            let mut guard = self.lock();
            guard.generation += 1;
            guard.pending = Some(snapshot);
            guard.generation
        };

        let coordinator = self.clone();
        let window = self.window;
        handle.spawn(async move {
            tokio::time::sleep(window).await;
            coordinator.settle(generation).await;
        });
        true
    }

    /// The marks observed when the current deadline was armed, if one is pending.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pending_snapshot(&self) -> Option<Vec<String>> {
        self.lock().pending.clone()
    }

    /// Whether a stability deadline is currently pending.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_armed(&self) -> bool {
        self.lock().pending.is_some()
    }

    /// Completed settlement passes in this process lifetime.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn settled_count(&self) -> u64 {
        self.lock().settled
    }

    /// Passes abandoned because the scheduler ended before the deadline.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn abandoned_count(&self) -> u64 {
        self.lock().abandoned
    }

    /// The plan the most recent completed pass derived.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_plan(&self) -> Option<MarkSettlementPlan> {
        self.lock().last_plan.clone()
    }

    /// Watch completed passes, settled and abandoned alike.
    ///
    /// The current value is already visible to a new receiver, so an observer
    /// that subscribes after a pass finished still sees it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn passes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.passes.subscribe()
    }

    fn record_pass(&self) {
        self.passes.send_modify(|passes| *passes += 1);
    }

    fn runtime(&self) -> Option<Arc<dyn MarkSettlementRuntime>> {
        self.lock().runtime.as_ref().and_then(Weak::upgrade)
    }

    /// Run one expired deadline, or exit because it was superseded.
    async fn settle(self: Arc<Self>, generation: u64) {
        let snapshot = {
            let mut guard = self.lock();
            if guard.generation != generation {
                // A later accepted operator write already replaced this snapshot.
                // Exactly one deadline is live, so this task simply retires.
                return;
            }
            guard.pending.take()
        };
        let Some(runtime) = self.runtime() else {
            return;
        };

        // Liveness is re-read at expiry, not trusted from arming time. A finite
        // scheduler that ended in between discards its snapshot here, which is
        // also what keeps the pending deadline from being a termination barrier.
        if !runtime.admits_dynamic_queue() {
            self.lock().abandoned += 1;
            runtime
                .report_abandoned_settlement(snapshot.unwrap_or_default())
                .await;
            self.record_pass();
            return;
        }

        let plan = runtime.settle_marks().await;
        {
            let mut guard = self.lock();
            guard.settled += 1;
            guard.last_plan = Some(plan);
        }
        self.record_pass();
    }
}

#[cfg(test)]
mod tests;
