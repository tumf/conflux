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
}
