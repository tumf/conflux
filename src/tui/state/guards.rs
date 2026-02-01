//! Guard logic for TUI state operations
//!
//! This module contains guard validation logic extracted from AppState methods
//! to improve readability and maintainability.

use crate::tui::events::TuiCommand;
use crate::tui::types::{QueueStatus, ViewMode, WorktreeInfo};

/// Result type for merge validation
pub enum MergeGuardResult {
    /// Merge is allowed
    Allowed,
    /// Merge is blocked with a warning message
    Blocked(String),
}

/// Validates that the view mode is correct for merge operations
pub fn validate_view_mode(view_mode: ViewMode) -> MergeGuardResult {
    if view_mode != ViewMode::Worktrees {
        MergeGuardResult::Blocked("Switch to Worktrees view to merge".to_string())
    } else {
        MergeGuardResult::Allowed
    }
}

/// Validates that no resolve operation is in progress
pub fn validate_not_resolving(is_resolving: bool) -> MergeGuardResult {
    if is_resolving {
        MergeGuardResult::Blocked("Cannot merge: resolve operation in progress".to_string())
    } else {
        MergeGuardResult::Allowed
    }
}

/// Validates that worktrees list is not empty
pub fn validate_worktrees_not_empty(worktrees_len: usize) -> MergeGuardResult {
    if worktrees_len == 0 {
        MergeGuardResult::Blocked("No worktrees loaded".to_string())
    } else {
        MergeGuardResult::Allowed
    }
}

/// Validates that cursor index is within bounds
pub fn validate_cursor_in_bounds(cursor_index: usize, worktrees_len: usize) -> MergeGuardResult {
    if cursor_index >= worktrees_len {
        MergeGuardResult::Blocked(format!(
            "Cursor out of range: {} >= {}",
            cursor_index, worktrees_len
        ))
    } else {
        MergeGuardResult::Allowed
    }
}

/// Validates worktree-specific constraints for merging
pub fn validate_worktree_mergeable(worktree: &WorktreeInfo) -> MergeGuardResult {
    // Cannot merge main worktree
    if worktree.is_main {
        return MergeGuardResult::Blocked("Cannot merge main worktree".to_string());
    }

    // Cannot merge detached HEAD
    if worktree.is_detached {
        return MergeGuardResult::Blocked("Cannot merge detached HEAD".to_string());
    }

    // Cannot merge if conflicts detected
    if worktree.has_merge_conflict() {
        return MergeGuardResult::Blocked(format!(
            "Cannot merge: {} conflict(s) detected",
            worktree.conflict_file_count()
        ));
    }

    // Branch name must not be empty
    if worktree.branch.is_empty() {
        return MergeGuardResult::Blocked("Cannot merge: no branch name".to_string());
    }

    // Cannot merge if no commits ahead of base branch
    if !worktree.has_commits_ahead {
        return MergeGuardResult::Blocked(
            "Cannot merge: no commits ahead of base branch".to_string(),
        );
    }

    // Cannot merge if already merging (redundant check after has_commits_ahead,
    // but kept for explicit validation)
    if worktree.is_merging {
        return MergeGuardResult::Blocked("Cannot merge: merge already in progress".to_string());
    }

    MergeGuardResult::Allowed
}

/// Result type for toggle selection validation
pub enum ToggleGuardResult {
    /// Operation is allowed
    Allowed,
    /// Operation is blocked with a warning message
    Blocked(String),
}

/// Validates that a change can be toggled for selection
pub fn validate_change_toggleable(
    is_approved: bool,
    is_parallel_eligible: bool,
    parallel_mode: bool,
    _queue_status: &QueueStatus,
    change_id: &str,
) -> ToggleGuardResult {
    // Cannot select unapproved changes
    if !is_approved {
        return ToggleGuardResult::Blocked(format!(
            "Cannot queue unapproved change '{}'. Press @ to approve first.",
            change_id
        ));
    }

    // Cannot select uncommitted changes in parallel mode
    if parallel_mode && !is_parallel_eligible {
        return ToggleGuardResult::Blocked(format!(
            "Cannot queue uncommitted change '{}' in parallel mode. Commit it first.",
            change_id
        ));
    }

    // MergeWait and ResolveWait can toggle execution mark (selected)
    // but cannot change queue_status or modify DynamicQueue
    // This is handled by the mode-specific handlers
    ToggleGuardResult::Allowed
}

