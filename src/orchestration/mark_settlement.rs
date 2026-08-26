//! Process-local stability policy between execution marks and queue intent.
//!
//! Execution marks and reducer queue intent stay two separate projections. What
//! was missing between them was an operator path for adding newly selected work
//! to a run that is *already live*: a mark expressed intent that nothing ever
//! consumed until the next Start.
//!
//! This module is that bridge, and it is deliberately only a *policy* bridge —
//! it merges no projection and owns no queue state. One accepted standalone
//! operator mark write arms a single 10-second stability deadline and records
//! the targets whose marks that write actually changed. Every later accepted
//! operator write merges its own changed targets into the same batch and
//! restarts that one deadline. When the deadline finally expires, settlement
//! re-reads *only the batch's targets* against one coherent reducer/operator
//! view and reconciles each of them in both directions through the same queue
//! command path a frontend would have used.
//!
//! Three properties are structural rather than rules to re-check:
//!
//! *Arming never touches the application transaction.* [`MarkSettlementCoordinator::notify`]
//! records timer state and spawns a task. It is called from inside the operator
//! mutation guard — including from a bulk write — so re-entering the reducer or
//! that guard here would deadlock the very command that armed it.
//!
//! *Settlement is delta-scoped.* A batch names exactly the targets whose marks
//! actually changed, and reconciliation touches only those. A global mark/queue
//! convergence would destroy explicit queue intent nobody expressed a mark for:
//! an unmarked row can be explicitly queued, and a marked row can be explicitly
//! removed from the queue. Neither may move because an unrelated mark settled.
//!
//! *Settlement is bidirectional but never a lifecycle control.* A newly marked
//! ordinary `not queued` row gains queue intent; a newly unmarked ordinary
//! pending row loses it. Removing queue intent is not dequeueing: no mark
//! control can cancel, stop, alter a phase, retry, or resolve admitted work,
//! because every row that is active, waiting, or terminal is excluded before a
//! mutation is ever planned.
//!
//! *A batch is held until it is answered, not until it is read.* Reconciliation
//! classifies each target from reducer runtime state, and a mark is accepted
//! against the *catalog*, which can be ahead of the reducer by a refresh
//! interval. A target the reducer cannot load yet has therefore produced no
//! answer at all, so it stays in the batch and the deadline is re-armed, for a
//! bounded number of attempts. Every other classification — terminal, active,
//! waiting, already reconciled, worktree-refused — is an answer, and ends the
//! batch's interest in that target immediately.
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

/// How many settlement passes one accepted batch may spend on targets the
/// reducer has not observed yet.
///
/// A mark is accepted against the *catalog*, and the catalog can be ahead of the
/// reducer: a proposal created seconds ago is markable through the TUI and
/// `cflx client` before any `ChangesRefreshed` has given it reducer runtime
/// state. The first pass then classifies it [`MarkSettlementExclusion::NotLoadable`],
/// which is the absence of evidence rather than evidence — so the batch keeps
/// its deadline instead of dropping the operator's intent.
///
/// Bounded on purpose. A target that never becomes loadable — a mark for a
/// change ID that does not exist — must stop costing passes and start producing
/// [`MarkSettlementFailure::UnreconciledBatch`] evidence instead of re-arming
/// for the rest of the process lifetime.
pub const MARK_SETTLEMENT_ATTEMPTS: u32 = 3;

// ============================================================================
// Classification
// ============================================================================

/// Why one named row produced no queue mutation at settlement.
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
    /// The change already carries reducer queue intent, and is marked.
    AlreadyQueued,
    /// The change already carries no reducer queue intent, and is unmarked.
    AlreadyNotQueued,
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
            Self::AlreadyNotQueued => "already_not_queued",
            Self::Unavailable => "unavailable",
        }
    }

    /// Whether this reason is evidence *about* the row, or merely its absence.
    ///
    /// Every reason but one is a decision taken over reducer runtime state the
    /// owner actually holds: terminal, active, waiting, already reconciled, or
    /// refused by worktree execution. [`Self::NotLoadable`] is the exception —
    /// it means the reducer has no runtime state for the target at all, which
    /// says nothing about whether the mark should become queue intent. Treating
    /// it as a settled answer is what let an accepted mark for a
    /// just-created proposal stay `not queued` forever: the catalog admitted the
    /// mark, the deadline expired before the next `ChangesRefreshed`, and the
    /// batch was discarded a second before the row became loadable.
    pub fn is_stable(self) -> bool {
        !matches!(self, Self::NotLoadable)
    }
}

