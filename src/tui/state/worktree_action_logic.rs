use crate::{tui::types::WorktreeInfo, vcs::GitWorkspaceManager};
use tracing::debug;

use super::{guards, worktree_logic, AppMode, AppState, ChangeState, TuiCommand, WorktreeAction};

pub(super) fn validate_delete_request(
    worktrees: &[WorktreeInfo],
    worktree_cursor_index: usize,
    changes: &[ChangeState],
) -> Result<(String, Option<String>), String> {
    if worktrees.is_empty() || worktree_cursor_index >= worktrees.len() {
        return Err("No worktree selected".to_string());
    }

    let worktree = &worktrees[worktree_cursor_index];

    if worktree.is_main {
        return Err("Cannot delete main worktree".to_string());
    }

    let change_id_opt = if worktree_logic::can_extract_change_id_from_worktree(worktree) {
        GitWorkspaceManager::extract_change_id_from_worktree_name(&worktree.branch)
    } else {
        None
    };

    if let Some(change_id) = change_id_opt {
        if let Some(change) = changes.iter().find(|c| c.id == change_id) {
            if worktree_logic::is_change_in_active_state(change) {
                return Err(format!(
                    "Cannot delete worktree: change '{}' is {}",
                    change_id,
                    change.display_status_cache.as_str()
                ));
            }
        }
    }

    let path_str = worktree.path.display().to_string();
    let branch_name = if !worktree.is_detached && !worktree.branch.is_empty() {
        Some(worktree.branch.clone())
    } else {
        None
    };

    Ok((path_str, branch_name))
}

pub(super) fn apply_delete_confirmation_state(
    pending_path: String,
    pending_branch: Option<String>,
    mode: &mut AppMode,
    pending_worktree_action: &mut Option<(String, WorktreeAction)>,
    pending_worktree_branch: &mut Option<String>,
    previous_mode: &mut Option<AppMode>,
) {
    *pending_worktree_action = Some((pending_path, WorktreeAction::Delete));
    *pending_worktree_branch = pending_branch;
    *previous_mode = Some(mode.clone());
    *mode = AppMode::ConfirmWorktreeDelete;
}

pub(super) fn request_merge_worktree_branch(state: &mut AppState) -> Option<TuiCommand> {
    debug!(
        "request_merge_worktree_branch called: view_mode={:?}, worktrees_len={}, cursor_index={}",
        state.view_mode,
        state.worktrees.len(),
        state.worktree_cursor_index
    );

    if let guards::MergeGuardResult::Blocked(msg) = guards::validate_view_mode(state.view_mode) {
        debug!(
            "Merge blocked: view_mode is {:?}, not Worktrees",
            state.view_mode
        );
        state.warning_message = Some(msg);
        return None;
    }

    if let guards::MergeGuardResult::Blocked(msg) =
        guards::validate_not_resolving(state.is_resolving)
    {
        debug!("Merge blocked: resolve operation in progress");
        state.warning_message = Some(msg);
        return None;
    }

    if let guards::MergeGuardResult::Blocked(msg) =
        guards::validate_worktrees_not_empty(state.worktrees.len())
    {
        debug!("Merge blocked: worktrees list is empty");
        state.warning_message = Some(msg);
        return None;
    }

    if let guards::MergeGuardResult::Blocked(msg) =
        guards::validate_cursor_in_bounds(state.worktree_cursor_index, state.worktrees.len())
    {
        debug!(
            "Merge blocked: cursor out of range: {} >= {}",
            state.worktree_cursor_index,
            state.worktrees.len()
        );
        state.warning_message = Some(msg);
        return None;
    }

    let worktree = &state.worktrees[state.worktree_cursor_index];
    debug!(
        "Worktree selected: path={}, branch={}, is_main={}, is_detached={}, has_conflict={}",
        worktree.path.display(),
        worktree.branch,
        worktree.is_main,
        worktree.is_detached,
        worktree.has_merge_conflict()
    );

    if let guards::MergeGuardResult::Blocked(msg) = guards::validate_worktree_mergeable(worktree) {
        debug!("Merge blocked: worktree validation failed");
        state.warning_message = Some(msg);
        return None;
    }

    let path = worktree.path.clone();
    let branch_name = worktree.branch.clone();

    debug!("Merge initiated: creating TuiCommand::MergeWorktreeBranch");
    Some(TuiCommand::MergeWorktreeBranch {
        worktree_path: path,
        branch_name,
    })
}
