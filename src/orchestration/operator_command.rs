//! Shared, frontend-independent operator command service.
//!
//! Frontends (TUI today, remote adapters later) map operator intent onto
//! [`OperatorCommand`] values and call this service. The service owns lifecycle
//! validation and coordinates authoritative reducer transitions with runtime
//! side effects (dynamic queue mutation, per-change cancellation, queue hooks,
//! retry routing) so no frontend has to duplicate that matrix.
//!
//! State axes stay separate on purpose:
//!
//! - execution mark: process-local operator intent ([`ExecutionMarkStore`])
//! - queue intent: reducer-owned membership in the dynamic pending set
//! - activity / wait / terminal: reducer-owned runtime facts
//! - `display_status()`: projection of the reducer-owned axes
//!
//! Execution marks are never written outside the process, so a restart starts
//! with every mark `false` and workflow routing keeps coming from workspace and
//! Git evidence.

// This module is a boundary, not an implementation detail: the TUI adapter uses
// part of it today and the remote frontend adapter will use the rest. Keeping the
// whole boundary defined (and tested) is deliberate, so unused-API warnings from
// the binary crate are allowed here.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::orchestration::state::{OrchestratorState, ReduceOutcome, ReducerCommand};

/// Default bound for waiting on confirmed task termination during stop-and-dequeue.
pub const DEFAULT_CANCELLATION_TIMEOUT: Duration = Duration::from_secs(30);

// ============================================================================
// Display-status vocabulary helpers
// ============================================================================

/// Display statuses that represent active execution.
///
/// `preparing` belongs here even though no agent process is running yet: an
/// admitted change is already creating, recreating, or setting up its managed
/// worktree, so destructive mutation and mark-based intent changes must be
/// refused exactly as they are for an operation in flight.
const ACTIVE_STATUSES: [&str; 6] = [
    "preparing",
    "applying",
    "accepting",
    "rejecting",
    "archiving",
    "resolving",
];

/// Display statuses that are final and cannot be mutated by operator intent.
const FINAL_STATUSES: [&str; 4] = ["archived", "merged", "pushed", "rejected"];

/// Display statuses that only accept mark-only mutation (base-lane waits).
const MARK_ONLY_WAIT_STATUSES: [&str; 2] = ["merge wait", "resolve pending"];

/// Returns true when the display status means Core is actively executing the change.
pub fn is_active_status(display_status: &str) -> bool {
    ACTIVE_STATUSES.contains(&display_status)
}

/// Returns true when the display status is a final outcome.
pub fn is_final_status(display_status: &str) -> bool {
    FINAL_STATUSES.contains(&display_status)
}

// ============================================================================
// Mode and routing
// ============================================================================

/// Frontend-neutral projection of the operator-facing application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorMode {
    /// Pre-run selection.
    Select,
    /// A run is active.
    Running,
    /// A graceful stop was requested but the run has not finished.
    Stopping,
    /// The run stopped and can be resumed.
    Stopped,
    /// The run ended in an error state and requires explicit retry.
    Error,
}

impl OperatorMode {
    /// Parse the canonical `app_mode` token carried by the monitoring snapshot.
    ///
    /// The mapping lives next to the enum rather than in a frontend so every
    /// surface resolves the same token to the same lifecycle mode. An unknown
    /// token falls back to [`OperatorMode::Select`], which is the most
    /// restrictive interpretation that still lets an operator express intent.
    pub fn from_app_mode(app_mode: &str) -> Self {
        match app_mode {
            "running" => Self::Running,
            "stopping" => Self::Stopping,
            "stopped" => Self::Stopped,
            "error" => Self::Error,
            _ => Self::Select,
        }
    }
}

/// How an execution-mark request must be routed for a mode/status pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkRoute {
    /// Mutate the process-local execution mark only; no reducer or queue effect.
    MarkOnly,
    /// Translate the mark into dynamic queue intent (add/remove).
    QueueIntent,
    /// Reject: recovery in this mode is owned by retry commands.
    RetryRequired,
    /// Reject: the row cannot be mutated by mark or queue intent.
    Immutable,
}

/// Decide how an execution-mark request must be handled.
///
/// This is the single lifecycle matrix shared by every frontend. It mirrors the
/// TUI key semantics that existed before the service was introduced, so moving
/// a frontend onto the service cannot silently change operator behavior.
pub fn classify_mark_route(mode: OperatorMode, display_status: &str) -> MarkRoute {
    if is_final_status(display_status) {
        return MarkRoute::Immutable;
    }

    match mode {
        // Error mode never mutates marks: `retry_change` / `retry_errors` own recovery.
        OperatorMode::Error => MarkRoute::RetryRequired,
        // Select mode has no runtime queue yet, so marks are pure operator intent.
        OperatorMode::Select => MarkRoute::MarkOnly,
        // A pending graceful stop is a transition; intent changes wait for it.
        OperatorMode::Stopping => MarkRoute::Immutable,
        OperatorMode::Stopped => {
            if matches!(display_status, "not queued" | "error")
                || MARK_ONLY_WAIT_STATUSES.contains(&display_status)
            {
                MarkRoute::MarkOnly
            } else {
                MarkRoute::Immutable
            }
        }
        OperatorMode::Running => {
            if MARK_ONLY_WAIT_STATUSES.contains(&display_status) {
                return MarkRoute::MarkOnly;
            }
            if is_active_status(display_status) {
                // Active rows are stopped through `StopAndDequeue`, not through marks.
                return MarkRoute::Immutable;
            }
            match display_status {
                "not queued" | "queued" | "error" => MarkRoute::QueueIntent,
                _ => MarkRoute::Immutable,
            }
        }
    }
}

/// Why worktree execution refuses a change, or that it does not.
///
/// Parallel eligibility is two independent workspace observations, and a
/// frontend has to keep them apart: dirty proposal content is something the
/// operator can commit, while a proposal that is simply absent from `HEAD` — an
/// archived change whose managed worktree is still around, for example — has no
/// uncommitted content to commit. Collapsing both into one boolean is what makes
/// a clean row claim a Git working-tree condition it does not have.
///
/// Admission is unchanged by the distinction: every non-[`Eligible`] variant is
/// refused by parallel queueing exactly as before.
///
/// Distinct from [`crate::web::remote_control_api::dto::ParallelEligibility`],
/// which is the wire projection of this same observation.
///
/// [`Eligible`]: ParallelEligibility::Eligible
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParallelEligibility {
    /// The proposal is present in `HEAD` and its directory is clean.
    #[default]
    Eligible,
    /// The proposal directory is absent from the current `HEAD` tree.
    ProposalAbsentFromHead,
    /// Uncommitted or untracked files exist under `openspec/changes/<id>/`.
    UncommittedProposalFiles,
}

