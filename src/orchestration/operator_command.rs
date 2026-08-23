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

use crate::orchestration::apply_commit_evidence::ApplyCommitEvidence;
use crate::orchestration::execution_facts::{project_phase, ExecutionFactsStore, ExecutionPhase};
use crate::orchestration::mark_settlement::{
    classify_mark_settlement_row, plan_mark_settlement, MarkSettlementAction,
    MarkSettlementCoordinator, MarkSettlementExclusion, MarkSettlementPlan, MarkSettlementRow,
};
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
///
/// This is the single backing vocabulary for [`is_active_status`], and it is
/// public so callers that must be exhaustive over active execution — notably
/// the TUI refresh precedence rule — can iterate it instead of maintaining a
/// second hand-written list that silently falls behind this one.
pub const ACTIVE_STATUSES: [&str; 6] = [
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
// Run boundary liveness
// ============================================================================

/// Whether the scheduler task that owns the current active-run state is alive.
///
/// Observability only. It reports `scheduler_running` on the execution-status
/// resource and tells a frontend whether an existing boundary can be notified
/// or a new one has to be started — it is deliberately *not* an operator-action
/// gate. A retained
/// [`crate::orchestration::state::ApplyIterationLimit`] record answers why one
/// invocation stopped; a later explicit retry is admitted on the target's own
/// terminal-error evidence, so owner lifetime never decides whether an operator
/// may act.
///
/// A process with no command-capable boundary (headless `cflx run`) binds
/// nothing here and reports no live scheduler.
pub trait RunBoundaryLiveness: Send + Sync {
    /// True when the scheduler task owning the current active-run state is live.
    fn boundary_running(&self) -> bool;
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

    /// The canonical `app_mode` token for this lifecycle mode.
    ///
    /// The exact inverse of [`Self::from_app_mode`], so a mode projected into
    /// the monitoring snapshot and parsed back out is the same value.
    pub fn as_app_mode(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

/// Whether one row accepts execution-mark mutation.
///
/// This is the *whole* mark admission rule. Execution mode, active/retry/wait
/// status, Apply iteration-limit evidence, queue intent, and parallel
/// eligibility are deliberately absent: a mark says nothing more than "consider
/// this change the next time a run command evaluates targets", and whether that
/// run may actually start is decided at final start/retry admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkAdmission {
    /// A visible non-terminal target: mark mutation is allowed.
    Allowed,
    /// Archived, merged, pushed, or rejected: the row is not a run candidate.
    TerminalTarget,
    /// The reducer recorded archive completion, so post-archive display statuses
    /// such as `resolving` or `merge wait` carry no next-run intent either.
    ArchiveComplete,
}

impl MarkAdmission {
    /// True when the row accepts mark mutation.
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Decide whether an execution-mark request may mutate this row.
///
/// The single classifier shared by TUI single-row marks, TUI bulk marks, and
/// the `/api/v2` `set_execution_mark` / `set_all_execution_marks` commands, so
/// no frontend can hold a second markability table.
///
/// `archive_complete` is *caller-supplied evidence*, not something derived here:
/// the reducer owns the archive milestone in
/// [`crate::orchestration::state::OrchestratorState::archived_changes`], and
/// orchestration must never reach into a frontend cache to read it. Operator and
/// API callers pass it from the same reducer read that produced
/// `display_status`; the TUI passes its synchronized presentation cache.
///
/// Deriving it from `display_status == "resolving"` instead would be the same
/// string-based inference this replaces: a fresh-process resolve retry is
/// `resolving` with no archive on record, and it stays markable.
pub fn classify_mark_admission(display_status: &str, archive_complete: bool) -> MarkAdmission {
    if is_final_status(display_status) {
        MarkAdmission::TerminalTarget
    } else if archive_complete {
        MarkAdmission::ArchiveComplete
    } else {
        MarkAdmission::Allowed
    }
}

/// True when the row is a visible non-terminal execution-mark target.
pub fn is_markable_status(display_status: &str, archive_complete: bool) -> bool {
    classify_mark_admission(display_status, archive_complete).is_allowed()
}

/// How an *explicit* queue command must be routed for a mode/status pair.
///
/// This is the DynamicQueue lifecycle matrix, and it is reachable only from a
/// client that intentionally invokes a queue command. Execution marks no longer
/// alias onto it: Space and bulk `x` write marks and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueIntentRoute {
    /// The mode/status pair has no runtime queue membership to mutate.
    NoQueueEffect,
    /// Dynamic queue intent (add/remove) may be mutated.
    Mutable,
    /// Reject: recovery in this mode is owned by retry commands.
    RetryRequired,
    /// Reject: the row's queue membership cannot be mutated.
    Immutable,
}

/// Decide how an explicit queue-intent request must be handled.
pub fn classify_queue_intent_route(mode: OperatorMode, display_status: &str) -> QueueIntentRoute {
    if is_final_status(display_status) {
        return QueueIntentRoute::Immutable;
    }

    match mode {
        // Error mode never mutates queue intent: `retry_change` / `retry_errors` own recovery.
        OperatorMode::Error => QueueIntentRoute::RetryRequired,
        // Select mode has no runtime queue yet.
        OperatorMode::Select => QueueIntentRoute::NoQueueEffect,
        // A pending graceful stop is a transition; queue changes wait for it.
        OperatorMode::Stopping => QueueIntentRoute::Immutable,
        OperatorMode::Stopped => {
            if matches!(display_status, "not queued" | "error")
                || MARK_ONLY_WAIT_STATUSES.contains(&display_status)
            {
                QueueIntentRoute::NoQueueEffect
            } else {
                QueueIntentRoute::Immutable
            }
        }
        OperatorMode::Running => {
            if MARK_ONLY_WAIT_STATUSES.contains(&display_status) {
                return QueueIntentRoute::NoQueueEffect;
            }
            if is_active_status(display_status) {
                // Active rows are stopped through `StopAndDequeue`.
                return QueueIntentRoute::Immutable;
            }
            match display_status {
                "not queued" | "queued" | "error" => QueueIntentRoute::Mutable,
                _ => QueueIntentRoute::Immutable,
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

    /// The action-blocked reason this observation produces; `None` when eligible.
    ///
    /// Execution marks no longer consult it — a temporarily ineligible row still
    /// accepts future run intent — but explicit queue commands and start
    /// admission do.
    pub fn queue_exclusion(self) -> Option<MarkExclusion> {
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
    /// The reducer recorded archive completion, so the row has no next run left.
    ///
    /// Distinct from [`MarkExclusion::FinalStatus`] on purpose: the row's display
    /// status is still a live post-archive one (`resolving`, `resolve pending`,
    /// `merge wait`), so "final or rejected" would not describe what an operator
    /// is looking at.
    ArchiveComplete,
}

impl MarkExclusion {
    /// Every exclusion, in the order used when grouping reasons for display.
    pub const ALL: [MarkExclusion; 8] = [
        MarkExclusion::ChangeActive,
        MarkExclusion::ParallelIneligible,
        MarkExclusion::ParallelProposalAbsent,
        MarkExclusion::ArchiveComplete,
        MarkExclusion::FinalStatus,
        MarkExclusion::RetryRequired,
        MarkExclusion::StopPending,
        MarkExclusion::StatusImmutable,
    ];

    /// Stable machine-readable token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArchiveComplete => "archive_complete",
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
            Self::ArchiveComplete => "archive complete (no next run)",
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
/// The caller supplies exactly the facts the decision needs — the reducer's
/// display status, the reducer's archive-completion evidence, and the current
/// mark. Worktree eligibility and Apply-limit evidence are deliberately *not*
/// carried here: a classifier that cannot see them cannot let them exclude a
/// non-terminal row from mark intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkTargetRow<'a> {
    /// Target change.
    pub change_id: &'a str,
    /// Reducer-derived display status.
    pub display_status: &'a str,
    /// Whether the reducer recorded archive completion for this change.
    ///
    /// Read from the same reducer snapshot as `display_status` — never inferred
    /// from the status string, and never read out of a frontend's row cache by
    /// orchestration itself.
    pub archive_complete: bool,
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

/// Classify one bulk-mark candidate; `None` means it is part of the target set.
///
/// Exactly the same rule a single-row mark request goes through, so a bulk
/// mutation and an individual command can never disagree about one row.
pub fn classify_bulk_mark_row(
    display_status: &str,
    archive_complete: bool,
) -> Option<MarkExclusion> {
    match classify_mark_admission(display_status, archive_complete) {
        MarkAdmission::Allowed => None,
        MarkAdmission::TerminalTarget => Some(MarkExclusion::FinalStatus),
        MarkAdmission::ArchiveComplete => Some(MarkExclusion::ArchiveComplete),
    }
}

/// Classify every row once and derive the single shared target mark state.
///
/// One classification pass over one coherent set of rows is what makes a bulk
/// mutation atomic in meaning: the target state cannot shift halfway through
/// because a row was re-read at a different instant.
pub fn plan_bulk_marks(rows: &[MarkTargetRow<'_>]) -> BulkMarkPlan {
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    let mut any_unmarked = false;

    for row in rows {
        match classify_bulk_mark_row(row.display_status, row.archive_complete) {
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

/// One settlement-derived queue mutation as the reducer write boundary saw it.
///
/// `skipped` is what separates "the guard refused this" from "the reducer had
/// nothing to change": a refused mutation touched nothing at all, while an
/// accepted one with `reducer_changed == false` simply found the intent already
/// where it belongs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementApplication {
    /// The per-target queue outcome, in the shape a frontend command produces.
    pub outcome: QueueOutcome,
    /// Why the application-time guard turned this into a reasoned no-op.
    pub skipped: Option<MarkSettlementExclusion>,
}

impl SettlementApplication {
    /// True when queue membership actually moved for this target.
    pub fn applied(&self) -> bool {
        self.outcome.reducer_changed || self.outcome.dynamic_queue_mutated
    }
}

/// The queue-command direction one settlement action reports as.
fn queue_mutation_for(action: MarkSettlementAction) -> QueueMutation {
    match action {
        MarkSettlementAction::Add => QueueMutation::Added,
        MarkSettlementAction::Remove => QueueMutation::Removed,
    }
}

/// Explanatory evidence fixed at a successful stop-and-dequeue settlement.
///
/// Every field is a *non-authoritative observation*. It explains what the
/// operator interrupted; it never gates a next action, never becomes durable
/// workflow state, and never causes a missing fact to be guessed. Unknown stays
/// unknown, which is why the Apply-commit fields are nullable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StopSettlement {
    /// The typed phase active immediately before dequeue was applied.
    ///
    /// `None` for an already-terminated target or one with no active phase. It
    /// is deliberately read at settlement rather than at admission: a worker can
    /// finish Apply and enter Acceptance while cancellation is in flight, and
    /// reporting the admitted-time phase would name the wrong one.
    pub cancelled_phase: ExecutionPhase,
    /// The last phase that published a typed completion fact.
    pub last_completed_phase: Option<ExecutionPhase>,
    /// Whether the final managed-worktree Apply commit was proven present;
    /// `None` when the evidence could not be read.
    pub apply_commit_present: Option<bool>,
    /// The proven Apply commit OID; `Some` only when presence is `Some(true)`.
    pub apply_commit_oid: Option<String>,
}

impl StopSettlement {
    /// The settlement of a target with nothing left running and no evidence.
    pub fn none() -> Self {
        Self {
            cancelled_phase: ExecutionPhase::None,
            last_completed_phase: None,
            apply_commit_present: None,
            apply_commit_oid: None,
        }
    }

    /// The one operator-facing sentence every frontend records for this settlement.
    ///
    /// Presentation only — a machine consumer reads the typed fields — but it
    /// must not mislead, so it names the phase that was actually cancelled,
    /// states what is known about the final Apply commit, and always denies
    /// rollback. The incident this exists for is a reader treating generic
    /// command success as proof that no Apply commit had been created.
    pub fn describe(&self, change_id: &str) -> String {
        let what = match self.cancelled_phase {
            ExecutionPhase::None => {
                format!("'{change_id}' was already terminated and was dequeued")
            }
            ExecutionPhase::Unknown => format!(
                "'{change_id}' was dequeued; the phase it was cancelled during could not be \
                 determined"
            ),
            phase => format!(
                "'{change_id}' was cancelled during {} and dequeued",
                phase.as_str()
            ),
        };
        let apply = match (self.apply_commit_present, self.apply_commit_oid.as_deref()) {
            (Some(true), Some(oid)) => {
                format!("; the final Apply commit {oid} was already created")
            }
            (Some(true), None) => "; the final Apply commit was already created".to_string(),
            _ => "; whether the final Apply commit exists could not be proven".to_string(),
        };
        format!("{what}{apply}; previously completed worktree effects were not rolled back")
    }
}

/// Why a command produced no state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoOpReason {
    /// The execution mark already had the requested value.
    MarkUnchanged,
    /// The target is archived, merged, pushed, or rejected and carries no
    /// next-run intent, so the request settles unchanged rather than failing.
    TerminalMarkTarget,
    /// The reducer recorded archive completion for the target, so its remaining
    /// post-archive work carries no next-run intent either.
    ArchiveCompleteMarkTarget,
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
        /// Explanatory evidence fixed at the settlement boundary.
        settlement: StopSettlement,
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
}

impl std::fmt::Display for OperatorCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
///
/// The store also owns the process's single mark-settlement notifier. It is the
/// one place both frontend service paths already write, so binding the stability
/// policy here is what keeps a keypress and an `/api/v2` command from arming two
/// different deadlines.
#[derive(Debug, Default)]
pub struct ExecutionMarkStore {
    marks: Mutex<HashSet<String>>,
    settlement: Arc<MarkSettlementCoordinator>,
}

impl ExecutionMarkStore {
    /// Create an empty store. A restarted process always begins here.
    pub fn new() -> Self {
        Self::default()
    }

    /// The process-local mark-settlement notifier this store owns.
    pub fn settlement(&self) -> Arc<MarkSettlementCoordinator> {
        self.settlement.clone()
    }

    /// Record `changed` in the settlement batch and arm mark settlement.
    ///
    /// Called only after an *accepted standalone operator* write actually
    /// changed the store, with exactly the targets that write flipped. A system
    /// revocation, a refused or unchanged command, and the mark writes Start
    /// admission performs deliberately do not reach here, so none of them can
    /// restart the stability deadline or create a delayed queue effect.
    ///
    /// The batch is the whole scope of the eventual reconciliation, which is why
    /// the changed targets are passed rather than the current mark set: a mark
    /// set would name every marked row, and reconciling those would move queue
    /// intent nobody touched in this batch.
    ///
    /// Returns true when a deadline is now pending.
    pub fn arm_settlement(&self, changed: Vec<String>) -> bool {
        self.settlement.clone().notify(changed)
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

    /// Replace the whole mark set.
    ///
    /// Test-only, and deliberately so. No frontend owns a mark projection it may
    /// publish back: a whole-set write derived from one frontend's cached rows
    /// both resurrects marks a concurrent event revoked and erases marks another
    /// frontend set. Production writes are target-scoped — through
    /// [`OperatorCommandService`] for operator intent, through the dispatch
    /// boundary's reconciliation for system-driven revocation.
    #[cfg(test)]
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

/// An issued cancellation whose confirmed termination has not been awaited yet.
///
/// Kept as a value so the wait happens *outside* whatever serialization gate
/// admitted the command. A never-completing waiter then blocks only its own
/// command: force stop, unrelated operator commands, event fan-out, and TUI
/// rendering all stay live.
#[derive(Clone, Debug)]
pub struct PendingTermination {
    change_id: String,
    waiter: TerminationWaiter,
    timeout: Duration,
}

impl PendingTermination {
    /// The change whose termination is pending.
    pub fn change_id(&self) -> &str {
        &self.change_id
    }

    /// Await confirmed termination within the configured bound.
    pub async fn confirm_termination(&self) -> OperatorResult<()> {
        if tokio::time::timeout(self.timeout, self.waiter.wait())
            .await
            .is_err()
        {
            return Err(OperatorCommandError::TerminationTimeout {
                change_id: self.change_id.clone(),
                waited: self.timeout,
            });
        }
        Ok(())
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
    /// Shared process-local execution facts.
    ///
    /// Read-only here, and only for explanatory settlement evidence: no
    /// admission, routing, or reducer decision consults it. `None` for a process
    /// with no observability consumer, which then reports unknown.
    execution_facts: Option<Arc<ExecutionFactsStore>>,
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
            execution_facts: None,
        }
    }

    /// Bind the shared parallel runtime store.
    pub fn with_parallel(mut self, parallel: Arc<ParallelRuntime>) -> Self {
        self.parallel = parallel;
        self
    }

    /// Bind the shared process-local execution-facts store.
    ///
    /// The same store the authoritative dispatch owner feeds, so the phase a
    /// settled command reports and the phase the execution-status resource
    /// publishes come from one observation rather than two.
    pub fn with_execution_facts(mut self, facts: Arc<ExecutionFactsStore>) -> Self {
        self.execution_facts = Some(facts);
        self
    }

    /// The bound execution-facts store, if this process has one.
    pub fn execution_facts(&self) -> Option<Arc<ExecutionFactsStore>> {
        self.execution_facts.clone()
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
    ///
    /// `mode` is retained for the queue and retry commands; execution marks are
    /// lifecycle-independent and never consult it.
    pub async fn execute(
        &self,
        _mode: OperatorMode,
        command: OperatorCommand,
    ) -> OperatorResult<OperatorOutcome> {
        match command {
            OperatorCommand::SetExecutionMark { change_id, marked } => {
                self.set_execution_mark(&change_id, marked).await
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
            OperatorCommand::SetAllExecutionMarks => self.set_all_execution_marks().await,
        }
    }

    /// Apply one already-classified target-scoped execution-mark write.
    ///
    /// A frontend that ran the shared admission rules itself — the TUI `Space`
    /// and `x` interactions, which own their own row guards and log lines — hands
    /// the *write* here rather than touching the store directly. Taking the same
    /// mutation guard event reconciliation takes is what makes an operator write
    /// and a concurrent revoking event serialize instead of interleaving, and the
    /// write is scoped to one change so a stale cached row set can never replace
    /// the store.
    ///
    /// Returns true when the stored value actually changed.
    ///
    /// An accepted write also arms mark settlement, which is what gives Space
    /// and bulk `x` the same delayed admission an `/api/v2` mark command gets
    /// without either frontend owning a timer.
    pub async fn apply_execution_mark(&self, change_id: &str, marked: bool) -> bool {
        let _mutation = self.parallel.lock_mutations().await;
        let changed = self.marks.set(change_id, marked);
        if changed {
            self.marks.arm_settlement(vec![change_id.to_string()]);
        }
        changed
    }

    /// Apply one execution-mark write that belongs to a Start request.
    ///
    /// Identical to [`Self::apply_execution_mark`] except that it never arms
    /// mark settlement. These writes are part of admission, not standalone
    /// operator intent: a Start that is later rejected must leave no delayed
    /// queue effect behind, and an accepted one has already queued its targets
    /// through run control, so a second delayed admission would be a duplicate
    /// with no request behind it.
    pub async fn apply_admission_execution_mark(&self, change_id: &str, marked: bool) -> bool {
        let _mutation = self.parallel.lock_mutations().await;
        self.marks.set(change_id, marked)
    }

    /// Apply an execution-mark request.
    ///
    /// The only thing this touches is [`ExecutionMarkStore`]. No queue, hook,
    /// cancellation, retry, resolve, scheduler, reducer, or mode effect exists
    /// on this path at all — which is what makes "unmarking cannot disturb the
    /// current run" structural rather than a rule to be re-checked.
    ///
    /// A terminal target settles as a reasoned unchanged no-op rather than a
    /// refusal: the row is simply not a run candidate any more. So does a target
    /// the reducer has already archived, whose post-archive display status is
    /// still a live one.
    pub async fn set_execution_mark(
        &self,
        change_id: &str,
        marked: bool,
    ) -> OperatorResult<OperatorOutcome> {
        let _mutation = self.parallel.lock_mutations().await;
        // One read for both facts: a status read and an archive-record read taken
        // at two instants could describe two different lifecycle states.
        let (display_status, archive_complete) = {
            let guard = self.state.read().await;
            (
                guard.display_status(change_id).to_string(),
                guard.is_archived(change_id),
            )
        };
        match classify_mark_admission(&display_status, archive_complete) {
            MarkAdmission::Allowed => {}
            MarkAdmission::TerminalTarget => {
                return Ok(OperatorOutcome::NoOp {
                    change_id: change_id.to_string(),
                    reason: NoOpReason::TerminalMarkTarget,
                })
            }
            MarkAdmission::ArchiveComplete => {
                return Ok(OperatorOutcome::NoOp {
                    change_id: change_id.to_string(),
                    reason: NoOpReason::ArchiveCompleteMarkTarget,
                })
            }
        }
        if self.marks.set(change_id, marked) {
            self.marks.arm_settlement(vec![change_id.to_string()]);
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

    /// Apply one derived execution-mark state to every eligible change.
    ///
    /// The whole operation is classified from a single read of the reducer, so
    /// the target state comes from one coherent view and every eligible row
    /// receives the identical mark. Excluded rows keep whatever intent they
    /// already had and are reported with a stable reason instead of being
    /// silently skipped.
    ///
    /// Classification and application are one mutation under the shared guard,
    /// so the row set this reads cannot move — and event reconciliation cannot
    /// run — while the plan derived from it is still being applied.
    ///
    /// Like the single-row path, this writes marks and nothing else, in every
    /// execution mode.
    pub async fn set_all_execution_marks(&self) -> OperatorResult<OperatorOutcome> {
        let _mutation = self.parallel.lock_mutations().await;

        // One read, one classification: re-reading per row could observe two
        // different instants and derive a target state from neither.
        let observed: Vec<(String, String, bool, bool)> = {
            let guard = self.state.read().await;
            guard
                .tracked_change_ids()
                .into_iter()
                .map(|change_id| {
                    let display_status = guard.display_status(&change_id).to_string();
                    // Same snapshot as the status, so an archive that completed
                    // mid-classification cannot make one row's two facts disagree.
                    let archive_complete = guard.is_archived(&change_id);
                    let marked = self.marks.is_marked(&change_id);
                    (change_id, display_status, archive_complete, marked)
                })
                .collect()
        };

        let rows: Vec<MarkTargetRow<'_>> = observed
            .iter()
            .map(
                |(change_id, display_status, archive_complete, marked)| MarkTargetRow {
                    change_id,
                    display_status,
                    archive_complete: *archive_complete,
                    marked: *marked,
                },
            )
            .collect();
        let plan = plan_bulk_marks(&rows);

        if plan.is_empty() {
            return Ok(OperatorOutcome::NoOp {
                change_id: String::new(),
                reason: NoOpReason::NoEligibleMarkTarget,
            });
        }

        let mut changed = Vec::new();
        for change_id in &plan.eligible {
            if self.marks.set(change_id, plan.target_state) {
                changed.push(change_id.clone());
            }
        }

        if changed.is_empty() {
            return Ok(OperatorOutcome::NoOp {
                change_id: String::new(),
                reason: NoOpReason::BulkMarksUnchanged,
            });
        }

        // One notification for the whole bulk mutation, not one per row: the
        // deadline describes one batch, and restarting it per row would make a
        // wide `x` take longer to settle than a narrow one. The batch carries
        // exactly the rows this bulk write flipped — never the excluded ones,
        // and never the eligible rows that already held the target state.
        self.marks.arm_settlement(changed.clone());

        Ok(OperatorOutcome::BulkMarks {
            marked: plan.target_state,
            changed,
            excluded: plan.excluded,
        })
    }

    /// Classify one settlement batch into a bidirectional plan.
    ///
    /// Read-only by construction. It derives *what* the settled batch would add
    /// and remove and nothing more, so the caller can apply the plan through the
    /// guarded queue path instead of this service growing a second one.
    ///
    /// `targets` is the batch's scope: every target whose mark an accepted
    /// operator write flipped, and nothing else. Rows outside it are never read
    /// and never planned, which is exactly what keeps an explicitly queued
    /// unmarked change — and a marked change explicitly removed from the queue —
    /// unaffected by somebody else's mark settling.
    ///
    /// The whole observation is taken under the shared mutation guard and one
    /// reducer read, which is what makes the marks, the statuses, and the
    /// worktree eligibility one coherent view rather than three instants.
    /// Deliberately re-reads the marks that exist *now*: an event that revoked a
    /// mark while the deadline was pending must land in this plan, not be
    /// overridden by the intent the batch was recorded with.
    pub async fn plan_mark_settlement(&self, targets: &[String]) -> MarkSettlementPlan {
        let observed: Vec<(String, String, bool, bool, bool)> = {
            let _mutation = self.parallel.lock_mutations().await;
            let guard = self.state.read().await;
            let tracked: HashSet<String> = guard.tracked_change_ids().into_iter().collect();
            targets
                .iter()
                .map(|change_id| {
                    let display_status = guard.display_status(change_id).to_string();
                    let tracked = tracked.contains(change_id);
                    let eligible = self.parallel.is_eligible(change_id);
                    let marked = self.marks.is_marked(change_id);
                    (change_id.clone(), display_status, tracked, eligible, marked)
                })
                .collect()
        };

        let rows: Vec<MarkSettlementRow<'_>> = observed
            .iter()
            .map(
                |(change_id, display_status, tracked, parallel_eligible, marked)| {
                    MarkSettlementRow {
                        change_id,
                        display_status,
                        tracked: *tracked,
                        parallel_eligible: *parallel_eligible,
                        marked: *marked,
                    }
                },
            )
            .collect();
        plan_mark_settlement(&rows)
    }

    /// Apply one settlement-derived queue mutation under the reducer write boundary.
    ///
    /// Classification and application are two instants, and a dispatch or a
    /// terminal transition can land between them. This is the guard that makes
    /// that race a reasoned no-op instead of a wrong mutation: the target is
    /// re-classified from the *same* write guard that applies the reducer
    /// command, so there is no window at all between deciding and mutating.
    ///
    /// It deliberately does not reuse [`Self::add_to_queue`]. That path's
    /// terminal-error branch is an explicit *retry* — it applies `RetryError`,
    /// releases the failed classification, and publishes an explicit-retry edge —
    /// and no mark ever expressed that intent. Here a terminal-error target is
    /// simply excluded, so a settled addition can never alias a retry.
    ///
    /// The scheduler is deliberately *not* notified here. One settled batch owns
    /// exactly one notification, and only the caller knows when the batch is
    /// done; see [`Self::notify_scheduler_after_settlement`].
    pub async fn apply_settlement_queue_intent(
        &self,
        change_id: &str,
        action: MarkSettlementAction,
    ) -> SettlementApplication {
        let queued = matches!(action, MarkSettlementAction::Add);
        let guard_outcome = {
            let mut guard = self.state.write().await;
            let tracked = guard.change_runtime(change_id).is_some();
            let display_status = guard.display_status(change_id);
            let row = MarkSettlementRow {
                change_id,
                display_status,
                tracked,
                // Worktree eligibility was proven when the plan was derived and
                // is not a reducer-boundary fact. What this guard re-reads is
                // the lifecycle, which is the only thing the reducer can have
                // moved since.
                parallel_eligible: true,
                marked: queued,
            };
            match classify_mark_settlement_row(&row) {
                Ok(current) if current == action => {
                    let command = if queued {
                        ReducerCommand::AddToQueue(change_id.to_string())
                    } else {
                        ReducerCommand::RemoveFromQueue(change_id.to_string())
                    };
                    Ok(matches!(
                        guard.apply_command(command),
                        ReduceOutcome::Changed(_)
                    ))
                }
                // A row whose reconciliation flipped direction under the guard
                // is already where the operator's current mark wants it.
                Ok(_) => Err(if queued {
                    MarkSettlementExclusion::AlreadyQueued
                } else {
                    MarkSettlementExclusion::AlreadyNotQueued
                }),
                Err(reason) => Err(reason),
            }
        };

        let reducer_changed = match guard_outcome {
            Ok(changed) => changed,
            Err(reason) => {
                // Nothing was touched: no reducer command, no dynamic queue
                // mutation, no hook, no explicit-retry edge, and no active
                // lifecycle evidence cleared.
                return SettlementApplication {
                    outcome: QueueOutcome {
                        change_id: change_id.to_string(),
                        mutation: queue_mutation_for(action),
                        reducer_changed: false,
                        dynamic_queue_mutated: false,
                        display_status: self.display_status(change_id).await,
                    },
                    skipped: Some(reason),
                };
            }
        };

        // Effect before commit, and only for a mutation the reducer accepted:
        // hooks describe real runtime mutations only, exactly once each.
        let dynamic_queue_mutated = if !reducer_changed {
            false
        } else if queued {
            let added = self.queue.add(change_id).await;
            if added {
                self.hooks.on_queue_add(change_id).await;
            }
            added
        } else {
            let removed = self.queue.remove(change_id).await;
            self.hooks.on_queue_remove(change_id).await;
            removed
        };

        SettlementApplication {
            outcome: QueueOutcome {
                change_id: change_id.to_string(),
                mutation: queue_mutation_for(action),
                reducer_changed,
                dynamic_queue_mutated,
                display_status: self.display_status(change_id).await,
            },
            skipped: None,
        }
    }

    /// Wake the scheduler once for a settled batch that changed queue membership.
    ///
    /// Coalesced on purpose. One settled batch is one analysis input change, so
    /// notifying per target would produce N duplicate analysis attempts for a
    /// single operator action — and a removal-only batch still changes what the
    /// scheduler should consider, so it notifies too.
    pub async fn notify_scheduler_after_settlement(&self) {
        self.queue.notify_scheduler().await;
    }

    /// Add a change to the dynamic queue.
    ///
    /// A dependency-ineligible change keeps its queue intent: dependency
    /// blocking is reported later as `blocked` display status, never by
    /// rejecting the operator's request.
    pub async fn add_to_queue(&self, change_id: &str) -> OperatorResult<QueueOutcome> {
        let (reduce_outcome, was_error_retry) = {
            let mut guard = self.state.write().await;
            // A terminal-error addition *is* a retry: it applies `RetryError`,
            // releases the failed classification, and publishes an explicit-retry
            // edge. It is therefore classified exactly like `retry_change` — the
            // alias is explicit retry intent, which is the one thing allowed to
            // consume a terminal error, retained Apply-limit diagnostic or not.
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
        let pending = self.begin_stop_and_dequeue(change_id).await?;
        pending.confirm_termination().await?;
        // No evidence port on this convenience path: it is the single-caller
        // shape used where nothing consumes explanatory Git evidence, and an
        // unproven commit is reported as unknown rather than guessed.
        self.commit_stop_and_dequeue(change_id, ApplyCommitEvidence::unknown())
            .await
    }

    /// The bound this service waits for confirmed task termination within.
    pub fn cancellation_timeout(&self) -> Duration {
        self.cancellation_timeout
    }

    /// Phase one of a stop-and-dequeue: validate and issue cancellation.
    ///
    /// Cancellation is an intentional runtime request, not a rollbackable
    /// decision-state mutation: once it has been issued, a later timeout or
    /// refusal must commit no dequeue state rather than pretend the request
    /// never happened.
    ///
    /// The returned waiter is deliberately *not* awaited here. A caller holding a
    /// serialization gate must release it before confirmation, or one slow
    /// termination would monopolize operator admission.
    pub async fn begin_stop_and_dequeue(
        &self,
        change_id: &str,
    ) -> OperatorResult<PendingTermination> {
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

        Ok(PendingTermination {
            change_id: change_id.to_string(),
            waiter,
            timeout: self.cancellation_timeout,
        })
    }

    /// Phase two of a stop-and-dequeue: commit after confirmed termination.
    ///
    /// Revalidates through the reducer rather than through the revision the
    /// command was admitted at: unrelated commands may legitimately advance the
    /// projection while termination is pending, but the target's own runtime
    /// state is what decides whether a dequeue is still correct.
    pub async fn commit_stop_and_dequeue(
        &self,
        change_id: &str,
        apply_commit: ApplyCommitEvidence,
    ) -> OperatorResult<OperatorOutcome> {
        // Commit the dequeue and its mark revocation as one indivisible mutation:
        // the same guard event reconciliation takes, so the `ChangeDequeued` edge
        // this produces cannot land between the two halves.
        let _mutation = self.parallel.lock_mutations().await;
        // The phase is read from the *same* write guard that applies the
        // dequeue, immediately before it. `DequeueChange` clears the activity it
        // describes, so a read taken after the commit — or under a second lock
        // acquisition — could only ever report `none`.
        let (cancelled_phase, reduce_outcome) = {
            let mut guard = self.state.write().await;
            let phase = guard
                .change_runtime(change_id)
                .map(|runtime| project_phase(runtime, self.push_open(change_id)))
                // The reducer does not track this change at all, so there is no
                // typed evidence to classify. That is unknown, not "nothing was
                // running".
                .unwrap_or(ExecutionPhase::Unknown);
            let outcome = guard.apply_command(ReducerCommand::DequeueChange(change_id.to_string()));
            (phase, outcome)
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
            settlement: StopSettlement {
                cancelled_phase,
                last_completed_phase: self
                    .execution_facts
                    .as_ref()
                    .and_then(|facts| facts.change(change_id).last_completed_phase),
                apply_commit_present: apply_commit.present,
                apply_commit_oid: apply_commit.oid,
            },
        })
    }

    /// Whether a typed push episode is open for a change.
    ///
    /// Publication reuses the reducer's `Resolving` activity, so without this
    /// the settlement would report a cancelled resolve where a cancelled push
    /// actually happened.
    fn push_open(&self, change_id: &str) -> bool {
        self.execution_facts
            .as_ref()
            .is_some_and(|facts| facts.change(change_id).current_phase == ExecutionPhase::Push)
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
        let routes = match self.plan_retry_change(change_id).await? {
            Some(route) => vec![(change_id.to_string(), route)],
            None => Vec::new(),
        };
        Ok(self.commit_retry_routes(&routes).await)
    }

    /// Classify one change's retry route without mutating anything.
    ///
    /// Read-only by construction, which is what lets a caller reserve every
    /// fallible runtime capability *before* any retry effect exists: a
    /// preparation failure then has nothing to roll back.
    ///
    /// `Ok(None)` is an accepted-but-empty classification — a hold whose blocker
    /// evidence must survive rather than be consumed — and is distinct from the
    /// typed refusal an unsupported status produces.
    pub async fn plan_retry_change(&self, change_id: &str) -> OperatorResult<Option<RetryRoute>> {
        // Classification reads the target's own evidence and nothing about the
        // invocation that failed it: a retained Apply-limit diagnostic explains
        // why one invocation stopped, never whether a later explicit command may
        // open a new one.
        let display_status = self.display_status(change_id).await;
        let blocker_kind = self.blocker_kind(change_id).await;
        let Some(route) = classify_retry_route(&display_status, blocker_kind) else {
            return Err(OperatorCommandError::RetryUnsupported {
                change_id: change_id.to_string(),
                display_status,
            });
        };
        Ok(self
            .route_is_committable(change_id, route)
            .await
            .then_some(route))
    }

    /// Classify every retryable change in `change_ids`, skipping the rest.
    ///
    /// The bulk counterpart of [`Self::plan_retry_change`], and read-only for the
    /// same reason.
    pub async fn plan_retry_errors(&self, change_ids: &[String]) -> Vec<(String, RetryRoute)> {
        let mut routes = Vec::new();
        for change_id in change_ids {
            let display_status = self.display_status(change_id).await;
            let blocker_kind = self.blocker_kind(change_id).await;
            let Some(route) = classify_retry_route(&display_status, blocker_kind) else {
                continue;
            };
            if self.route_is_committable(change_id, route).await {
                routes.push((change_id.clone(), route));
            }
        }
        routes
    }

    /// Whether an already-classified route may actually be consumed.
    ///
    /// A non-resumable acceptance hold is refused so its blocker evidence
    /// survives and no ambiguous work is dispatched.
    async fn route_is_committable(&self, change_id: &str, route: RetryRoute) -> bool {
        if !matches!(route, RetryRoute::AcceptanceStall) {
            return true;
        }
        let refuse = {
            let guard = self.state.read().await;
            guard
                .change_runtime(change_id)
                .is_some_and(|rt| rt.is_acceptance_stalled() && !rt.is_resumable_acceptance_stall())
        };
        if refuse {
            tracing::warn!(
                change_id = %change_id,
                "Explicit retry refused: the acceptance stall is not resumable, so its \
                 blocker evidence is retained"
            );
        }
        !refuse
    }

    /// Commit already-classified retry routes.
    ///
    /// The mutating half of the split: every guard the routes had to pass has
    /// already passed, so this only applies reducer intent, publishes the
    /// explicit-retry edges the accepted routes imply, and restores marks.
    pub async fn commit_retry_routes(&self, routes: &[(String, RetryRoute)]) -> RetryPlan {
        let mut plan = RetryPlan {
            change_ids: Vec::new(),
            routes: Vec::new(),
            explicit_retry: false,
        };
        for (change_id, route) in routes {
            let accepted = self.apply_retry_route(change_id, *route).await;
            plan.change_ids.extend(accepted.change_ids);
            plan.routes.extend(accepted.routes);
            plan.explicit_retry |= accepted.explicit_retry;
        }
        plan
    }

    /// Route a bulk retry request.
    ///
    /// Changes without retryable evidence are skipped rather than rejected, so a
    /// bulk retry never consumes an unsupported or identity-mismatched hold. An
    /// active-run-limited target is skipped for the same reason: one exhausted
    /// per-change ceiling is not a reason to refuse every unrelated candidate,
    /// and a skipped target is never reported as accepted.
    pub async fn retry_errors(&self, change_ids: &[String]) -> RetryPlan {
        let routes = self.plan_retry_errors(change_ids).await;
        self.commit_retry_routes(&routes).await
    }

    async fn apply_retry_route(&self, change_id: &str, route: RetryRoute) -> RetryPlan {
        let command = match route {
            RetryRoute::TerminalError => ReducerCommand::RetryError(change_id.to_string()),
            // An in-memory acceptance hold resumes through the explicit-retry run
            // path; the reducer only has to restore ordinary queue intent.
            // Whether the hold may be consumed at all was already decided by
            // [`Self::route_is_committable`] during classification.
            RetryRoute::AcceptanceStall => ReducerCommand::AddToQueue(change_id.to_string()),
        };
        let is_error_retry = matches!(command, ReducerCommand::RetryError(_));
        // Retry restores fresh execution intent, so it is a mark mutation and
        // serializes with event reconciliation like every other one.
        let _mutation = self.parallel.lock_mutations().await;
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
