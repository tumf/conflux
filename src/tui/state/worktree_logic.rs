use crate::tui::types::WorktreeInfo;

use super::ChangeState;

/// Whether a change still owns its managed worktree, so deleting it would race
/// live work.
///
/// `preparing` is included for the strongest reason of all: that is exactly the
/// window in which Conflux is itself creating, recreating, or running
/// `.wt/setup` inside the worktree an operator is asking to delete.
pub(super) fn is_change_in_active_state(change: &ChangeState) -> bool {
    matches!(
        change.display_status_cache.as_str(),
        "queued"
            | "preparing"
            | "applying"
            | "archiving"
            | "resolving"
            | "accepting"
            | "merge wait"
    )
}

pub(super) fn can_extract_change_id_from_worktree(worktree: &WorktreeInfo) -> bool {
    !worktree.branch.is_empty() && !worktree.is_detached
}

pub(super) fn previous_worktree_cursor_index(current: usize, worktree_len: usize) -> Option<usize> {
    if worktree_len == 0 {
        return None;
    }

    Some(if current == 0 {
        worktree_len - 1
    } else {
        current - 1
    })
}

pub(super) fn next_worktree_cursor_index(current: usize, worktree_len: usize) -> Option<usize> {
    if worktree_len == 0 {
        return None;
    }

    Some((current + 1) % worktree_len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::operator_command::ParallelEligibility;

    #[test]
    fn detect_active_change_statuses_for_worktree_guard() {
        let active = ChangeState {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "applying".to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
            display_color_cache: ratatui::style::Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert!(is_change_in_active_state(&active));

        let inactive = ChangeState {
            id: "change-b".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "not queued".to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
            display_color_cache: ratatui::style::Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert!(!is_change_in_active_state(&inactive));
    }

    /// Deleting the worktree Conflux is currently building is the one race this
    /// guard exists to prevent, so `preparing` must never fall through it.
    #[test]
    fn preparing_is_active_for_the_worktree_delete_guard() {
        let preparing = ChangeState {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "preparing".to_string(),
            blocker_kind_cache: crate::orchestration::state::BlockerKind::None,
            blocker_detail_cache: None,
            display_color_cache: ratatui::style::Color::Green,
            error_message_cache: None,
            selected: false,
            is_new: false,
            parallel_eligibility: ParallelEligibility::Eligible,
            has_worktree: true,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert!(is_change_in_active_state(&preparing));
    }

    #[test]
    fn allow_change_id_extraction_check() {
        let branch_set = WorktreeInfo {
            path: Default::default(),
            head: String::new(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };

        let detached = WorktreeInfo {
            path: Default::default(),
            head: String::new(),
            branch: "main".to_string(),
            is_detached: true,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };

        assert!(can_extract_change_id_from_worktree(&branch_set));
        assert!(!can_extract_change_id_from_worktree(&detached));
    }

    #[test]
    fn previous_worktree_cursor_index_wraps_when_at_top() {
        assert_eq!(previous_worktree_cursor_index(0, 3), Some(2));
        assert_eq!(previous_worktree_cursor_index(2, 3), Some(1));
    }

    #[test]
    fn previous_worktree_cursor_index_returns_none_for_empty_list() {
        assert_eq!(previous_worktree_cursor_index(0, 0), None);
    }

    #[test]
    fn next_worktree_cursor_index_wraps_when_at_bottom() {
        assert_eq!(next_worktree_cursor_index(2, 3), Some(0));
        assert_eq!(next_worktree_cursor_index(0, 3), Some(1));
    }

    #[test]
    fn next_worktree_cursor_index_returns_none_for_empty_list() {
        assert_eq!(next_worktree_cursor_index(0, 0), None);
    }
}
