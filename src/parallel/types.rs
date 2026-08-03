//! Common types for parallel execution.

use std::collections::{HashMap, HashSet};

/// Result of a workspace execution (VCS-agnostic)
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// OpenSpec change ID
    pub change_id: String,
    /// Workspace name
    pub workspace_name: String,
    /// Final revision if successful
    pub final_revision: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Rejection reason when acceptance was blocked and rejection flow completed
    pub rejected: Option<String>,
}

/// Bounded, machine-readable classification of a terminal resolve failure.
///
/// The token set is closed on purpose: it is copied into lifecycle diagnostics,
/// so it must stay comparable across runs. Unbounded agent output never becomes
/// a classification — it stays in observability events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveFailureClassification {
    /// Conflicts were still present after the last bounded attempt.
    UnresolvedConflict,
    /// The resolve agent itself failed (non-zero exit or protocol failure) and
    /// left conflicts behind.
    ResolveAgentFailed,
    /// Repository evidence withheld further agent action because the batch
    /// identity was unproven.
    EvidenceWithheld,
    /// The bounded retry budget ran out without a more specific final class.
    RetriesExhausted,
}

impl ResolveFailureClassification {
    /// Stable operator/machine-facing token for this classification.
    pub fn token(self) -> &'static str {
        match self {
            Self::UnresolvedConflict => "unresolved_conflict",
            Self::ResolveAgentFailed => "resolve_agent_failed",
            Self::EvidenceWithheld => "evidence_withheld",
            Self::RetriesExhausted => "retries_exhausted",
        }
    }
}

/// Which typed change-scoped owner already reported a background failure.
///
/// A failure that crosses the shared base-lane boundary carrying one of these
/// kinds has already produced its own lifecycle event, so promoting it again at
/// the queue boundary would duplicate an owned transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlreadyReportedFailureKind {
    /// `PushFailed` was emitted by the publication owner.
    Push,
    /// `HookFailed` was emitted by the change hook owner.
    Hook,
    /// `RejectionReviewFailed` was emitted by the rejection-review owner.
    RejectionReview,
}

impl AlreadyReportedFailureKind {
    /// Stable operator/machine-facing token for this kind.
    pub fn token(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Hook => "hook",
            Self::RejectionReview => "rejection_review",
        }
    }
}

/// Exhaustive outcome of a background base-lane operation.
///
/// This type is the single classification boundary between the merge/resolve
/// producers and the scheduler. It deliberately replaces a bare
/// `Result<_, String>`: a string erases whether a change-scoped lifecycle event
/// was already emitted, whether manual retry evidence survives, and whether
/// repository truth is still safe — which is exactly what decides between a
/// change-local failure and a run-fatal one.
///
/// Anything that cannot be proven change-local fails closed as
/// [`MergeTaskOutcome::RunFatal`]; scope is never inferred from diagnostic text
/// or from [`MergeResultOrigin`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeTaskOutcome {
    /// The archived change was merged into the base branch.
    Merged,
    /// The merge did not complete and must remain pending for retry or operator action.
    Deferred {
        /// Human-readable reason for defer.
        reason: String,
        /// Whether the deferral can be retried automatically when the base-mutating lane frees.
        auto_resumable: bool,
    },
    /// Bounded conflict resolution exhausted its attempts while repository and
    /// worktree evidence remained available for explicit retry.
    ///
    /// The conflict layer has already emitted the one authoritative
    /// `ResolveFailed` for this change.
    ResolveExhausted {
        /// Change whose bounded resolve was exhausted.
        change_id: String,
        /// Number of attempts that were exhausted.
        attempts: u32,
        /// Bounded final failure classification.
        classification: ResolveFailureClassification,
        /// Sanitized bounded operator detail.
        detail: String,
    },
    /// A failure whose typed change-scoped owner already emitted its lifecycle event.
    RecoverableAlreadyReported {
        /// Change the reported failure belongs to.
        change_id: String,
        /// Which typed owner already reported it.
        kind: AlreadyReportedFailureKind,
        /// Sanitized bounded operator detail.
        detail: String,
    },
    /// Base or repository truth is unsafe or unknown and no safe continuation exists.
    RunFatal {
        /// Sanitized bounded operator detail.
        detail: String,
    },
}

impl MergeTaskOutcome {
    pub fn deferred(reason: impl Into<String>, auto_resumable: bool) -> Self {
        Self::Deferred {
            reason: reason.into(),
            auto_resumable,
        }
    }

    pub fn resolve_exhausted(
        change_id: impl Into<String>,
        attempts: u32,
        classification: ResolveFailureClassification,
        detail: impl Into<String>,
    ) -> Self {
        Self::ResolveExhausted {
            change_id: change_id.into(),
            attempts,
            classification,
            detail: crate::events::sanitize_detail(&detail.into()),
        }
    }

    pub fn already_reported(
        change_id: impl Into<String>,
        kind: AlreadyReportedFailureKind,
        detail: impl Into<String>,
    ) -> Self {
        Self::RecoverableAlreadyReported {
            change_id: change_id.into(),
            kind,
            detail: crate::events::sanitize_detail(&detail.into()),
        }
    }

