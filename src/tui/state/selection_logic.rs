use crate::tui::types::AppMode;

use super::{guards, ChangeState};

pub(super) fn can_bulk_toggle_change(
    mode: AppMode,
    parallel_mode: bool,
    change: &ChangeState,
) -> bool {
    if matches!(mode, AppMode::Running) && change.is_active_display_status() {
        return false;
    }

    guards::validate_change_toggleable(
        change.is_parallel_eligible,
        parallel_mode,
        &change.display_status_cache,
        &change.id,
    )
    .is_allowed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn make_change_state(
        id: &str,
        display_status_cache: &str,
        is_parallel_eligible: bool,
    ) -> ChangeState {
        ChangeState {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            display_status_cache: display_status_cache.to_string(),
            display_color_cache: Color::DarkGray,
            error_message_cache: None,
            selected: false,
            is_new: false,
            is_parallel_eligible,
            has_worktree: false,
            started_at: None,
            elapsed_time: None,
            iteration_number: None,
        }
    }

    #[test]
    fn running_mode_excludes_active_rows_from_bulk_toggle() {
        let change = make_change_state("active", "applying", true);

        assert!(!can_bulk_toggle_change(AppMode::Running, false, &change));
        assert!(can_bulk_toggle_change(AppMode::Select, false, &change));
    }

    #[test]
    fn parallel_mode_excludes_uncommitted_rows_from_bulk_toggle() {
        let ineligible = make_change_state("uncommitted", "not queued", false);

        assert!(!can_bulk_toggle_change(AppMode::Select, true, &ineligible));
        assert!(can_bulk_toggle_change(AppMode::Select, false, &ineligible));
    }
}