impl ParallelEligibility {
    /// Classify one change from a single workspace refresh observation.
    ///
    /// Dirty content wins over absence on purpose: a brand-new proposal is both
    /// untracked and absent from `HEAD`, and committing it is the one action
    /// that resolves both.
    pub fn observe(
        change_id: &str,
        committed_change_ids: &HashSet<String>,
        uncommitted_file_change_ids: &HashSet<String>,
    ) -> Self {
        if uncommitted_file_change_ids.contains(change_id) {
            Self::UncommittedProposalFiles
        } else if !committed_change_ids.contains(change_id) {
            Self::ProposalAbsentFromHead
        } else {
            Self::Eligible
        }
    }

    /// True when the change may take part in parallel execution.
    pub fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }

    /// True when uncommitted or untracked proposal files were actually observed.
    ///
    /// This is the only condition that may be presented as uncommitted state.
    pub fn has_uncommitted_proposal_files(self) -> bool {
        matches!(self, Self::UncommittedProposalFiles)
    }

    /// The bulk-mark exclusion this reason produces; `None` when eligible.
    pub fn mark_exclusion(self) -> Option<MarkExclusion> {
        match self {
            Self::Eligible => None,
            Self::ProposalAbsentFromHead => Some(MarkExclusion::ParallelProposalAbsent),
            Self::UncommittedProposalFiles => Some(MarkExclusion::ParallelIneligible),
        }
    }
}

/// Reason text for intent cleared because worktree execution refuses a change.
///
/// Deliberately reason-agnostic: the cleanup pass clears every ineligible row,
/// so naming one specific cause would mislabel the others.
pub const PARALLEL_INELIGIBLE_CLEANUP_REASON: &str = "not eligible for worktree execution";

/// Why a change is excluded from a bulk execution-mark mutation.
///
/// Every variant is a stable token a frontend branches on rather than prose, so
/// a remote client and the TUI describe the same exclusion the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarkExclusion {
    /// The change reached a final outcome and accepts no operator mutation.
    FinalStatus,
    /// Recovery in this mode is owned by the retry commands.
    RetryRequired,
    /// A graceful stop is in flight; intent changes wait for it.
    StopPending,
    /// The change is executing and must be stopped rather than marked.
    ChangeActive,
    /// The mode/status pair refuses this mutation.
    StatusImmutable,
    /// A change with uncommitted proposal files cannot be queued.
    ParallelIneligible,
    /// A change whose proposal is absent from `HEAD` cannot be queued.
    ParallelProposalAbsent,
}

impl MarkExclusion {
    /// Every exclusion, in the order used when grouping reasons for display.
    pub const ALL: [MarkExclusion; 7] = [
        MarkExclusion::ChangeActive,
        MarkExclusion::ParallelIneligible,
        MarkExclusion::ParallelProposalAbsent,
        MarkExclusion::FinalStatus,
        MarkExclusion::RetryRequired,
        MarkExclusion::StopPending,
        MarkExclusion::StatusImmutable,
    ];

    /// Stable machine-readable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FinalStatus => "final_status",
            Self::RetryRequired => "retry_required",
            Self::StopPending => "stop_pending",
            Self::ChangeActive => "change_active",
            Self::StatusImmutable => "status_immutable",
            Self::ParallelIneligible => "parallel_ineligible",
            Self::ParallelProposalAbsent => "parallel_proposal_absent",
        }
    }

    /// Short operator-facing reason describing what can be done about it.
    pub fn reason(self) -> &'static str {
        match self {
            Self::FinalStatus => "final or rejected and read-only",
            Self::RetryRequired => "in error mode (use retry)",
            Self::StopPending => "waiting for the pending stop",
            Self::ChangeActive => "in progress (use K to stop)",
            Self::StatusImmutable => "not mutable in this mode",
            Self::ParallelIneligible => "uncommitted (commit first)",
            Self::ParallelProposalAbsent => "not present in HEAD (cannot queue)",
        }
    }
}

/// One candidate row for a bulk execution-mark mutation.
///
/// The caller supplies the three facts the decision needs — the reducer's
/// display status, the server-observed parallel eligibility, and the current
/// mark — so the classification itself stays a pure function every frontend can
/// call with its own view of the same state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkTargetRow<'a> {
    /// Target change.
    pub change_id: &'a str,
    /// Reducer-derived display status.
    pub display_status: &'a str,
    /// Server-observed parallel eligibility, with its reason.
    pub parallel_eligibility: ParallelEligibility,
    /// Current execution mark.
    pub marked: bool,
}

/// The classified target set of one bulk execution-mark mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkMarkPlan {
    /// Mark state applied to every eligible row.
    ///
    /// `true` when at least one eligible row is unmarked (mark all), `false`
    /// when every eligible row is already marked (unmark all). Excluded rows
    /// never influence it.
    pub target_state: bool,
    /// Eligible change IDs, in input order.
    pub eligible: Vec<String>,
    /// Excluded change IDs paired with their stable reason, in input order.
    pub excluded: Vec<(String, MarkExclusion)>,
}

impl BulkMarkPlan {
    /// True when no row can be mutated.
    pub fn is_empty(&self) -> bool {
        self.eligible.is_empty()
    }

