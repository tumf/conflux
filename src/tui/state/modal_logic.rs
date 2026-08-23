//! Variant-specific modal validity policy.
//!
//! A modal is advisory presentation state, so it may outlive the execution
//! transition it was opened over. What it must not outlive is its own target:
//! this module decides, from a fresh observation, whether the active modal still
//! describes something the operator can act on. The functions here are pure so
//! the whole matrix is testable without a terminal, a repository, or a runtime.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::orchestration::operator_command::{is_active_status, is_final_status};
use crate::tui::types::{AppExecutionMode, ModalState, WorktreeInfo};
use crate::vcs::GitWorkspaceManager;

use super::{worktree_logic, ChangeState};

/// Fresh observation a modal's validity is judged against.
///
/// Every field is a current view of authoritative-enough state; nothing here is
/// captured at the moment the modal opened.
pub(crate) struct ModalValidityContext<'a> {
    /// Current execution lifecycle mode.
    pub(crate) execution_mode: AppExecutionMode,
    /// Current Web UI URL, when web monitoring is enabled.
    pub(crate) web_url: Option<&'a str>,
    /// Latest observed worktrees.
    pub(crate) worktrees: &'a [WorktreeInfo],
    /// Latest observed change rows.
    pub(crate) changes: &'a [ChangeState],
    /// Worktrees with an accepted delete already in flight.
    pub(crate) deleting_worktree_paths: &'a HashSet<PathBuf>,
}

/// Why the active modal no longer describes an actionable target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalInvalidation {
    /// Web monitoring was disabled or the URL was removed.
    WebUrlUnavailable,
    /// The worktree the confirmation targets is gone.
    WorktreeTargetAbsent,
    /// The path now resolves to the main worktree.
    WorktreeTargetIsMain,
    /// The worktree's change became active again.
    WorktreeTargetActive,
    /// A delete for this worktree is already in flight.
    WorktreeTargetDeleting,
    /// The path now carries a different branch identity.
    WorktreeIdentityChanged,
    /// The worktree moved to a different commit since the confirmation opened.
    WorktreeHeadMoved,
    /// The force-kill target is no longer present in the change list.
    ForceKillTargetAbsent,
    /// The force-kill target reached a final status.
    ForceKillTargetFinal,
    /// The force-kill target is no longer active work (dequeued or idle).
    ForceKillTargetNotActive,
    /// Global execution state no longer admits stop-and-dequeue.
    ForceKillExecutionInvalid,
}

impl ModalInvalidation {
    /// Operator-facing reason, used when a confirmation is refused at input time.
    pub(crate) fn reason(self) -> &'static str {
        match self {
            ModalInvalidation::WebUrlUnavailable => "web monitoring is no longer available",
            ModalInvalidation::WorktreeTargetAbsent => "the worktree no longer exists",
            ModalInvalidation::WorktreeTargetIsMain => "the target is the main worktree",
            ModalInvalidation::WorktreeTargetActive => "its change became active again",
            ModalInvalidation::WorktreeTargetDeleting => "a delete is already in progress",
            ModalInvalidation::WorktreeIdentityChanged => {
                "the worktree now carries a different branch"
            }
            ModalInvalidation::WorktreeHeadMoved => "the worktree moved to a different commit",
            ModalInvalidation::ForceKillTargetAbsent => "the change is no longer listed",
            ModalInvalidation::ForceKillTargetFinal => "the change already reached a final status",
            ModalInvalidation::ForceKillTargetNotActive => "the change is no longer active work",
            ModalInvalidation::ForceKillExecutionInvalid => {
                "no run is active to stop and dequeue from"
            }
        }
    }
}

/// Branch identity a delete confirmation stores for a worktree observation.
///
/// The confirmation payload and a fresh observation must agree on this value, so
/// deriving it in one place is what makes "same worktree" mean the same thing at
/// open time, at invalidation time, and at confirmation time.
///
/// `None` means this observation has no branch to name — a detached HEAD. Such a
/// worktree carries nothing that can be revalidated later, so it never becomes a
/// confirmation payload and never matches one.
pub(crate) fn delete_branch_identity(worktree: &WorktreeInfo) -> Option<String> {
    (!worktree.is_detached && !worktree.branch.is_empty()).then(|| worktree.branch.clone())
}

/// Locate a worktree by path in a fresh observation.
pub(crate) fn find_worktree<'a>(
    worktrees: &'a [WorktreeInfo],
    path: &Path,
) -> Option<&'a WorktreeInfo> {
    worktrees.iter().find(|worktree| worktree.path == path)
}