/// Why a settlement lifecycle could not carry an accepted batch to a decision.
///
/// Distinct from [`MarkSettlementExclusion`], which is a *classification* of a
/// row that settlement did reach. These are failures of the machinery itself,
/// and the requirement they exist for is that none of them may present as a
/// silent mark-only result while the owner reports a live scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkSettlementFailure {
    /// No settlement runtime was ever bound to this process.
    ///
    /// Correct and expected for a CLI run or a unit test; a defect only in a
    /// command-capable owner, where binding happens right after the application
    /// transaction is built.
    RuntimeUnbound,
    /// The bound runtime was dropped before the batch could be settled.
    RuntimeGone,
    /// No async runtime handle was available to own the deadline task.
    NoTaskRuntime,
    /// The batch spent its whole attempt budget without becoming loadable.
    UnreconciledBatch,
}

impl MarkSettlementFailure {
    /// Stable machine-readable token for logs, events, and tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeUnbound => "runtime_unbound",
            Self::RuntimeGone => "runtime_gone",
            Self::NoTaskRuntime => "no_task_runtime",
            Self::UnreconciledBatch => "unreconciled_batch",
        }
    }
}

/// The queue mutation one reconciled row asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkSettlementAction {
    /// The row is marked and idle-ordinary-`not queued`: publish queue intent.
    Add,
    /// The row is unmarked and idle-ordinary-`queued`: withdraw queue intent.
    Remove,
}

/// One named change as a single settlement observation saw it.
#[derive(Debug, Clone, Copy)]
pub struct MarkSettlementRow<'a> {
    /// Target change.
    pub change_id: &'a str,
    /// Reducer-derived display status at the observation instant.
    ///
    /// One word carries the whole lifecycle: `"queued"` is reachable only when
    /// the reducer intent is `Queued` *and* the activity is idle *and* no wait
    /// or terminal state applies, and `"not queued"` is its `NotQueued` mirror.
    /// That is exactly the "idle ordinary pending" the reconciliation rule
    /// names, derived from one snapshot instead of three separate reads that
    /// could disagree.
    pub display_status: &'a str,
    /// Whether the reducer tracks the change at all.
    pub tracked: bool,
    /// Whether worktree execution admits the change.
    pub parallel_eligible: bool,
    /// Whether the change carries an execution mark *now*, at expiry.
    ///
    /// Read at the settlement observation rather than taken from the batch: a
    /// target can be marked and unmarked again inside one stability window, and
    /// what the operator left behind is what reconciliation must honour.
    pub marked: bool,
}

/// The bidirectional plan one settlement pass derived.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MarkSettlementPlan {
    /// Changes to queue through the shared queue command path, in observation order.
    pub additions: Vec<String>,
    /// Changes to unqueue through the shared queue command path, in observation order.
    pub removals: Vec<String>,
    /// Named rows that gained no queue effect, each with its reason.
    pub excluded: Vec<(String, MarkSettlementExclusion)>,
}

impl MarkSettlementPlan {
    /// True when the plan would mutate nothing.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty() && self.removals.is_empty()
    }
}