    /// Grouped exclusion reasons with counts, e.g. `2 rejected and read-only`.
    pub fn exclusion_summary(&self) -> String {
        MarkExclusion::ALL
            .iter()
            .filter_map(|reason| {
                let count = self
                    .excluded
                    .iter()
                    .filter(|(_, actual)| actual == reason)
                    .count();
                (count > 0).then(|| format!("{} {}", count, reason.reason()))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Modes where a bulk execution-mark mutation is meaningful.
///
/// `Error` and `Stopping` are excluded for the same reason the per-change matrix
/// refuses them: recovery is owned by retry, and a pending stop is a transition.
pub fn supports_bulk_marks(mode: OperatorMode) -> bool {
    matches!(
        mode,
        OperatorMode::Select | OperatorMode::Running | OperatorMode::Stopped
    )
}

/// Classify one bulk-mark candidate; `None` means it is part of the target set.
///
/// Eligibility is derived from the same [`classify_mark_route`] matrix a
/// single-row mark request goes through, so a bulk mutation can never touch a
/// row an individual command would refuse.
pub fn classify_bulk_mark_row(
    mode: OperatorMode,
    display_status: &str,
    parallel_eligibility: ParallelEligibility,
) -> Option<MarkExclusion> {
    if !is_final_status(display_status) {
        // The refusal is identical for every ineligible reason; only the reason
        // reported back to the operator differs.
        if let Some(exclusion) = parallel_eligibility.mark_exclusion() {
            return Some(exclusion);
        }
    }

    match classify_mark_route(mode, display_status) {
        MarkRoute::MarkOnly | MarkRoute::QueueIntent => None,
        MarkRoute::RetryRequired => Some(MarkExclusion::RetryRequired),
        MarkRoute::Immutable => Some(if is_final_status(display_status) {
            MarkExclusion::FinalStatus
        } else if matches!(mode, OperatorMode::Stopping) {
            MarkExclusion::StopPending
        } else if is_active_status(display_status) {
            MarkExclusion::ChangeActive
        } else {
            MarkExclusion::StatusImmutable
        }),
    }
}

/// Classify every row once and derive the single shared target mark state.
///
/// One classification pass over one coherent set of rows is what makes a bulk
/// mutation atomic in meaning: the target state cannot shift halfway through
/// because a row was re-read at a different instant.
pub fn plan_bulk_marks(mode: OperatorMode, rows: &[MarkTargetRow<'_>]) -> BulkMarkPlan {
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    let mut any_unmarked = false;

    if !supports_bulk_marks(mode) {
        return BulkMarkPlan {
            target_state: false,
            eligible,
            excluded,
        };
    }

    for row in rows {
        match classify_bulk_mark_row(mode, row.display_status, row.parallel_eligibility) {
            Some(reason) => excluded.push((row.change_id.to_string(), reason)),
            None => {
                any_unmarked |= !row.marked;
                eligible.push(row.change_id.to_string());
            }
        }
    }

    BulkMarkPlan {
        // If any eligible row is unmarked, mark them all; otherwise unmark them all.
        target_state: any_unmarked,
        eligible,
        excluded,
    }
}

/// Changes whose mark and queue presentation an eligibility refresh must clear.
///
/// Shared so every frontend cleans up exactly the same rows: an ineligible
/// change that carries operator intent — a mark, a queue intent, or both —
/// cannot stay in a target set worktree execution would refuse to start.
pub fn parallel_cleanup_targets(rows: &[ParallelCleanupRow<'_>]) -> Vec<String> {
    rows.iter()
        .filter(|row| !row.parallel_eligible && (row.marked || row.queued))
        .map(|row| row.change_id.to_string())
        .collect()
}

/// One candidate row for the parallel-mode cleanup pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParallelCleanupRow<'a> {
    /// Target change.
    pub change_id: &'a str,
    /// True when the change may take part in parallel execution.
    pub parallel_eligible: bool,
    /// Current execution mark.
    pub marked: bool,
    /// True when the change currently carries queue intent or presentation.
    pub queued: bool,
}

// ============================================================================
// Parallel runtime (process-local)
// ============================================================================

/// Worktree runtime facts for this process incarnation.
///
/// One store, shared by every frontend. Worktree eligibility is derived from
/// workspace observation that only the frontend running the refresh loop
/// performs; publishing it here is what lets a keypress and a remote command
/// read the *same* value instead of each keeping a copy that can drift.
///
/// Nothing here is durable: a restart re-observes the workspace.
#[derive(Debug, Default)]
pub struct ParallelRuntime {
    inner: Mutex<ParallelRuntimeInner>,
    /// Serializes whole operator mutations, not individual field accesses.
    ///
    /// `inner` makes one read or one write atomic; it cannot make a *sequence*
    /// atomic. A bulk mark classifies against one observation and then awaits
    /// the reducer and the queue; holding this guard for the whole mutation is
    /// what keeps an interleaved mutation from re-marking a row another one
    /// just cleared.
    mutations: tokio::sync::Mutex<()>,
}

#[derive(Debug, Default)]
struct ParallelRuntimeInner {
    max_concurrent: usize,
    vcs_backend: String,
    /// Ineligible changes only, each mapped to the reason it is refused.
    ///
    /// Absence means eligible, so the map is the reason set and the membership
    /// set at once and the two can never disagree.
    ineligible: HashMap<String, ParallelEligibility>,
}

/// A coherent read of the worktree runtime facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParallelRuntimeFacts {
    /// Maximum number of concurrently executing changes.
    pub max_concurrent: usize,
    /// VCS backend the run would use.
    pub vcs_backend: String,
}

impl ParallelRuntime {
    /// Create an empty projection (nothing excluded).
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ParallelRuntimeInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Publish the configured maximum concurrency.
    pub fn set_max_concurrent(&self, max_concurrent: usize) {
        self.lock().max_concurrent = max_concurrent;
    }

    /// Publish the VCS backend a run would use.
    pub fn set_vcs_backend(&self, backend: impl Into<String>) {
        self.lock().vcs_backend = backend.into();
    }

    /// Publish the changes worktree execution refuses to start, each with its reason.
    ///
    /// [`ParallelEligibility::Eligible`] entries are dropped rather than stored:
    /// an "ineligible" entry that claims eligibility would be a contradiction the
    /// readers below would have to re-check.
    pub fn set_parallel_ineligible(
        &self,
        entries: impl IntoIterator<Item = (String, ParallelEligibility)>,
    ) {
        self.lock().ineligible = entries
            .into_iter()
            .filter(|(_, eligibility)| !eligibility.is_eligible())
            .collect();
    }

    /// True when the change may take part in worktree execution.
    pub fn is_eligible(&self, change_id: &str) -> bool {
        !self.lock().ineligible.contains_key(change_id)
    }

    /// The observed eligibility of one change, including why it is refused.
    pub fn eligibility(&self, change_id: &str) -> ParallelEligibility {
        self.lock()
            .ineligible
            .get(change_id)
            .copied()
            .unwrap_or_default()
    }

    /// Every change worktree execution refuses, sorted for deterministic output.
    pub fn ineligible_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.lock().ineligible.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// One coherent read of every published runtime fact.
    pub fn facts(&self) -> ParallelRuntimeFacts {
        let guard = self.lock();
        ParallelRuntimeFacts {
            max_concurrent: guard.max_concurrent,
            vcs_backend: guard.vcs_backend.clone(),
        }
    }

    /// Take the shared guard for one indivisible operator mutation.
    ///
    /// Held for the entire mutation, across every await inside it. Nothing on
    /// the read path takes it, so publishing facts and rendering a snapshot are
    /// never blocked by a mutation in flight.
    pub async fn lock_mutations(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mutations.lock().await
    }

    /// Targets that worktree execution refuses, in request order.
    pub fn rejected(&self, targets: &[String]) -> Vec<String> {
        let guard = self.lock();
        targets
            .iter()
            .filter(|id| guard.ineligible.contains_key(*id))
            .cloned()
            .collect()
    }
}

/// Where a retry request must be routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryRoute {
    /// A recoverable terminal error: use `ReducerCommand::RetryError`.
    TerminalError,
    /// A resumable acceptance hold: resume acceptance via the explicit-retry run path.
    AcceptanceStall,
}

/// Decide the retry route for a change from its reducer-derived status and
/// blocker kind.
///
/// `blocked` is not retryable on its own: a dependency wait clears when the
/// dependency completes, while a validated external prerequisite wait is
/// explicitly retryable so the blocked phase can run again and supply fresh
/// classification evidence.
///
/// Returns `None` when the status carries no retryable evidence.
pub fn classify_retry_route(
    display_status: &str,
    blocker_kind: crate::orchestration::state::BlockerKind,
) -> Option<RetryRoute> {
    use crate::orchestration::state::BlockerKind;
    match (display_status, blocker_kind) {
        ("error", _) => Some(RetryRoute::TerminalError),
        ("stalled", _) => Some(RetryRoute::AcceptanceStall),
        ("blocked", BlockerKind::External) => Some(RetryRoute::AcceptanceStall),
        _ => None,
    }
}

// ============================================================================
// Commands, outcomes, errors
// ============================================================================

/// Frontend-independent operator intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorCommand {
    /// Set the process-local execution mark for a change.
    SetExecutionMark {
        /// Target change.
        change_id: String,
        /// Requested mark value.
        marked: bool,
    },
    /// Add a change to the dynamic queue.
    AddToQueue {
        /// Target change.
        change_id: String,
    },
    /// Remove a change from the dynamic queue.
    RemoveFromQueue {
        /// Target change.
        change_id: String,
    },
    /// Stop an in-flight change and dequeue it once termination is confirmed.
    StopAndDequeue {
        /// Target change.
        change_id: String,
    },
    /// Retry a single change using its reconciled evidence.
    RetryChange {
        /// Target change.
        change_id: String,
    },
    /// Apply one derived execution-mark state to every eligible change.
    SetAllExecutionMarks,
}

/// Which direction a dynamic queue mutation went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueMutation {
    /// The change was added.
    Added,
    /// The change was removed.
    Removed,
}

/// Result of a queue-intent command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueOutcome {
    /// Target change.
    pub change_id: String,
    /// Requested direction.
    pub mutation: QueueMutation,
    /// True when the reducer accepted the intent change.
    pub reducer_changed: bool,
    /// True when the runtime dynamic queue really changed.
    ///
    /// Queue hooks run exactly once when, and only when, this is true.
    pub dynamic_queue_mutated: bool,
    /// Display status after the command.
    pub display_status: String,
}

