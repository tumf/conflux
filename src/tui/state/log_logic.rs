pub(super) fn apply_log_buffer_limit(current_len: usize, max_entries: usize) -> bool {
    current_len > max_entries
}

pub(super) fn next_log_offset_on_append(
    auto_scroll: bool,
    current_offset: usize,
    logs_len_after_append: usize,
) -> usize {
    if auto_scroll {
        return 0;
    }

    let incremented = current_offset.saturating_add(1);
    let max_offset = logs_len_after_append.saturating_sub(1);
    incremented.min(max_offset)
}

pub(super) fn scroll_logs_up(current_offset: usize, logs_len: usize, page_size: usize) -> usize {
    let max_offset = logs_len.saturating_sub(1);
    (current_offset + page_size).min(max_offset)
}

pub(super) fn scroll_logs_down(current_offset: usize, page_size: usize) -> usize {
    current_offset.saturating_sub(page_size)
}

pub(super) fn scroll_logs_to_top(logs_len: usize) -> usize {
    logs_len.saturating_sub(1)
}

pub(super) fn toggle_logs_panel(current: bool) -> bool {
    !current
}

/// Flip the presentation-only selected-proposal log filter.
pub(super) fn toggle_selected_proposal_log_filter(current: bool) -> bool {
    !current
}

/// Decide whether a cursor move must return the Logs panel to its newest
/// matching position.
///
/// Only an enabled filter whose target actually changed invalidates the current
/// entry-based scroll offset; cursor movement with the filter off leaves the
/// unfiltered scroll position alone.
pub(super) fn should_reset_log_position_for_target_change(
    filter_enabled: bool,
    previous_target: Option<&str>,
    next_target: Option<&str>,
) -> bool {
    filter_enabled && previous_target != next_target
}

/// Exact structured association between a log entry and the filter target.
///
/// Proposal identity comes only from `LogEntry::change_id`; entries without one
/// (global orchestration output) and entries carrying a different ID (including
/// remote project-only IDs) are excluded while the filter is enabled.
pub(super) fn log_entry_matches_selected_proposal(
    filter_enabled: bool,
    entry_change_id: Option<&str>,
    target_change_id: Option<&str>,
) -> bool {
    if !filter_enabled {
        return true;
    }

    match (entry_change_id, target_change_id) {
        (Some(entry_id), Some(target_id)) => entry_id == target_id,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_log_buffer_limit_only_when_exceeding_max() {
        assert!(!apply_log_buffer_limit(1000, 1000));
        assert!(apply_log_buffer_limit(1001, 1000));
    }

    #[test]
    fn next_log_offset_resets_when_auto_scroll_enabled() {
        assert_eq!(next_log_offset_on_append(true, 7, 100), 0);
    }

    #[test]
    fn next_log_offset_increments_and_clamps_when_auto_scroll_disabled() {
        assert_eq!(next_log_offset_on_append(false, 2, 10), 3);
        assert_eq!(next_log_offset_on_append(false, 10, 5), 4);
    }

    #[test]
    fn scroll_logs_up_clamps_to_oldest_available_entry() {
        assert_eq!(scroll_logs_up(0, 10, 3), 3);
        assert_eq!(scroll_logs_up(8, 10, 10), 9);
    }

    #[test]
    fn scroll_logs_down_saturates_at_bottom() {
        assert_eq!(scroll_logs_down(10, 3), 7);
        assert_eq!(scroll_logs_down(2, 10), 0);
    }

    #[test]
    fn scroll_logs_to_top_points_to_oldest_entry() {
        assert_eq!(scroll_logs_to_top(10), 9);
        assert_eq!(scroll_logs_to_top(0), 0);
    }

    #[test]
    fn toggle_logs_panel_flips_visibility_flag() {
        assert!(toggle_logs_panel(false));
        assert!(!toggle_logs_panel(true));
    }

    #[test]
    fn toggle_selected_proposal_log_filter_flips_flag() {
        assert!(toggle_selected_proposal_log_filter(false));
        assert!(!toggle_selected_proposal_log_filter(true));
    }

    #[test]
    fn cursor_move_resets_log_position_only_when_active_target_changes() {
        assert!(should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            Some("beta")
        ));
        assert!(!should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            Some("alpha")
        ));
        assert!(!should_reset_log_position_for_target_change(
            false,
            Some("alpha"),
            Some("beta")
        ));
    }

    #[test]
    fn cursor_move_resets_log_position_when_target_becomes_unavailable() {
        assert!(should_reset_log_position_for_target_change(
            true,
            Some("alpha"),
            None
        ));
        assert!(!should_reset_log_position_for_target_change(
            true, None, None
        ));
    }

    #[test]
    fn disabled_filter_keeps_every_entry_eligible() {
        assert!(log_entry_matches_selected_proposal(
            false,
            Some("beta"),
            Some("alpha")
        ));
        assert!(log_entry_matches_selected_proposal(false, None, None));
    }

    #[test]
    fn enabled_filter_matches_only_exact_structured_change_id() {
        assert!(log_entry_matches_selected_proposal(
            true,
            Some("alpha"),
            Some("alpha")
        ));
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("beta"),
            Some("alpha")
        ));
        // Remote project-only identity must not be promoted to proposal identity.
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("project-1"),
            Some("project-1::demo/alpha")
        ));
    }

    #[test]
    fn enabled_filter_excludes_unscoped_and_targetless_entries() {
        assert!(!log_entry_matches_selected_proposal(
            true,
            None,
            Some("alpha")
        ));
        assert!(!log_entry_matches_selected_proposal(
            true,
            Some("alpha"),
            None
        ));
        assert!(!log_entry_matches_selected_proposal(true, None, None));
    }
}