/// Result of toggle selection action
pub enum ToggleActionResult {
    /// No command needed (state change only), with optional log message
    StateOnly(Option<String>),
    /// Return a TuiCommand, with optional log message
    Command(TuiCommand, Option<String>),
    /// Do nothing (no state change, no command)
    None,
}

/// Handle toggle selection in Select mode
pub fn handle_toggle_select_mode(
    change: &mut crate::tui::state::ChangeState,
    new_change_count: &mut usize,
) -> ToggleActionResult {
    change.selected = !change.selected;
    // Clear NEW flag when user interacts with the change
    if change.is_new {
        change.is_new = false;
        *new_change_count = new_change_count.saturating_sub(1);
    }
    ToggleActionResult::StateOnly(None)
}

/// Handle toggle selection in Running mode
pub fn handle_toggle_running_mode(
    change: &mut crate::tui::state::ChangeState,
    new_change_count: &mut usize,
) -> ToggleActionResult {
    match &change.queue_status {
        QueueStatus::NotQueued => {
            // Add to queue
            change.queue_status = QueueStatus::Queued;
            change.selected = true;
            // Clear NEW flag when user adds to queue
            if change.is_new {
                change.is_new = false;
                *new_change_count = new_change_count.saturating_sub(1);
            }
            let id = change.id.clone();
            let log_msg = format!("Added to queue: {}", id);
            ToggleActionResult::Command(TuiCommand::AddToQueue(id), Some(log_msg))
        }
        QueueStatus::Queued => {
            // Remove from queue
            change.queue_status = QueueStatus::NotQueued;
            change.selected = false;
            let id = change.id.clone();
            let log_msg = format!("Removed from queue: {}", id);
            ToggleActionResult::Command(TuiCommand::RemoveFromQueue(id), Some(log_msg))
        }
        QueueStatus::MergeWait | QueueStatus::ResolveWait => {
            // Only toggle execution mark (selected), do not modify queue_status or DynamicQueue
            change.selected = !change.selected;
            // Clear NEW flag when user interacts with the change
            if change.is_new {
                change.is_new = false;
                *new_change_count = new_change_count.saturating_sub(1);
            }
            let id = change.id.clone();
            let log_msg = if change.selected {
                format!("Marked for execution: {}", id)
            } else {
                format!("Unmarked: {}", id)
            };
            ToggleActionResult::StateOnly(Some(log_msg))
        }
        // Processing, Completed, Archived, Error - cannot change status
        _ => ToggleActionResult::None,
    }
}