/// Why a command produced no state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoOpReason {
    /// The execution mark already had the requested value.
    MarkUnchanged,
    /// The reducer rejected the intent in the current lifecycle state.
    ReducerRejected,
    /// Every eligible row already carried the derived target mark.
    BulkMarksUnchanged,
    /// No row was eligible for the bulk mutation.
    NoEligibleMarkTarget,
}

/// What a successful command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorOutcome {
    /// The process-local execution mark changed.
    MarkSet {
        /// Target change.
        change_id: String,
        /// New mark value.
        marked: bool,
    },
    /// A queue-intent command completed.
    Queue(QueueOutcome),
    /// An active change was cancelled, confirmed terminated, and dequeued.
    Dequeued {
        /// Target change.
        change_id: String,
    },
    /// A retry was accepted and routed.
    Retry(RetryPlan),
    /// A bulk execution-mark mutation completed.
    BulkMarks {
        /// The single target state applied to every eligible row.
        marked: bool,
        /// Changes whose mark or queue intent actually changed, in plan order.
        changed: Vec<String>,
        /// Rows the plan excluded, with their stable reason, in plan order.
        excluded: Vec<(String, MarkExclusion)>,
    },
    /// Nothing changed.
    NoOp {
        /// Target change (empty for bulk commands).
        change_id: String,
        /// Why nothing changed.
        reason: NoOpReason,
    },
}

/// Routing decision for a retry request.
///
/// The service decides routing and applies the reducer transition; starting or
/// waking a run stays with the caller so the existing scheduler ownership model
/// is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPlan {
    /// Changes that were accepted for retry, in request order.
    pub change_ids: Vec<String>,
    /// Route per accepted change, in the same order as `change_ids`.
    pub routes: Vec<RetryRoute>,
    /// True when the run must be started with explicit-retry semantics so a
    /// reconciled acceptance hold is consumed and acceptance resumes.
    pub explicit_retry: bool,
}

impl RetryPlan {
    /// True when no change was accepted for retry.
    pub fn is_empty(&self) -> bool {
        self.change_ids.is_empty()
    }
}