/// Classify one named row into the mutation it asks for, or why it asks for none.
///
/// The order is precedence, not convenience: a terminal row that also happens to
/// be ineligible must report `Terminal`, because that is the reason its mark can
/// never become queue intent again. Every lifecycle exclusion is evaluated
/// before the mark is consulted at all, which is what makes "unmarking cannot
/// disturb active, waiting, or terminal work" structural rather than a rule the
/// removal branch has to re-check.
pub fn classify_mark_settlement_row(
    row: &MarkSettlementRow<'_>,
) -> std::result::Result<MarkSettlementAction, MarkSettlementExclusion> {
    if !row.tracked {
        return Err(MarkSettlementExclusion::NotLoadable);
    }
    if is_final_status(row.display_status) || matches!(row.display_status, "error" | "stopped") {
        return Err(MarkSettlementExclusion::Terminal);
    }
    if is_active_status(row.display_status) {
        return Err(MarkSettlementExclusion::Active);
    }
    if matches!(
        row.display_status,
        "merge wait" | "resolve pending" | "reject pending" | "blocked" | "stalled"
    ) {
        return Err(MarkSettlementExclusion::Waiting);
    }
    match row.display_status {
        "queued" if row.marked => Err(MarkSettlementExclusion::AlreadyQueued),
        "queued" => Ok(MarkSettlementAction::Remove),
        "not queued" if !row.marked => Err(MarkSettlementExclusion::AlreadyNotQueued),
        "not queued" if !row.parallel_eligible => Err(MarkSettlementExclusion::Unavailable),
        "not queued" => Ok(MarkSettlementAction::Add),
        // An unrecognised word is not evidence of admissibility. Fail closed so a
        // future display status cannot silently become a queue mutation.
        _ => Err(MarkSettlementExclusion::Waiting),
    }
}