/// Whether a worktree-delete confirmation still targets the same eligible worktree.
pub(crate) fn evaluate_worktree_delete(
    path: &Path,
    branch: &str,
    ctx: &ModalValidityContext<'_>,
) -> Result<(), ModalInvalidation> {
    let Some(worktree) = find_worktree(ctx.worktrees, path) else {
        return Err(ModalInvalidation::WorktreeTargetAbsent);
    };

    if worktree.is_main {
        return Err(ModalInvalidation::WorktreeTargetIsMain);
    }

    if ctx.deleting_worktree_paths.contains(&worktree.path) {
        return Err(ModalInvalidation::WorktreeTargetDeleting);
    }

    // A detached observation yields `None` here, so it can never equal the
    // confirmed branch: re-detaching the path invalidates the confirmation.
    if delete_branch_identity(worktree).as_deref() != Some(branch) {
        return Err(ModalInvalidation::WorktreeIdentityChanged);
    }

    if worktree_change_is_active(worktree, ctx.changes) {
        return Err(ModalInvalidation::WorktreeTargetActive);
    }

    Ok(())
}

/// Whether a destructive discard confirmation still targets the worktree it was
/// opened over.
///
/// This is the ordinary delete policy plus one more anchor: the destructive
/// confirmation was escalated from a specific observed commit, so a target that
/// moved its HEAD in the meantime is no longer the thing the operator agreed to
/// discard. That anchor matters even more for an ahead discard, where the
/// confirmed commit is also the OID the branch is deleted at. The check is
/// necessary but not sufficient — the shared service revalidates the same
/// identity under its own mutation guard, where an active-state transition after
/// this point is still visible.
pub(crate) fn evaluate_discard(
    path: &Path,
    branch: &str,
    head: &str,
    ctx: &ModalValidityContext<'_>,
) -> Result<(), ModalInvalidation> {
    evaluate_worktree_delete(path, branch, ctx)?;

    let Some(worktree) = find_worktree(ctx.worktrees, path) else {
        return Err(ModalInvalidation::WorktreeTargetAbsent);
    };
    if worktree.head != head {
        return Err(ModalInvalidation::WorktreeHeadMoved);
    }

    Ok(())
}

/// Whether a force-kill confirmation still targets retryable active work.
pub(crate) fn evaluate_force_kill(
    change_id: &str,
    ctx: &ModalValidityContext<'_>,
) -> Result<(), ModalInvalidation> {
    // Stop-and-dequeue only exists inside an active or stopping run; Select,
    // Stopped, and Error have no in-flight task to terminate.
    if !matches!(
        ctx.execution_mode,
        AppExecutionMode::Running | AppExecutionMode::Stopping
    ) {
        return Err(ModalInvalidation::ForceKillExecutionInvalid);
    }

    let Some(change) = ctx.changes.iter().find(|change| change.id == change_id) else {
        return Err(ModalInvalidation::ForceKillTargetAbsent);
    };

    let status = change.display_status_cache.as_str();
    if is_final_status(status) {
        return Err(ModalInvalidation::ForceKillTargetFinal);
    }
    if !is_active_status(status) {
        return Err(ModalInvalidation::ForceKillTargetNotActive);
    }

    Ok(())
}

/// Whether the active modal still describes an actionable target.
pub(crate) fn evaluate(
    modal: &ModalState,
    ctx: &ModalValidityContext<'_>,
) -> Result<(), ModalInvalidation> {
    match modal {
        ModalState::QrPopup => {
            if ctx.web_url.is_some() {
                Ok(())
            } else {
                Err(ModalInvalidation::WebUrlUnavailable)
            }
        }
        ModalState::ConfirmWorktreeDelete { path, branch } => {
            evaluate_worktree_delete(path, branch, ctx)
        }
        ModalState::ConfirmDirtyDiscard {
            path, branch, head, ..
        }
        | ModalState::ConfirmAheadDiscard {
            path, branch, head, ..
        } => evaluate_discard(path, branch, head, ctx),
        ModalState::ConfirmForceKill { change_id } => evaluate_force_kill(change_id, ctx),
    }
}