/// Why a command was rejected without side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorCommandError {
    /// Mark mutation is not allowed for this mode/status pair.
    MarkNotAllowed {
        /// Target change.
        change_id: String,
        /// Mode the request arrived in.
        mode: OperatorMode,
        /// Route that rejected the request.
        route: MarkRoute,
        /// Display status at rejection time.
        display_status: String,
    },
    /// The change is active but has no registered cancellation handle.
    MissingCancellationHandle {
        /// Target change.
        change_id: String,
    },
    /// Cancellation could not be issued.
    CancellationFailed {
        /// Target change.
        change_id: String,
        /// Failure detail from the runtime.
        message: String,
    },
    /// Termination was not confirmed within the bound.
    TerminationTimeout {
        /// Target change.
        change_id: String,
        /// How long the service waited.
        waited: Duration,
    },
    /// Retry is not supported for the change's current evidence.
    RetryUnsupported {
        /// Target change.
        change_id: String,
        /// Display status that carries no retryable evidence.
        display_status: String,
    },
    /// Bulk execution-mark mutation is not available in this mode.
    BulkMarksNotAllowed {
        /// Mode the request arrived in.
        mode: OperatorMode,
    },
}

impl std::fmt::Display for OperatorCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarkNotAllowed {
                change_id,
                mode,
                route,
                display_status,
            } => write!(
                f,
                "execution mark for '{change_id}' is not allowed in {mode:?} mode \
                 (status '{display_status}', route {route:?})"
            ),
            // Inline workspace preparation is the common way to reach this: the
            // worktree is being created or `.wt/setup` is running, and neither
            // is killable. The stop request itself is still recorded, so saying
            // only "refused" would understate what actually happened.
            Self::MissingCancellationHandle { change_id } => write!(
                f,
                "no cancellation handle registered for active change '{change_id}'; \
                 the stop request is recorded and takes effect before the next operation starts"
            ),
            Self::CancellationFailed { change_id, message } => {
                write!(f, "cancellation failed for '{change_id}': {message}")
            }
            Self::TerminationTimeout { change_id, waited } => write!(
                f,
                "termination of '{change_id}' was not confirmed within {waited:?}"
            ),
            Self::RetryUnsupported {
                change_id,
                display_status,
            } => write!(
                f,
                "retry is not supported for '{change_id}' with status '{display_status}'"
            ),
            Self::BulkMarksNotAllowed { mode } => write!(
                f,
                "bulk execution-mark mutation is not available in {mode:?} mode"
            ),
        }
    }
}

/// Result alias for operator command execution.
pub type OperatorResult<T> = std::result::Result<T, OperatorCommandError>;

// ============================================================================
// Execution marks (process-local)
// ============================================================================

/// Process-local store of execution marks.
///
/// Marks express "the operator wants this change considered at the next
/// applicable boundary". They are never persisted, so a new process starts with
/// every mark `false` by construction and marks can never become durable
/// workflow-control evidence.
#[derive(Debug, Default)]
pub struct ExecutionMarkStore {
    marks: Mutex<HashSet<String>>,
}

impl ExecutionMarkStore {
    /// Create an empty store. A restarted process always begins here.
    pub fn new() -> Self {
        Self::default()
    }

    /// True when the change currently carries an execution mark.
    pub fn is_marked(&self, change_id: &str) -> bool {
        self.lock().contains(change_id)
    }

    /// Set the mark for a change. Returns true when the value changed.
    pub fn set(&self, change_id: &str, marked: bool) -> bool {
        let mut guard = self.lock();
        if marked {
            guard.insert(change_id.to_string())
        } else {
            guard.remove(change_id)
        }
    }

    /// Replace the whole mark set (used by frontends that own a mark projection).
    pub fn replace(&self, change_ids: impl IntoIterator<Item = String>) {
        *self.lock() = change_ids.into_iter().collect();
    }

    /// All marked change IDs, sorted for deterministic output.
    pub fn marked_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.lock().iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Drop every mark.
    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.marks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

// ============================================================================
// Ports
// ============================================================================

/// Waiter that completes once an executor confirms a change's task exited.
#[derive(Clone, Debug)]
pub struct TerminationWaiter {
    done: CancellationToken,
}

impl TerminationWaiter {
    /// Wrap the `done` token an executor cancels at task completion.
    pub fn new(done: CancellationToken) -> Self {
        Self { done }
    }

    /// A waiter that is already satisfied.
    pub fn already_terminated() -> Self {
        let done = CancellationToken::new();
        done.cancel();
        Self { done }
    }

    /// A waiter that never completes (used to exercise timeout handling).
    pub fn never() -> Self {
        Self {
            done: CancellationToken::new(),
        }
    }

    /// Wait until termination is confirmed.
    pub async fn wait(&self) {
        self.done.cancelled().await;
    }
}

/// Runtime queue operations the service coordinates.
#[async_trait]
pub trait QueuePort: Send + Sync {
    /// Add a change to the runtime queue. Returns true when the queue really changed.
    async fn add(&self, change_id: &str) -> bool;

    /// Remove a change from the runtime queue. Returns true when the queue really changed.
    async fn remove(&self, change_id: &str) -> bool;

    /// Issue cancellation for a change.
    ///
    /// `Ok(None)` means no cancellation handle is registered. `Err` means the
    /// cancellation request itself failed and no termination will follow.
    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String>;

    /// Wake the scheduler without changing queue contents.
    async fn notify_scheduler(&self);

    /// Publish a target-ID-bearing one-shot explicit-retry edge to a live scheduler.
    ///
    /// Only an accepted, state-changing `ReducerCommand::RetryError` reaches
    /// here. A refused or no-op retry, an ordinary `AddToQueue`, and a generic
    /// queue notification deliberately do not: retry intent is what releases a
    /// change's ephemeral failed classification, and nothing else may look like
    /// it. The default implementation drops the edge, which is correct for
    /// ports without a live scheduler behind them.
    async fn publish_explicit_retry(&self, _change_id: &str) {}
}

/// Queue hook dispatch, isolated so the service can be verified without
/// executing user commands.
#[async_trait]
pub trait QueueHookPort: Send + Sync {
    /// Run `on_queue_add` for a completed dynamic addition.
    async fn on_queue_add(&self, change_id: &str);

