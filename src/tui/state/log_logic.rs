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
}