/// Derive the whole bidirectional plan from one coherent observation.
///
/// `rows` are the batch's targets and nothing else. A row the batch never named
/// is not in this input, which is what keeps an explicitly queued unmarked
/// change — and a marked change explicitly removed from the queue — untouched by
/// somebody else's mark settling.
pub fn plan_mark_settlement(rows: &[MarkSettlementRow<'_>]) -> MarkSettlementPlan {
    let mut plan = MarkSettlementPlan::default();
    for row in rows {
        match classify_mark_settlement_row(row) {
            Ok(MarkSettlementAction::Add) => plan.additions.push(row.change_id.to_string()),
            Ok(MarkSettlementAction::Remove) => plan.removals.push(row.change_id.to_string()),
            Err(reason) => plan.excluded.push((row.change_id.to_string(), reason)),
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

    /// Re-read exactly `targets`, classify them, and apply the plan.
    ///
    /// `targets` is the settled batch: every change whose mark an accepted
    /// operator write actually flipped since the last pass, in the order those
    /// writes were accepted. It is a *scope*, not a set of marks — the current
    /// mark of each target is read here, at expiry.
    async fn settle_marks(&self, targets: Vec<String>) -> MarkSettlementPlan;

    /// Report that a pending batch was abandoned because the scheduler ended.
    ///
    /// Informational only. A finite run gets no new termination barrier, so the
    /// operator learns that the marks they made will not be reconciled rather
    /// than discovering it from a run that silently did nothing.
    async fn report_abandoned_settlement(&self, pending: Vec<String>);

    /// Report a settlement lifecycle failure with a stable reason.
    ///
    /// The counterpart of [`Self::report_abandoned_settlement`] for everything
    /// that is *not* an orderly scheduler end: a batch that never became
    /// loadable, or a runtime that vanished under a pending deadline. A
    /// command-capable owner turns this into an operator-visible entry, because
    /// the alternative — a mark that stays marked and `not queued` with nothing
    /// said about it — is the exact failure this whole path exists to prevent.
    ///
    /// The default drops it, which is correct for a runtime with no
    /// operator-facing dispatch behind it.
    async fn report_settlement_failure(
        &self,
        _failure: MarkSettlementFailure,
        _targets: Vec<String>,
    ) {
    }
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
    /// The settlement batch: every target whose mark an accepted operator write
    /// changed since the last completed pass, in acceptance order.
    ///
    /// It accumulates rather than replaces. Restarting the deadline must not
    /// drop the earlier writes it is waiting on, or a two-keystroke unmark
    /// followed by a mark elsewhere would reconcile only the second target.
    ///
    /// Scope only — settlement deliberately re-reads the *marks* that exist at
    /// expiry, so a lifecycle-driven revocation lands in the final plan without
    /// extending the deadline.
    pending: Option<Vec<String>>,
    /// Completed settlement passes, for observability and deterministic tests.
    settled: u64,
    /// Passes abandoned because the scheduler ended before the deadline.
    abandoned: u64,
    /// The plan the most recent completed pass derived.
    last_plan: Option<MarkSettlementPlan>,
    /// Passes the current batch has already spent on unloadable targets.
    ///
    /// Reset by every accepted operator write, because a fresh write is fresh
    /// intent and deserves the full budget again.
    attempts: u32,
    /// The most recent lifecycle failure, for diagnostics and tests.
    last_failure: Option<MarkSettlementFailure>,
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

    /// Merge `changed` into the settlement batch and restart the one deadline.
    ///
    /// `changed` names the targets whose marks this accepted write actually
    /// flipped — one for a single-row write, many for a bulk one, none for a
    /// no-op, which never reaches here at all.
    ///
    /// Returns true when a deadline is now pending. False means the process has
    /// no live scheduler capable of dynamic queue admission, so the mark write
    /// stays exactly what it was: mark-only.
    ///
    /// Lock discipline matters here. This is called with the operator mutation
    /// guard held, so it must never take the reducer, take that guard again, or
    /// await anything; it records timer state and spawns the settlement task.
    pub fn notify(self: &Arc<Self>, changed: Vec<String>) -> bool {
        let runtime = match self.runtime_or_failure() {
            Ok(runtime) => runtime,
            Err(failure) => {
                self.record_failure(failure, &changed);
                return false;
            }
        };
        // Not a failure: a process with no live scheduler has nothing to admit
        // work into, so a mark-only result is the correct one.
        if !runtime.admits_dynamic_queue() {
            return false;
        }
        // No runtime handle means no task can be spawned to settle the deadline,
        // and an armed deadline that can never expire would be a lie.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.record_failure(MarkSettlementFailure::NoTaskRuntime, &changed);
            return false;
        };

        let generation = {
            let mut guard = self.lock();
            guard.generation += 1;
            // Fresh operator intent, so the batch gets its whole attempt budget
            // back: a retry budget spent waiting for an earlier target to become
            // loadable must not shorten the wait this write is entitled to.
            guard.attempts = 0;
            let batch = guard.pending.get_or_insert_with(Vec::new);
            for change_id in changed {
                // Acceptance order, deduplicated: a target marked, unmarked, and
                // marked again inside one window is still one target to
                // reconcile, and re-reading it twice could only produce the same
                // classification twice.
                if !batch.iter().any(|existing| existing == &change_id) {
                    batch.push(change_id);
                }
            }
            guard.generation
        };

        self.arm(&handle, generation);
        true
    }

    /// Spawn the one task that owns `generation`'s deadline.
    fn arm(self: &Arc<Self>, handle: &tokio::runtime::Handle, generation: u64) {
        let coordinator = self.clone();
        let window = self.window;
        handle.spawn(async move {
            tokio::time::sleep(window).await;
            coordinator.settle(generation).await;
        });
    }

    /// The batch the current deadline is waiting on, if one is pending.
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

    /// The most recent settlement lifecycle failure, if one was recorded.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn last_failure(&self) -> Option<MarkSettlementFailure> {
        self.lock().last_failure
    }

    fn record_pass(&self) {
        self.passes.send_modify(|passes| *passes += 1);
    }

    /// Record a lifecycle failure and say so with a stable reason token.
    ///
    /// `RuntimeUnbound` is the one that is ordinary rather than wrong: a CLI run
    /// and a unit test both reach it by construction, so it is a debug line
    /// while the rest are warnings a command-capable owner should never emit.
    fn record_failure(&self, failure: MarkSettlementFailure, targets: &[String]) {
        self.lock().last_failure = Some(failure);
        let reason = failure.as_str();
        let targets = if targets.is_empty() {
            "no marked change".to_string()
        } else {
            targets.join(", ")
        };
        match failure {
            MarkSettlementFailure::RuntimeUnbound => {
                tracing::debug!("Mark settlement is mark-only (reason={reason}): {targets}");
            }
            _ => tracing::warn!("Mark settlement could not complete (reason={reason}): {targets}"),
        }
    }

    fn runtime(&self) -> Option<Arc<dyn MarkSettlementRuntime>> {
        self.lock().runtime.as_ref().and_then(Weak::upgrade)
    }

    /// The bound runtime, or which half of the binding is missing.
    ///
    /// "Never bound" and "bound and then dropped" are two different facts about
    /// a process, and only the second one is a defect in a command-capable
    /// owner. Collapsing them into `Option` is what made both silent.
    fn runtime_or_failure(
        &self,
    ) -> std::result::Result<Arc<dyn MarkSettlementRuntime>, MarkSettlementFailure> {
        let bound = self.lock().runtime.clone();
        match bound {
            None => Err(MarkSettlementFailure::RuntimeUnbound),
            Some(weak) => weak.upgrade().ok_or(MarkSettlementFailure::RuntimeGone),
        }
    }

    /// Run one expired deadline, or exit because it was superseded.
    async fn settle(self: Arc<Self>, generation: u64) {
        let batch = {
            let mut guard = self.lock();
            if guard.generation != generation {
                // A later accepted operator write already merged into this batch
                // and restarted the deadline. Exactly one deadline is live, so
                // this task simply retires and the batch stays pending.
                return;
            }
            guard.pending.take()
        };
        let batch = batch.unwrap_or_default();
        let Some(runtime) = self.runtime() else {
            // The batch has already been taken, so returning quietly here would
            // destroy the operator's accepted intent *and* leave every observer
            // waiting on a pass that can no longer arrive. Report it and close
            // the pass instead.
            self.record_failure(MarkSettlementFailure::RuntimeGone, &batch);
            self.lock().abandoned += 1;
            self.record_pass();
            return;
        };

        // Liveness is re-read at expiry, not trusted from arming time. A finite
        // scheduler that ended in between discards its snapshot here, which is
        // also what keeps the pending deadline from being a termination barrier.
        if !runtime.admits_dynamic_queue() {
            self.lock().abandoned += 1;
            runtime.report_abandoned_settlement(batch).await;
            self.record_pass();
            return;
        }

        let plan = runtime.settle_marks(batch).await;
        // Targets the reducer could not even load carry no decision, so the
        // batch keeps them rather than dropping intent the owner never judged.
        let unreconciled: Vec<String> = plan
            .excluded
            .iter()
            .filter(|(_, reason)| !reason.is_stable())
            .map(|(change_id, _)| change_id.clone())
            .collect();

        let retry = {
            let mut guard = self.lock();
            guard.settled += 1;
            guard.last_plan = Some(plan);
            if unreconciled.is_empty() {
                None
            } else {
                guard.attempts += 1;
                if guard.attempts >= MARK_SETTLEMENT_ATTEMPTS {
                    None
                } else {
                    guard.generation += 1;
                    let pending = guard.pending.get_or_insert_with(Vec::new);
                    for change_id in &unreconciled {
                        if !pending.iter().any(|existing| existing == change_id) {
                            pending.push(change_id.clone());
                        }
                    }
                    Some(guard.generation)
                }
            }
        };

        match retry {
            // The deadline task is re-armed *before* the pass is published, so an
            // observer that wakes on the pass already sees the pending batch.
            Some(generation) => match tokio::runtime::Handle::try_current() {
                Ok(handle) => self.arm(&handle, generation),
                // Unreachable in practice — this method only ever runs on a
                // spawned task — but the batch is deliberately left pending
                // rather than cleared: with no runtime there is nothing left to
                // race it, and clearing it here would also discard targets a
                // concurrent accepted write had merged in.
                Err(_) => {
                    self.record_failure(MarkSettlementFailure::NoTaskRuntime, &unreconciled);
                    runtime
                        .report_settlement_failure(
                            MarkSettlementFailure::NoTaskRuntime,
                            unreconciled,
                        )
                        .await;
                }
            },
            None if !unreconciled.is_empty() => {
                // The budget is spent. Say so with a stable reason rather than
                // leaving a marked row silently outside the queue forever.
                self.record_failure(MarkSettlementFailure::UnreconciledBatch, &unreconciled);
                runtime
                    .report_settlement_failure(
                        MarkSettlementFailure::UnreconciledBatch,
                        unreconciled,
                    )
                    .await;
            }
            None => {}
        }
        self.record_pass();
    }
}

#[cfg(test)]
mod tests;