    /// Run `on_queue_remove` for a completed dynamic removal.
    async fn on_queue_remove(&self, change_id: &str);
}

/// Hook port that runs nothing (CLI/headless callers without hook config).
pub struct NoopQueueHooks;

#[async_trait]
impl QueueHookPort for NoopQueueHooks {
    async fn on_queue_add(&self, _change_id: &str) {}
    async fn on_queue_remove(&self, _change_id: &str) {}
}

/// Hook port backed by the real configured [`crate::hooks::HookRunner`].
pub struct HookRunnerQueueHooks {
    runner: crate::hooks::HookRunner,
}

impl HookRunnerQueueHooks {
    /// Wrap a configured hook runner.
    pub fn new(runner: crate::hooks::HookRunner) -> Self {
        Self { runner }
    }

    async fn run(&self, hook_type: crate::hooks::HookType, change_id: &str) {
        let context = crate::hooks::HookContext::new(0, 0, 0, false).with_change(change_id, 0, 0);
        if let Err(error) = self.runner.run_hook(hook_type, &context).await {
            tracing::warn!("{hook_type} hook failed for '{change_id}': {error}");
        }
    }
}

#[async_trait]
impl QueueHookPort for HookRunnerQueueHooks {
    async fn on_queue_add(&self, change_id: &str) {
        self.run(crate::hooks::HookType::OnQueueAdd, change_id)
            .await;
    }

    async fn on_queue_remove(&self, change_id: &str) {
        self.run(crate::hooks::HookType::OnQueueRemove, change_id)
            .await;
    }
}

// ============================================================================
// Service
// ============================================================================

/// Process-local application service for operator commands.
pub struct OperatorCommandService {
    state: Arc<RwLock<OrchestratorState>>,
    queue: Arc<dyn QueuePort>,
    hooks: Arc<dyn QueueHookPort>,
    marks: Arc<ExecutionMarkStore>,
    parallel: Arc<ParallelRuntime>,
    cancellation_timeout: Duration,
}

impl OperatorCommandService {
    /// Build a service over the shared reducer state and runtime ports.
    ///
    /// The parallel runtime defaults to an unshared empty projection; a process
    /// that has one binds it with [`Self::with_parallel`] so the toggle the
    /// start guard reads and the toggle this service mutates are one value.
    pub fn new(
        state: Arc<RwLock<OrchestratorState>>,
        queue: Arc<dyn QueuePort>,
        hooks: Arc<dyn QueueHookPort>,
        marks: Arc<ExecutionMarkStore>,
    ) -> Self {
        Self {
            state,
            queue,
            hooks,
            marks,
            parallel: Arc::new(ParallelRuntime::new()),
            cancellation_timeout: DEFAULT_CANCELLATION_TIMEOUT,
        }
    }

    /// Bind the shared parallel runtime store.
    pub fn with_parallel(mut self, parallel: Arc<ParallelRuntime>) -> Self {
        self.parallel = parallel;
        self
    }

    /// Override the bound used when waiting for confirmed task termination.
    pub fn with_cancellation_timeout(mut self, timeout: Duration) -> Self {
        self.cancellation_timeout = timeout;
        self
    }

    /// Shared process-local execution marks.
    pub fn marks(&self) -> Arc<ExecutionMarkStore> {
        self.marks.clone()
    }

    /// Shared process-local parallel runtime facts.
    pub fn parallel(&self) -> Arc<ParallelRuntime> {
        self.parallel.clone()
    }

    /// Current display status for a change.
    pub async fn display_status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    /// Execute a typed operator command.
    pub async fn execute(
        &self,
        mode: OperatorMode,
        command: OperatorCommand,
    ) -> OperatorResult<OperatorOutcome> {
        match command {
            OperatorCommand::SetExecutionMark { change_id, marked } => {
                self.set_execution_mark(mode, &change_id, marked).await
            }
            OperatorCommand::AddToQueue { change_id } => self
                .add_to_queue(&change_id)
                .await
                .map(OperatorOutcome::Queue),
            OperatorCommand::RemoveFromQueue { change_id } => self
                .remove_from_queue(&change_id)
                .await
                .map(OperatorOutcome::Queue),
            OperatorCommand::StopAndDequeue { change_id } => {
                self.stop_and_dequeue(&change_id).await
            }
            OperatorCommand::RetryChange { change_id } => self
                .retry_change(&change_id)
                .await
                .map(OperatorOutcome::Retry),
            OperatorCommand::SetAllExecutionMarks => self.set_all_execution_marks(mode).await,
        }
    }

    /// Apply an execution-mark request through the shared lifecycle matrix.
    pub async fn set_execution_mark(
        &self,
        mode: OperatorMode,
        change_id: &str,
        marked: bool,
    ) -> OperatorResult<OperatorOutcome> {
        let display_status = self.display_status(change_id).await;
        match classify_mark_route(mode, &display_status) {
            MarkRoute::MarkOnly => {
                if self.marks.set(change_id, marked) {
                    Ok(OperatorOutcome::MarkSet {
                        change_id: change_id.to_string(),
                        marked,
                    })
                } else {
                    Ok(OperatorOutcome::NoOp {
                        change_id: change_id.to_string(),
                        reason: NoOpReason::MarkUnchanged,
                    })
                }
            }
            MarkRoute::QueueIntent => {
                let outcome = if marked {
                    self.add_to_queue(change_id).await?
                } else {
                    self.remove_from_queue(change_id).await?
                };
                self.marks.set(change_id, marked);
                Ok(OperatorOutcome::Queue(outcome))
            }
            route @ (MarkRoute::RetryRequired | MarkRoute::Immutable) => {
                Err(OperatorCommandError::MarkNotAllowed {
                    change_id: change_id.to_string(),
                    mode,
                    route,
                    display_status,
                })
            }
        }
    }

