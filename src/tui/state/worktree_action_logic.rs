use std::path::PathBuf;

use crate::{tui::types::WorktreeInfo, vcs::GitWorkspaceManager};
use tracing::debug;

use super::{guards, modal_logic, worktree_logic, AppState, ChangeState, TuiCommand};

/// Validate a delete request and return the identity the confirmation must carry.
///
/// The returned `(path, branch)` pair is the identity re-checked against a fresh
/// observation before anything is deleted, so it is derived through the same
/// helper the validity policy uses.
pub(super) fn validate_delete_request(
    worktrees: &[WorktreeInfo],
    worktree_cursor_index: usize,
    changes: &[ChangeState],
) -> Result<(PathBuf, Option<String>), String> {
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

    Ok((
        worktree.path.clone(),
        modal_logic::delete_branch_identity(worktree),
    ))
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
        guards::validate_not_resolving(state.is_resolving())
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

    if state.suppress_if_selected_worktree_deleting() {
        debug!("Merge blocked: selected worktree is already being deleted");
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