/// Whether the change bound to a worktree branch is currently active.
fn worktree_change_is_active(worktree: &WorktreeInfo, changes: &[ChangeState]) -> bool {
    if !worktree_logic::can_extract_change_id_from_worktree(worktree) {
        return false;
    }

    let Some(change_id) =
        GitWorkspaceManager::extract_change_id_from_worktree_name(&worktree.branch)
    else {
        return false;
    };

    changes
        .iter()
        .find(|change| change.id == change_id)
        .is_some_and(worktree_logic::is_change_in_active_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::operator_command::ParallelEligibility;
    use ratatui::style::Color;

    fn change(id: &str, status: &str) -> ChangeState {
        ChangeState {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: status.to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
            display_color_cache: Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
            apply_operation_cache: "apply".to_string(),
            archive_complete_cache: false,
        }
    }

    fn worktree(path: &str, branch: &str) -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from(path),
            head: "abc1234".to_string(),
            branch: branch.to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
            inspection: crate::worktree_ops::InspectionState::Checked,
        }
    }

    struct Fixture {
        worktrees: Vec<WorktreeInfo>,
        changes: Vec<ChangeState>,
        deleting: HashSet<PathBuf>,
        web_url: Option<String>,
        execution_mode: AppExecutionMode,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                worktrees: vec![worktree("/tmp/wt-a", "change-a")],
                changes: vec![change("change-a", "not queued")],
                deleting: HashSet::new(),
                web_url: Some("http://127.0.0.1:8080".to_string()),
                execution_mode: AppExecutionMode::Running,
            }
        }

        fn ctx(&self) -> ModalValidityContext<'_> {
            ModalValidityContext {
                execution_mode: self.execution_mode,
                web_url: self.web_url.as_deref(),
                worktrees: &self.worktrees,
                changes: &self.changes,
                deleting_worktree_paths: &self.deleting,
            }
        }
    }

    fn delete_modal() -> ModalState {
        ModalState::ConfirmWorktreeDelete {
            path: PathBuf::from("/tmp/wt-a"),
            branch: "change-a".to_string(),
        }
    }

    fn kill_modal() -> ModalState {
        ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        }
    }

    #[test]
    fn qr_survives_every_execution_transition_while_the_url_remains() {
        let mut fixture = Fixture::new();
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            fixture.execution_mode = mode;
            assert_eq!(evaluate(&ModalState::QrPopup, &fixture.ctx()), Ok(()));
        }
    }

    #[test]
    fn qr_invalidates_when_the_web_url_disappears() {
        let mut fixture = Fixture::new();
        fixture.web_url = None;

        assert_eq!(
            evaluate(&ModalState::QrPopup, &fixture.ctx()),
            Err(ModalInvalidation::WebUrlUnavailable)
        );
    }

    #[test]
    fn worktree_delete_survives_execution_transitions_while_identity_holds() {
        let mut fixture = Fixture::new();
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            fixture.execution_mode = mode;
            assert_eq!(evaluate(&delete_modal(), &fixture.ctx()), Ok(()));
        }
    }

    #[test]
    fn worktree_delete_invalidation_boundaries() {
        // Target disappeared.
        let mut absent = Fixture::new();
        absent.worktrees.clear();
        assert_eq!(
            evaluate(&delete_modal(), &absent.ctx()),
            Err(ModalInvalidation::WorktreeTargetAbsent)
        );

        // Path now resolves to the main worktree.
        let mut main = Fixture::new();
        main.worktrees[0].is_main = true;
        assert_eq!(
            evaluate(&delete_modal(), &main.ctx()),
            Err(ModalInvalidation::WorktreeTargetIsMain)
        );

        // A delete for the same path is already in flight.
        let mut deleting = Fixture::new();
        deleting.deleting.insert(PathBuf::from("/tmp/wt-a"));
        assert_eq!(
            evaluate(&delete_modal(), &deleting.ctx()),
            Err(ModalInvalidation::WorktreeTargetDeleting)
        );

        // Same path, different branch identity.
        let mut rebranded = Fixture::new();
        rebranded.worktrees[0].branch = "change-z".to_string();
        assert_eq!(
            evaluate(&delete_modal(), &rebranded.ctx()),
            Err(ModalInvalidation::WorktreeIdentityChanged)
        );

        // Same path, branch identity dropped by detaching HEAD.
        let mut detached = Fixture::new();
        detached.worktrees[0].is_detached = true;
        assert_eq!(
            evaluate(&delete_modal(), &detached.ctx()),
            Err(ModalInvalidation::WorktreeIdentityChanged)
        );

        // Same path, branch name no longer observable at all.
        let mut nameless = Fixture::new();
        nameless.worktrees[0].branch = String::new();
        assert_eq!(
            evaluate(&delete_modal(), &nameless.ctx()),
            Err(ModalInvalidation::WorktreeIdentityChanged)
        );

        // The bound change became active again.
        let mut active = Fixture::new();
        active.changes[0].set_display_status_cache("applying");
        assert_eq!(
            evaluate(&delete_modal(), &active.ctx()),
            Err(ModalInvalidation::WorktreeTargetActive)
        );
    }

    /// A named mutation and the invalidation it must produce.
    type InvalidationCase = (&'static str, fn(&mut Fixture), ModalInvalidation);

    fn dirty_discard_modal() -> ModalState {
        ModalState::ConfirmDirtyDiscard {
            path: PathBuf::from("/tmp/wt-a"),
            identity: "gitdir: /tmp/wt-a/.git".to_string(),
            branch: "change-a".to_string(),
            head: "abc1234".to_string(),
            skip_teardown: false,
        }
    }

    #[test]
    fn tui_dirty_worktree_delete_confirmation_survives_while_the_target_holds_still() {
        let mut fixture = Fixture::new();
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            fixture.execution_mode = mode;
            assert_eq!(evaluate(&dirty_discard_modal(), &fixture.ctx()), Ok(()));
        }
    }

    #[test]
    fn tui_dirty_worktree_delete_confirmation_inherits_every_delete_invalidation() {
        // A destructive confirmation may not be *easier* to satisfy than the
        // ordinary one it was escalated from.
        let cases: [InvalidationCase; 6] = [
            (
                "absent",
                |f: &mut Fixture| f.worktrees.clear(),
                ModalInvalidation::WorktreeTargetAbsent,
            ),
            (
                "main",
                |f: &mut Fixture| f.worktrees[0].is_main = true,
                ModalInvalidation::WorktreeTargetIsMain,
            ),
            (
                "deleting",
                |f: &mut Fixture| {
                    f.deleting.insert(PathBuf::from("/tmp/wt-a"));
                },
                ModalInvalidation::WorktreeTargetDeleting,
            ),
            (
                "rebranded",
                |f: &mut Fixture| f.worktrees[0].branch = "change-z".to_string(),
                ModalInvalidation::WorktreeIdentityChanged,
            ),
            (
                "detached",
                |f: &mut Fixture| f.worktrees[0].is_detached = true,
                ModalInvalidation::WorktreeIdentityChanged,
            ),
            (
                "active",
                |f: &mut Fixture| f.changes[0].set_display_status_cache("applying"),
                ModalInvalidation::WorktreeTargetActive,
            ),
        ];

        for (name, mutate, expected) in cases {
            let mut fixture = Fixture::new();
            mutate(&mut fixture);
            assert_eq!(
                evaluate(&dirty_discard_modal(), &fixture.ctx()),
                Err(expected),
                "{name}: a destructive confirmation must invalidate like an ordinary one"
            );
        }
    }

    #[test]
    fn tui_dirty_worktree_delete_confirmation_invalidates_when_head_moves() {
        // The escalation named a specific commit. A worktree that moved is no
        // longer the thing the operator was shown and agreed to discard.
        let mut moved = Fixture::new();
        moved.worktrees[0].head = "def5678".to_string();

        assert_eq!(
            evaluate(&dirty_discard_modal(), &moved.ctx()),
            Err(ModalInvalidation::WorktreeHeadMoved)
        );
    }

    #[test]
    fn force_kill_survives_running_to_stopping_while_the_target_is_active() {
        let mut fixture = Fixture::new();
        fixture.changes[0].set_display_status_cache("applying");

        for mode in [AppExecutionMode::Running, AppExecutionMode::Stopping] {
            fixture.execution_mode = mode;
            assert_eq!(evaluate(&kill_modal(), &fixture.ctx()), Ok(()));
        }
    }

    #[test]
    fn force_kill_invalidation_boundaries() {
        // Target no longer listed.
        let mut absent = Fixture::new();
        absent.changes.clear();
        assert_eq!(
            evaluate(&kill_modal(), &absent.ctx()),
            Err(ModalInvalidation::ForceKillTargetAbsent)
        );

        // Target reached a final status.
        let mut final_status = Fixture::new();
        final_status.changes[0].set_display_status_cache("archived");
        assert_eq!(
            evaluate(&kill_modal(), &final_status.ctx()),
            Err(ModalInvalidation::ForceKillTargetFinal)
        );

        // Target dequeued back to idle.
        let dequeued = Fixture::new();
        assert_eq!(
            evaluate(&kill_modal(), &dequeued.ctx()),
            Err(ModalInvalidation::ForceKillTargetNotActive)
        );

        // Global execution state no longer admits stop-and-dequeue.
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut invalid = Fixture::new();
            invalid.execution_mode = mode;
            invalid.changes[0].set_display_status_cache("applying");
            assert_eq!(
                evaluate(&kill_modal(), &invalid.ctx()),
                Err(ModalInvalidation::ForceKillExecutionInvalid),
                "{:?} must not keep a force-kill confirmation alive",
                mode
            );
        }
    }

    #[test]
    fn every_invalidation_reason_is_reportable() {
        for reason in [
            ModalInvalidation::WebUrlUnavailable,
            ModalInvalidation::WorktreeTargetAbsent,
            ModalInvalidation::WorktreeTargetIsMain,
            ModalInvalidation::WorktreeTargetActive,
            ModalInvalidation::WorktreeTargetDeleting,
            ModalInvalidation::WorktreeIdentityChanged,
            ModalInvalidation::WorktreeHeadMoved,
            ModalInvalidation::ForceKillTargetAbsent,
            ModalInvalidation::ForceKillTargetFinal,
            ModalInvalidation::ForceKillTargetNotActive,
            ModalInvalidation::ForceKillExecutionInvalid,
        ] {
            assert!(!reason.reason().is_empty());
        }
    }
}