    /// Apply one derived execution-mark state to every eligible change.
    ///
    /// The whole operation is classified from a single read of the reducer, so
    /// the target state comes from one coherent view and every eligible row
    /// receives the identical mark. Excluded rows keep whatever intent they
    /// already had and are reported with a stable reason instead of being
    /// silently skipped.
    ///
    /// Classification and application are one mutation under the shared guard,
    /// so the toggle this reads cannot move — and its cleanup cannot run —
    /// while the plan derived from it is still being applied.
    pub async fn set_all_execution_marks(
        &self,
        mode: OperatorMode,
    ) -> OperatorResult<OperatorOutcome> {
        if !supports_bulk_marks(mode) {
            return Err(OperatorCommandError::BulkMarksNotAllowed { mode });
        }

        let _mutation = self.parallel.lock_mutations().await;

        // One read, one classification: re-reading per row could observe two
        // different instants and derive a target state from neither.
        let observed: Vec<(String, String, ParallelEligibility, bool)> = {
            let guard = self.state.read().await;
            guard
                .tracked_change_ids()
                .into_iter()
                .map(|change_id| {
                    let display_status = guard.display_status(&change_id).to_string();
                    let eligibility = self.parallel.eligibility(&change_id);
                    let marked = self.marks.is_marked(&change_id);
                    (change_id, display_status, eligibility, marked)
                })
                .collect()
        };

        let rows: Vec<MarkTargetRow<'_>> = observed
            .iter()
            .map(
                |(change_id, display_status, parallel_eligibility, marked)| MarkTargetRow {
                    change_id,
                    display_status,
                    parallel_eligibility: *parallel_eligibility,
                    marked: *marked,
                },
            )
            .collect();
        let plan = plan_bulk_marks(mode, &rows);

        if plan.is_empty() {
            return Ok(OperatorOutcome::NoOp {
                change_id: String::new(),
                reason: NoOpReason::NoEligibleMarkTarget,
            });
        }

        let mut changed = Vec::new();
        for change_id in &plan.eligible {
            let display_status = observed
                .iter()
                .find(|(id, ..)| id == change_id)
                .map(|(_, status, ..)| status.as_str())
                .unwrap_or("not queued");
            // The route comes from the classified snapshot, not from a second
            // read, so a status that moved mid-operation cannot reroute a row
            // the plan already accepted.
            match classify_mark_route(mode, display_status) {
                MarkRoute::MarkOnly => {
                    if self.marks.set(change_id, plan.target_state) {
                        changed.push(change_id.clone());
                    }
                }
                MarkRoute::QueueIntent => {
                    let outcome = if plan.target_state {
                        self.add_to_queue(change_id).await?
                    } else {
                        self.remove_from_queue(change_id).await?
                    };
                    let mark_changed = self.marks.set(change_id, plan.target_state);
                    if mark_changed || outcome.reducer_changed || outcome.dynamic_queue_mutated {
                        changed.push(change_id.clone());
                    }
                }
                // `plan_bulk_marks` only admits the two routes above.
                MarkRoute::RetryRequired | MarkRoute::Immutable => {}
            }
        }

        if changed.is_empty() {
            return Ok(OperatorOutcome::NoOp {
                change_id: String::new(),
                reason: NoOpReason::BulkMarksUnchanged,
            });
        }

        Ok(OperatorOutcome::BulkMarks {
            marked: plan.target_state,
            changed,
            excluded: plan.excluded,
        })
    }

    /// Add a change to the dynamic queue.
    ///
    /// A dependency-ineligible change keeps its queue intent: dependency
    /// blocking is reported later as `blocked` display status, never by
    /// rejecting the operator's request.
    pub async fn add_to_queue(&self, change_id: &str) -> OperatorResult<QueueOutcome> {
        let (reduce_outcome, was_error_retry) = {
            let mut guard = self.state.write().await;
            if guard.is_terminal_error_change(change_id) {
                (
                    guard.apply_command(ReducerCommand::RetryError(change_id.to_string())),
                    true,
                )
            } else {
                (
                    guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string())),
                    false,
                )
            }
        };
        let reducer_changed = matches!(reduce_outcome, ReduceOutcome::Changed(_));

        // Only the `RetryError` half of this branch is an explicit retry, and
        // only when the reducer actually changed state. An ordinary `AddToQueue`
        // never releases a failed classification.
        if was_error_retry && reducer_changed {
            self.queue.publish_explicit_retry(change_id).await;
        }

        // Effect before commit: hooks describe real runtime mutations only.
        let dynamic_queue_mutated = if reducer_changed {
            self.queue.add(change_id).await
        } else {
            false
        };
        if dynamic_queue_mutated {
            // Wake the scheduler so newly queued work is reconsidered immediately,
            // then run the hook for the mutation that actually happened.
            self.queue.notify_scheduler().await;
            self.hooks.on_queue_add(change_id).await;
        }

