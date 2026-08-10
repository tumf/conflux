//! Event-driven reconciliation of the shared execution-mark set.
//!
//! [`ExecutionMarkStore`] is the process-local authority for "which changes an
//! operator asked to run". TUI rows and `/api/v2` snapshots are projections of
//! it. Before this module existed, several TUI event handlers cleared only their
//! own row cache when a change failed, was rejected, or was dequeued, so the
//! screen could show `[ ]` while `/api/v2` still reported
//! `execution_marked: true` and a later Start read the stale target.
//!
//! The fix is one reconciliation step at the authoritative dispatch boundary,
//! after the reducer applies the event and before any frontend sink runs. It is
//! deliberately **edge-based, not status-based**: a steady `error` row may carry
//! a *fresh* explicit re-mark, and clearing every marked error row on every
//! event would destroy exactly the intent an operator just expressed. A mark is
//! revoked only when applying this event is what moved the target into the
//! revoking state.
//!
//! Nothing here is durable. Marks live for one process lifetime, are never
//! written outside the workspace, and never become workflow-control evidence.

use std::sync::Arc;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::{
    parallel_cleanup_targets, ExecutionMarkStore, ParallelCleanupRow, ParallelEligibility,
    ParallelRuntime,
};
use crate::orchestration::state::{OrchestratorState, TerminalState, WaitState};

/// Reducer evidence for one change, captured immediately before and after an
/// event is applied.
///
/// Only the facts an edge is classified from: comparing whole runtime states
/// would make unrelated reducer bookkeeping look like a mark decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MarkEvidence {
    /// The change is in a recoverable change-level terminal error.
    pub error: bool,
    /// The change reached terminal `Rejected`.
    pub rejected: bool,
    /// The change carries the process-local dequeue guard.
    pub dequeued: bool,
    /// The change is waiting for a merge that needs operator recovery.
    pub merge_wait: bool,
    /// The change completed archive and left the pending set.
    pub archived: bool,
}

impl MarkEvidence {
    /// Observe the evidence for `change_id` in `state`.
    ///
    /// An untracked change reports the default (nothing observed), which is what
    /// makes a first-sighting event unable to look like a transition.
    pub fn observe(state: &OrchestratorState, change_id: &str) -> Self {
        // Archive membership is a run-level aggregate rather than per-change
        // runtime, so it is read even for a change with no runtime entry.
        let archived = state.is_archived(change_id);
        let Some(runtime) = state.change_runtime(change_id) else {
            return Self {
                archived,
                ..Self::default()
            };
        };
        Self {
            error: matches!(runtime.terminal, TerminalState::Error(_)),
            rejected: matches!(runtime.terminal, TerminalState::Rejected(_)),
            dequeued: runtime.dequeued,
            merge_wait: matches!(runtime.wait_state, WaitState::MergeWait),
            archived,
        }
    }
}

/// Which reducer edge one typed event is allowed to revoke a mark on.
///
/// The kind is decided from the event variant, and the *decision* is decided
/// from pre/post evidence. Keeping the two apart is what makes a late
/// `…Failed` variant that cannot supersede a final outcome a no-op instead of a
/// revocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokingEdge {
    /// Processing/apply/acceptance/archive/push/rejection-review failure.
    ChangeLevelFailure,
    /// Terminal rejection.
    Rejection,
    /// Successful per-change dequeue, or the legacy target-scoped stop.
    Dequeue,
    /// The first `on_merged` hook failure that enters merge-wait recovery.
    MergedHookRecovery,
    /// The first effective archive completion.
    ///
    /// This is the terminal edge where next-run intent stops having meaning: an
    /// archived row is no longer a run candidate, and the later `merged` and
    /// `pushed` transitions must not be able to bring the mark back.
    Archive,
}

impl RevokingEdge {
    /// Whether this edge was actually created by applying the event.
    ///
    /// Each edge tests exactly one property, so an event whose reducer effect
    /// was refused — a duplicate delivery, a late failure behind a final
    /// outcome — reports `false` and leaves a fresh re-mark alone.
    pub fn revokes(self, pre: MarkEvidence, post: MarkEvidence) -> bool {
        match self {
            Self::ChangeLevelFailure => post.error && !pre.error,
            Self::Rejection => post.rejected && !pre.rejected,
            Self::Dequeue => post.dequeued && !pre.dequeued,
            Self::MergedHookRecovery => post.merge_wait && !pre.merge_wait,
            Self::Archive => post.archived && !pre.archived,
        }
    }
}

/// The change an event can revoke, and the edge it is allowed to revoke on.
///
/// `None` for every event that carries no revoking edge, including process-level
/// `Stopped`, global fatal `Error`, `ChangeSkipped`, dependency and stalled
/// holds, ordinary merge/resolve waits, and every non-archive success
/// transition — `MergeCompleted` and a push completion included, because the
/// archive edge already cleared the mark and a second clear would only give the
/// same fact two authorities.
pub fn revoking_edge_target(event: &ExecutionEvent) -> Option<(&str, RevokingEdge)> {
    use ExecutionEvent as E;
    match event {
        E::ProcessingError { id, .. } => Some((id, RevokingEdge::ChangeLevelFailure)),
        E::ApplyFailed { change_id, .. }
        | E::AcceptanceFailed { change_id, .. }
        | E::ArchiveFailed { change_id, .. }
        | E::PushFailed { change_id, .. }
        | E::RejectionReviewFailed { change_id, .. } => {
            Some((change_id, RevokingEdge::ChangeLevelFailure))
        }
        E::ChangeRejected { change_id, .. } => Some((change_id, RevokingEdge::Rejection)),
        E::ChangeArchived(change_id) => Some((change_id, RevokingEdge::Archive)),
        E::ChangeDequeued { change_id } | E::ChangeStopped { change_id } => {
            Some((change_id, RevokingEdge::Dequeue))
        }
        E::HookFailed {
            change_id,
            hook_type,
            ..
        } if hook_type == crate::hooks::HookType::OnMerged.config_key() => {
            Some((change_id, RevokingEdge::MergedHookRecovery))
        }
        _ => None,
    }
}