/// Handle toggle selection in Stopped mode
pub fn handle_toggle_stopped_mode(
    change: &mut crate::tui::state::ChangeState,
    new_change_count: &mut usize,
) -> ToggleActionResult {
    // In Stopped mode, only toggle execution mark (selected), not queue_status.
    // For wait states (MergeWait/ResolveWait), queue_status MUST remain unchanged.
    // For NotQueued, queue_status remains NotQueued until resume.
    if !matches!(
        change.queue_status,
        QueueStatus::NotQueued | QueueStatus::MergeWait | QueueStatus::ResolveWait
    ) {
        // Cannot modify processing/completed/error states.
        return ToggleActionResult::None;
    }

    // Toggle execution mark only
    change.selected = !change.selected;

    // Clear NEW flag when user interacts with the change
    if change.is_new {
        change.is_new = false;
        *new_change_count = new_change_count.saturating_sub(1);
    }

    let id = change.id.clone();
    let log_msg = if change.selected {
        format!("Marked for execution: {}", id)
    } else {
        format!("Unmarked: {}", id)
    };
    ToggleActionResult::StateOnly(Some(log_msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::types::WorktreeInfo;
    use std::path::PathBuf;

    fn create_test_worktree() -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from("/test/path"),
            head: "abc123".to_string(),
            branch: "test-branch".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }
    }

    #[test]
    fn test_validate_view_mode_allowed() {
        let result = validate_view_mode(ViewMode::Worktrees);
        assert!(matches!(result, MergeGuardResult::Allowed));
    }

    #[test]
    fn test_validate_view_mode_blocked() {
        let result = validate_view_mode(ViewMode::Changes);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_not_resolving_allowed() {
        let result = validate_not_resolving(false);
        assert!(matches!(result, MergeGuardResult::Allowed));
    }

    #[test]
    fn test_validate_not_resolving_blocked() {
        let result = validate_not_resolving(true);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_worktrees_not_empty_allowed() {
        let result = validate_worktrees_not_empty(1);
        assert!(matches!(result, MergeGuardResult::Allowed));
    }

    #[test]
    fn test_validate_worktrees_not_empty_blocked() {
        let result = validate_worktrees_not_empty(0);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_cursor_in_bounds_allowed() {
        let result = validate_cursor_in_bounds(0, 1);
        assert!(matches!(result, MergeGuardResult::Allowed));
    }

    #[test]
    fn test_validate_cursor_in_bounds_blocked() {
        let result = validate_cursor_in_bounds(1, 1);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_worktree_mergeable_allowed() {
        let worktree = create_test_worktree();
        let result = validate_worktree_mergeable(&worktree);
        assert!(matches!(result, MergeGuardResult::Allowed));
    }

    #[test]
    fn test_validate_worktree_mergeable_blocked_main() {
        let mut worktree = create_test_worktree();
        worktree.is_main = true;
        let result = validate_worktree_mergeable(&worktree);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_worktree_mergeable_blocked_detached() {
        let mut worktree = create_test_worktree();
        worktree.is_detached = true;
        let result = validate_worktree_mergeable(&worktree);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_worktree_mergeable_blocked_no_branch() {
        let mut worktree = create_test_worktree();
        worktree.branch = String::new();
        let result = validate_worktree_mergeable(&worktree);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_worktree_mergeable_blocked_no_commits_ahead() {
        let mut worktree = create_test_worktree();
        worktree.has_commits_ahead = false;
        let result = validate_worktree_mergeable(&worktree);
        assert!(matches!(result, MergeGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_change_toggleable_allowed() {
        let result = validate_change_toggleable(
            true,                    // is_approved
            true,                    // is_parallel_eligible
            false,                   // parallel_mode
            &QueueStatus::NotQueued, // queue_status
            "test-change",           // change_id
        );
        assert!(matches!(result, ToggleGuardResult::Allowed));
    }

    #[test]
    fn test_validate_change_toggleable_blocked_unapproved() {
        let result = validate_change_toggleable(
            false,                   // is_approved (blocked)
            true,                    // is_parallel_eligible
            false,                   // parallel_mode
            &QueueStatus::NotQueued, // queue_status
            "test-change",           // change_id
        );
        assert!(matches!(result, ToggleGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_change_toggleable_blocked_parallel_uncommitted() {
        let result = validate_change_toggleable(
            true,                    // is_approved
            false,                   // is_parallel_eligible (blocked in parallel mode)
            true,                    // parallel_mode (enabled)
            &QueueStatus::NotQueued, // queue_status
            "test-change",           // change_id
        );
        assert!(matches!(result, ToggleGuardResult::Blocked(_)));
    }

    #[test]
    fn test_validate_change_toggleable_allows_resolve_wait() {
        // ResolveWait should now be allowed (only selected toggles, no queue change)
        let result = validate_change_toggleable(
            true,                      // is_approved
            true,                      // is_parallel_eligible
            false,                     // parallel_mode
            &QueueStatus::ResolveWait, // queue_status (now allowed)
            "test-change",             // change_id
        );
        assert!(matches!(result, ToggleGuardResult::Allowed));
    }

    #[test]
    fn test_validate_change_toggleable_allows_merge_wait() {
        // MergeWait should be allowed (only selected toggles, no queue change)
        let result = validate_change_toggleable(
            true,                    // is_approved
            true,                    // is_parallel_eligible
            false,                   // parallel_mode
            &QueueStatus::MergeWait, // queue_status (allowed)
            "test-change",           // change_id
        );
        assert!(matches!(result, ToggleGuardResult::Allowed));
    }

    #[test]
    fn test_handle_toggle_running_mode_merge_wait_toggles_selected_only() {
        use crate::openspec::Change;
        use crate::tui::state::ChangeState;

        let change_data = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let mut change = ChangeState::from_change(&change_data, true);
        change.queue_status = QueueStatus::MergeWait;
        change.selected = false;

        let mut new_change_count = 0;

        // Toggle should only change selected, not queue_status
        let result = handle_toggle_running_mode(&mut change, &mut new_change_count);

        assert!(change.selected); // Selected toggled to true
        assert!(matches!(change.queue_status, QueueStatus::MergeWait)); // Queue status unchanged
        assert!(matches!(result, ToggleActionResult::StateOnly(_))); // No command issued
    }

    #[test]
    fn test_handle_toggle_running_mode_resolve_wait_toggles_selected_only() {
        use crate::openspec::Change;
        use crate::tui::state::ChangeState;

        let change_data = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let mut change = ChangeState::from_change(&change_data, true);
        change.queue_status = QueueStatus::ResolveWait;
        change.selected = true;

        let mut new_change_count = 0;

        // Toggle should only change selected, not queue_status
        let result = handle_toggle_running_mode(&mut change, &mut new_change_count);

        assert!(!change.selected); // Selected toggled to false
        assert!(matches!(change.queue_status, QueueStatus::ResolveWait)); // Queue status unchanged
        assert!(matches!(result, ToggleActionResult::StateOnly(_))); // No command issued
    }

    #[test]
    fn test_handle_toggle_running_mode_merge_wait_clears_new_flag() {
        use crate::openspec::Change;
        use crate::tui::state::ChangeState;

        let change_data = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let mut change = ChangeState::from_change(&change_data, true);
        change.queue_status = QueueStatus::MergeWait;
        change.is_new = true;
        change.selected = false;

        let mut new_change_count = 1;

        // Toggle should clear new flag
        let _ = handle_toggle_running_mode(&mut change, &mut new_change_count);

        assert!(!change.is_new); // New flag cleared
        assert_eq!(new_change_count, 0); // Counter decremented
    }

    #[test]
    fn test_handle_toggle_stopped_mode_merge_wait_toggles_selected_only() {
        use crate::openspec::Change;
        use crate::tui::state::ChangeState;

        let change_data = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let mut change = ChangeState::from_change(&change_data, true);
        change.queue_status = QueueStatus::MergeWait;
        change.selected = false;

        let mut new_change_count = 0;
        let result = handle_toggle_stopped_mode(&mut change, &mut new_change_count);

        assert!(change.selected);
        assert!(matches!(change.queue_status, QueueStatus::MergeWait));
        assert!(matches!(result, ToggleActionResult::StateOnly(_)));
    }

    #[test]
    fn test_handle_toggle_stopped_mode_resolve_wait_toggles_selected_only() {
        use crate::openspec::Change;
        use crate::tui::state::ChangeState;

        let change_data = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let mut change = ChangeState::from_change(&change_data, true);
        change.queue_status = QueueStatus::ResolveWait;
        change.selected = true;

        let mut new_change_count = 0;
        let result = handle_toggle_stopped_mode(&mut change, &mut new_change_count);

        assert!(!change.selected);
        assert!(matches!(change.queue_status, QueueStatus::ResolveWait));
        assert!(matches!(result, ToggleActionResult::StateOnly(_)));
    }
}