        Ok(QueueOutcome {
            change_id: change_id.to_string(),
            mutation: QueueMutation::Added,
            reducer_changed,
            dynamic_queue_mutated,
            display_status: self.display_status(change_id).await,
        })
    }

    /// Remove a change from the dynamic queue.
    pub async fn remove_from_queue(&self, change_id: &str) -> OperatorResult<QueueOutcome> {
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::RemoveFromQueue(change_id.to_string()))
        };
        let reducer_changed = matches!(reduce_outcome, ReduceOutcome::Changed(_));

        // A removal is a real dynamic mutation when the change actually left the
        // pending set: either it was sitting in the dynamic queue, or the reducer
        // accepted a Queued -> NotQueued intent transition. A duplicate removal
        // satisfies neither and must not run the hook.
        let removed_from_dynamic_queue = self.queue.remove(change_id).await;
        let dynamic_queue_mutated = reducer_changed || removed_from_dynamic_queue;
        if dynamic_queue_mutated {
            self.hooks.on_queue_remove(change_id).await;
        }

        Ok(QueueOutcome {
            change_id: change_id.to_string(),
            mutation: QueueMutation::Removed,
            reducer_changed,
            dynamic_queue_mutated,
            display_status: self.display_status(change_id).await,
        })
    }

    /// Stop an in-flight change and dequeue it only after confirmed termination.
    ///
    /// Ordering is validate, cancel, confirm, commit. A missing handle, a failed
    /// cancellation, or a confirmation timeout leaves the active reducer state
    /// untouched.
    pub async fn stop_and_dequeue(&self, change_id: &str) -> OperatorResult<OperatorOutcome> {
        let display_status = self.display_status(change_id).await;
        let was_active = is_active_status(&display_status);

        let waiter = match self.queue.request_cancellation(change_id).await {
            Err(message) => {
                return Err(OperatorCommandError::CancellationFailed {
                    change_id: change_id.to_string(),
                    message,
                })
            }
            Ok(Some(waiter)) => waiter,
            Ok(None) if was_active => {
                // An active change without a handle cannot be proven terminated,
                // so dequeue must not be applied.
                return Err(OperatorCommandError::MissingCancellationHandle {
                    change_id: change_id.to_string(),
                });
            }
            // Idle/queued rows have no task to terminate.
            Ok(None) => TerminationWaiter::already_terminated(),
        };

        if tokio::time::timeout(self.cancellation_timeout, waiter.wait())
            .await
            .is_err()
        {
            return Err(OperatorCommandError::TerminationTimeout {
                change_id: change_id.to_string(),
                waited: self.cancellation_timeout,
            });
        }

        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::DequeueChange(change_id.to_string()))
        };
        if matches!(reduce_outcome, ReduceOutcome::NoOp) {
            return Ok(OperatorOutcome::NoOp {
                change_id: change_id.to_string(),
                reason: NoOpReason::ReducerRejected,
            });
        }
        self.marks.set(change_id, false);

        Ok(OperatorOutcome::Dequeued {
            change_id: change_id.to_string(),
        })
    }

    /// Record operator intent to resolve a merge for a change.
    ///
    /// The reducer owns whether the intent is valid for the change's current
    /// wait state, so a frontend never has to decide that itself. Returns true
    /// when the reducer accepted the intent.
    pub async fn resolve_merge(&self, change_id: &str) -> bool {
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()))
        };
        matches!(reduce_outcome, ReduceOutcome::Changed(_))
    }

    /// Reducer-derived blocker kind for a change.
    async fn blocker_kind(&self, change_id: &str) -> crate::orchestration::state::BlockerKind {
        self.state
            .read()
            .await
            .change_runtime(change_id)
            .map(crate::orchestration::state::ChangeRuntimeState::blocker_kind)
            .unwrap_or_default()
    }

    /// Route a retry request for one change.
    pub async fn retry_change(&self, change_id: &str) -> OperatorResult<RetryPlan> {
        let display_status = self.display_status(change_id).await;
        let blocker_kind = self.blocker_kind(change_id).await;
        let Some(route) = classify_retry_route(&display_status, blocker_kind) else {
            return Err(OperatorCommandError::RetryUnsupported {
                change_id: change_id.to_string(),
                display_status,
            });
        };
        let plan = self.apply_retry_route(change_id, route).await;
        Ok(plan)
    }

    /// Route a bulk retry request.
    ///
    /// Changes without retryable evidence are skipped rather than rejected, so a
    /// bulk retry never consumes an unsupported or identity-mismatched hold.
    pub async fn retry_errors(&self, change_ids: &[String]) -> RetryPlan {
        let mut plan = RetryPlan {
            change_ids: Vec::new(),
            routes: Vec::new(),
            explicit_retry: false,
        };
        for change_id in change_ids {
            let display_status = self.display_status(change_id).await;
            let blocker_kind = self.blocker_kind(change_id).await;
            let Some(route) = classify_retry_route(&display_status, blocker_kind) else {
                continue;
            };
            let accepted = self.apply_retry_route(change_id, route).await;
            plan.change_ids.extend(accepted.change_ids);
            plan.routes.extend(accepted.routes);
            plan.explicit_retry |= accepted.explicit_retry;
        }
        plan
    }

    async fn apply_retry_route(&self, change_id: &str, route: RetryRoute) -> RetryPlan {
        let command = match route {
            RetryRoute::TerminalError => ReducerCommand::RetryError(change_id.to_string()),
            // An in-memory acceptance hold resumes through the explicit-retry run
            // path; the reducer only has to restore ordinary queue intent. A
            // non-resumable hold is refused so its blocker evidence survives and
            // no ambiguous work is dispatched.
            RetryRoute::AcceptanceStall => {
                let refuse = {
                    let guard = self.state.read().await;
                    guard.change_runtime(change_id).is_some_and(|rt| {
                        rt.is_acceptance_stalled() && !rt.is_resumable_acceptance_stall()
                    })
                };
                if refuse {
                    tracing::warn!(
                        change_id = %change_id,
                        "Explicit retry refused: the acceptance stall is not resumable, so its \
                         blocker evidence is retained"
                    );
                    return RetryPlan {
                        change_ids: Vec::new(),
                        routes: Vec::new(),
                        explicit_retry: false,
                    };
                }
                ReducerCommand::AddToQueue(change_id.to_string())
            }
        };
        let is_error_retry = matches!(command, ReducerCommand::RetryError(_));
        let reduce_outcome = {
            let mut guard = self.state.write().await;
            guard.apply_command(command)
        };
        if matches!(reduce_outcome, ReduceOutcome::NoOp) {
            return RetryPlan {
                change_ids: Vec::new(),
                routes: Vec::new(),
                explicit_retry: false,
            };
        }
        // A live scheduler holds an ephemeral failed classification for this
        // change that only an accepted, state-changing `RetryError` may release.
        // The acceptance-stall route restores ordinary queue intent instead and
        // deliberately publishes nothing.
        if is_error_retry {
            self.queue.publish_explicit_retry(change_id).await;
        }
        self.marks.set(change_id, true);
        RetryPlan {
            change_ids: vec![change_id.to_string()],
            routes: vec![route],
            // Both routes must start the run with explicit-retry semantics: it
            // releases the repair budget and lets a valid acceptance hold resume
            // acceptance instead of rerunning apply.
            explicit_retry: true,
        }
    }
}

// ============================================================================
// DynamicQueue adapter
// ============================================================================

#[async_trait]
impl QueuePort for crate::tui::queue::DynamicQueue {
    async fn add(&self, change_id: &str) -> bool {
        self.push(change_id.to_string()).await
    }

    async fn remove(&self, change_id: &str) -> bool {
        let removed_from_queue = crate::tui::queue::DynamicQueue::remove(self, change_id).await;
        // Always record the pending removal so the scheduler drops the change from
        // its own pending set, even when it was not sitting in the dynamic queue.
        self.mark_removed(change_id.to_string()).await;
        removed_from_queue
    }

    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        Ok(
            crate::tui::queue::DynamicQueue::request_cancellation(self, change_id)
                .await
                .map(TerminationWaiter::new),
        )
    }

    async fn notify_scheduler(&self) {
        crate::tui::queue::DynamicQueue::notify_scheduler(self);
    }

    async fn publish_explicit_retry(&self, change_id: &str) {
        crate::tui::queue::DynamicQueue::publish_explicit_retry(self, change_id.to_string()).await;
    }
}

#[cfg(test)]
mod tests;
