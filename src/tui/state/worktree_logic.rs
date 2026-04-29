use crate::tui::types::WorktreeInfo;

use super::ChangeState;

pub(super) fn is_change_in_active_state(change: &ChangeState) -> bool {
    matches!(
        change.display_status_cache.as_str(),
        "queued" | "applying" | "archiving" | "resolving" | "accepting" | "merge wait"
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

    #[test]
    fn detect_active_change_statuses_for_worktree_guard() {
        let active = ChangeState {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: "applying".to_string(),
            display_color_cache: ratatui::style::Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
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
            display_color_cache: ratatui::style::Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible: true,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        };

        assert!(!is_change_in_active_state(&inactive));
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