/// Evidence captured before an event is applied, kept until reconciliation.
#[derive(Debug, Clone)]
pub enum MarkPreState {
    /// A single target and the edge its event may revoke.
    Target {
        /// Target change.
        change_id: String,
        /// Edge the event is allowed to revoke on.
        edge: RevokingEdge,
        /// Reducer evidence observed before the event was applied.
        evidence: MarkEvidence,
    },
    /// A catalog refresh, classified from its own payload plus post-state.
    Refresh,
}

/// Capture the pre-application evidence one event needs, if any.
pub fn capture_pre_state(
    event: &ExecutionEvent,
    state: &OrchestratorState,
) -> Option<MarkPreState> {
    if matches!(event, ExecutionEvent::ChangesRefreshed { .. }) {
        return Some(MarkPreState::Refresh);
    }
    let (change_id, edge) = revoking_edge_target(event)?;
    Some(MarkPreState::Target {
        change_id: change_id.to_string(),
        edge,
        evidence: MarkEvidence::observe(state, change_id),
    })
}

/// Targets one `ChangesRefreshed` revokes, in deterministic order.
///
/// Two independent observations, both target-scoped:
///
/// * a change that appears as a rejected marker row, and
/// * a marked change the same refresh classifies parallel-ineligible, decided by
///   the canonical [`parallel_cleanup_targets`] rule rather than by a
///   frontend-local copy of it.
///
/// Only currently marked rows are returned, which makes repeated refresh
/// cleanup idempotent and leaves every unrelated remote mark untouched.
pub fn refresh_revocation_targets(
    event: &ExecutionEvent,
    marks: &ExecutionMarkStore,
    post: &OrchestratorState,
) -> Vec<String> {
    let ExecutionEvent::ChangesRefreshed {
        changes,
        rejected_changes,
        committed_change_ids,
        uncommitted_file_change_ids,
        ..
    } = event
    else {
        return Vec::new();
    };

    let mut targets: Vec<String> = Vec::new();
    for change in rejected_changes {
        if marks.is_marked(&change.id) && !targets.contains(&change.id) {
            targets.push(change.id.clone());
        }
    }

    let rows: Vec<ParallelCleanupRow<'_>> = changes
        .iter()
        .map(|change| ParallelCleanupRow {
            change_id: &change.id,
            parallel_eligible: ParallelEligibility::observe(
                &change.id,
                committed_change_ids,
                uncommitted_file_change_ids,
            )
            .is_eligible(),
            marked: marks.is_marked(&change.id),
            queued: post.display_status(&change.id) == "queued",
        })
        .collect();

    for change_id in parallel_cleanup_targets(&rows) {
        if marks.is_marked(&change_id) && !targets.contains(&change_id) {
            targets.push(change_id);
        }
    }

    targets
}

/// The shared execution-mark store bound to one authoritative dispatch path,
/// together with the operator mutation guard it serializes against.
///
/// A process has exactly one of these. A dispatcher without operator marks — a
/// CLI run, a unit test — binds `None` instead of constructing a second store,
/// because a second store would be a second answer to "which targets are
/// marked".
#[derive(Clone)]
pub struct ExecutionMarkReconciler {
    marks: Arc<ExecutionMarkStore>,
    guard: Arc<ParallelRuntime>,
}

impl std::fmt::Debug for ExecutionMarkReconciler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionMarkReconciler")
            .field("marked", &self.marks.marked_ids())
            .finish()
    }
}

impl ExecutionMarkReconciler {
    /// Bind the shared mark store and the shared operator mutation guard.
    pub fn new(marks: Arc<ExecutionMarkStore>, guard: Arc<ParallelRuntime>) -> Self {
        Self { marks, guard }
    }

    /// Take the shared operator mutation guard for one ordered dispatch.
    ///
    /// The same guard operator mark actions take, so a mark write and an event
    /// revocation can never interleave inside one another.
    pub async fn lock_mutations(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.guard.lock_mutations().await
    }

    /// Capture the pre-application evidence for `event`.
    pub fn capture(
        &self,
        event: &ExecutionEvent,
        state: &OrchestratorState,
    ) -> Option<MarkPreState> {
        capture_pre_state(event, state)
    }

    /// Apply the target-scoped revocation this event's edge implies, if any.
    ///
    /// Returns the changes whose mark actually moved, so a caller can report a
    /// revocation instead of leaving an operator to discover it.
    pub fn reconcile(
        &self,
        event: &ExecutionEvent,
        pre: &MarkPreState,
        post: &OrchestratorState,
    ) -> Vec<String> {
        match pre {
            MarkPreState::Target {
                change_id,
                edge,
                evidence,
            } => {
                let after = MarkEvidence::observe(post, change_id);
                if edge.revokes(*evidence, after) && self.marks.set(change_id, false) {
                    vec![change_id.clone()]
                } else {
                    Vec::new()
                }
            }
            MarkPreState::Refresh => {
                let targets = refresh_revocation_targets(event, &self.marks, post);
                targets
                    .into_iter()
                    .filter(|change_id| self.marks.set(change_id, false))
                    .collect()
            }
        }
    }
}

#[cfg(test)]
mod tests;