    pub fn run_fatal(detail: impl Into<String>) -> Self {
        Self::RunFatal {
            detail: crate::events::sanitize_detail(&detail.into()),
        }
    }

    /// Scheduler disposition owed by this outcome.
    ///
    /// The match is exhaustive with no `_` arm: a new outcome variant cannot be
    /// added without deciding, at compile time, whether it continues the run or
    /// aborts it.
    pub fn disposition(&self) -> MergeResultDisposition {
        match self {
            Self::Merged => MergeResultDisposition::Merged,
            Self::Deferred { .. } => MergeResultDisposition::Deferred,
            Self::ResolveExhausted { .. } | Self::RecoverableAlreadyReported { .. } => {
                MergeResultDisposition::ContinueWithErrors
            }
            Self::RunFatal { .. } => MergeResultDisposition::AbortRun,
        }
    }

    /// Change this outcome is scoped to, when it carries change identity.
    pub fn scoped_change_id(&self) -> Option<&str> {
        match self {
            Self::ResolveExhausted { change_id, .. }
            | Self::RecoverableAlreadyReported { change_id, .. } => Some(change_id.as_str()),
            Self::Merged | Self::Deferred { .. } | Self::RunFatal { .. } => None,
        }
    }
}

/// Scheduler disposition owed by one handled base-lane result.
///
/// Lane and counter release happens before the disposition is applied, so
/// severity never decides whether ownership is returned to the scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeResultDisposition {
    /// The base lane produced a completed merge; success-only follow-up may run.
    Merged,
    /// Non-terminal continuation with no failure recorded.
    Deferred,
    /// A change-local failure was recorded; the run continues.
    ContinueWithErrors,
    /// The run is invalidated; new dispatch stops and the scheduler fails.
    AbortRun,
}

impl MergeResultDisposition {
    /// Whether this disposition represents an actual completed merge.
    pub fn is_merged(self) -> bool {
        matches!(self, Self::Merged)
    }
}

/// Render bounded operator detail for a terminal resolve failure.
///
/// The lifecycle payload carries attempts, the stable classification token, and
/// a sanitized bounded summary. Raw agent output stays in observability events.
pub fn resolve_failure_detail(
    attempts: u32,
    classification: ResolveFailureClassification,
    summary: &str,
) -> String {
    crate::events::sanitize_detail(&format!(
        "resolve exhausted after {} attempt(s) [{}]: {}",
        attempts,
        classification.token(),
        summary
    ))
}

/// Scheduler-visible origin of a background base-lane operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeResultOrigin {
    /// Normal post-archive merge task spawned after a workspace completes.
    PostArchiveMerge,
    /// Retry of a change that was waiting in ResolveWait.
    ResolveWaitRetry,
    /// Retry of a change that was waiting in RejectWait.
    RejectWaitRetry,
}

/// Result of a background merge or base-lane retry operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Change associated with this base-lane attempt.
    pub change_id: String,
    /// Workspace name that produced this attempt.
    pub workspace_name: String,
    /// Scheduler-visible origin of this background base-lane operation.
    pub origin: MergeResultOrigin,
    /// Exhaustively typed outcome of the merge attempt.
    pub outcome: MergeTaskOutcome,
}

/// Tracks failed changes and their dependencies to enable automatic skipping.
///
/// When a change fails, any changes that depend on it should be skipped
/// since they are unlikely to succeed without the dependency.
#[derive(Debug, Default)]
pub struct FailedChangeTracker {
    /// Set of failed change IDs
    failed_changes: HashSet<String>,
    /// Dependencies between changes (change_id -> list of dependencies)
    dependencies: HashMap<String, Vec<String>>,
}

impl FailedChangeTracker {
    /// Create a new empty tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the dependencies for all changes.
    ///
    /// The dependencies map should contain change_id -> [dependency_ids].
    pub fn set_dependencies(&mut self, dependencies: HashMap<String, Vec<String>>) {
        self.dependencies = dependencies;
    }

    /// Mark a change as failed
    pub fn mark_failed(&mut self, change_id: &str) {
        self.failed_changes.insert(change_id.to_string());
    }

    /// Check if a change should be skipped due to a failed dependency.
    ///
    /// Returns `Some(failed_dep_id)` if the change depends on a failed change,
    /// otherwise returns `None`.
    pub fn should_skip(&self, change_id: &str) -> Option<String> {
        if let Some(deps) = self.dependencies.get(change_id) {
            for dep in deps {
                if self.failed_changes.contains(dep) {
                    return Some(dep.clone());
                }
            }
        }
        None
    }

