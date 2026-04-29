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
}