    /// Get all failed changes
    #[allow(dead_code)] // Public API for external callers
    pub fn failed_changes(&self) -> &HashSet<String> {
        &self.failed_changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_outcome_disposes_as_merged() {
        assert_eq!(
            MergeTaskOutcome::Merged.disposition(),
            MergeResultDisposition::Merged
        );
    }

    #[test]
    fn deferred_outcome_is_continuation_without_recorded_failure() {
        for auto_resumable in [true, false] {
            assert_eq!(
                MergeTaskOutcome::deferred("base lane busy", auto_resumable).disposition(),
                MergeResultDisposition::Deferred,
                "a deferral is pending work, not a change failure"
            );
        }
    }

    #[test]
    fn resolve_exhausted_outcome_continues_with_errors_and_keeps_change_scope() {
        let outcome = MergeTaskOutcome::resolve_exhausted(
            "alpha",
            3,
            ResolveFailureClassification::UnresolvedConflict,
            "conflicts remain in a.rs",
        );

        assert_eq!(
            outcome.disposition(),
            MergeResultDisposition::ContinueWithErrors
        );
        assert_eq!(outcome.scoped_change_id(), Some("alpha"));
    }

    #[test]
    fn already_reported_outcomes_continue_with_errors_for_every_kind() {
        for kind in [
            AlreadyReportedFailureKind::Push,
            AlreadyReportedFailureKind::Hook,
            AlreadyReportedFailureKind::RejectionReview,
        ] {
            let outcome = MergeTaskOutcome::already_reported("alpha", kind, "already reported");
            assert_eq!(
                outcome.disposition(),
                MergeResultDisposition::ContinueWithErrors,
                "{:?} already has a typed owner and must never abort the run",
                kind
            );
            assert_eq!(outcome.scoped_change_id(), Some("alpha"));
        }
    }

    #[test]
    fn run_fatal_outcome_aborts_the_run_and_carries_no_change_scope() {
        let outcome = MergeTaskOutcome::run_fatal("base branch could not be identified");

        assert_eq!(outcome.disposition(), MergeResultDisposition::AbortRun);
        assert_eq!(outcome.scoped_change_id(), None);
    }

    #[test]
    fn outcome_details_are_sanitized_and_bounded() {
        let outcome = MergeTaskOutcome::run_fatal("line one\nline two\u{7}");
        let MergeTaskOutcome::RunFatal { detail } = outcome else {
            panic!("expected run-fatal outcome");
        };

        assert!(!detail.contains('\n'), "raw newlines must be escaped");
        assert!(
            !detail.contains('\u{7}'),
            "control characters must be dropped"
        );
    }

    #[test]
    fn resolve_failure_detail_carries_attempts_classification_and_summary() {
        let detail = resolve_failure_detail(
            3,
            ResolveFailureClassification::ResolveAgentFailed,
            "resolve command exited 1",
        );

        assert!(
            detail.contains("3 attempt(s)"),
            "missing attempts: {detail}"
        );
        assert!(
            detail.contains("resolve_agent_failed"),
            "missing classification token: {detail}"
        );
        assert!(
            detail.contains("resolve command exited 1"),
            "missing summary: {detail}"
        );
    }

    #[test]
    fn classification_tokens_are_distinct_and_stable() {
        let tokens = [
            ResolveFailureClassification::UnresolvedConflict.token(),
            ResolveFailureClassification::ResolveAgentFailed.token(),
            ResolveFailureClassification::EvidenceWithheld.token(),
            ResolveFailureClassification::RetriesExhausted.token(),
        ];
        let unique: HashSet<&str> = tokens.iter().copied().collect();

        assert_eq!(unique.len(), tokens.len(), "tokens must be distinguishable");
    }

    #[test]
    fn test_failed_tracker_new() {
        let tracker = FailedChangeTracker::new();
        assert!(tracker.failed_changes.is_empty());
        assert!(tracker.dependencies.is_empty());
    }

    #[test]
    fn test_mark_failed() {
        let mut tracker = FailedChangeTracker::new();
        tracker.mark_failed("change-a");
        assert!(tracker.failed_changes.contains("change-a"));
    }

    #[test]
    fn test_should_skip_no_dependencies() {
        let tracker = FailedChangeTracker::new();
        assert!(tracker.should_skip("change-a").is_none());
    }

    #[test]
    fn test_should_skip_with_failed_dependency() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-b depends on change-a
        let mut deps = HashMap::new();
        deps.insert("change-b".to_string(), vec!["change-a".to_string()]);
        tracker.set_dependencies(deps);

        // change-a fails
        tracker.mark_failed("change-a");

        // change-b should be skipped
        let result = tracker.should_skip("change-b");
        assert_eq!(result, Some("change-a".to_string()));
    }

    #[test]
    fn test_should_skip_no_failed_dependency() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-b depends on change-a
        let mut deps = HashMap::new();
        deps.insert("change-b".to_string(), vec!["change-a".to_string()]);
        tracker.set_dependencies(deps);

        // change-a did NOT fail
        // change-b should NOT be skipped
        assert!(tracker.should_skip("change-b").is_none());
    }

    #[test]
    fn test_should_skip_with_multiple_dependencies() {
        let mut tracker = FailedChangeTracker::new();

        // Set up: change-c depends on change-a and change-b
        let mut deps = HashMap::new();
        deps.insert(
            "change-c".to_string(),
            vec!["change-a".to_string(), "change-b".to_string()],
        );
        tracker.set_dependencies(deps);

        // Only change-b fails
        tracker.mark_failed("change-b");

        // change-c should be skipped (returns first failed dep found)
        let result = tracker.should_skip("change-c");
        assert_eq!(result, Some("change-b".to_string()));
    }
}
