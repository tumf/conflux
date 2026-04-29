use crate::tui::events::{LogEntry, TuiCommand};
use crate::tui::types::AppMode;

use super::{guards, processing_logic, AppState, ChangeState};

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

pub(super) fn toggle_selection(state: &mut AppState) -> Option<TuiCommand> {
    if state.changes.is_empty() || state.cursor_index >= state.changes.len() {
        return None;
    }

    {
        let change = &state.changes[state.cursor_index];
        if let guards::ToggleGuardResult::Blocked(msg) = guards::validate_change_toggleable(
            change.is_parallel_eligible,
            state.parallel_mode,
            &change.display_status_cache,
            &change.id,
        ) {
            state.warning_message = Some(msg);
            return None;
        }
    }

    let mode = state.mode.clone();
    let mut new_change_count = state.new_change_count;
    let result = {
        let change = &mut state.changes[state.cursor_index];
        match mode {
            AppMode::Select => guards::handle_toggle_select_mode(change, &mut new_change_count),
            AppMode::Running => guards::handle_toggle_running_mode(change, &mut new_change_count),
            AppMode::Stopped => guards::handle_toggle_stopped_mode(change, &mut new_change_count),
            AppMode::Stopping
            | AppMode::Error
            | AppMode::ConfirmWorktreeDelete
            | AppMode::QrPopup
            | AppMode::ConfirmForceKill { .. } => return None,
        }
    };
    state.new_change_count = new_change_count;

    dispatch_toggle_result(state, result)
}

fn dispatch_toggle_result(
    state: &mut AppState,
    result: guards::ToggleActionResult,
) -> Option<TuiCommand> {
    match result {
        guards::ToggleActionResult::StateOnly(log_msg) => {
            if let Some(msg) = log_msg {
                state.add_log(LogEntry::info(msg));
            }
            None
        }
        guards::ToggleActionResult::Command(cmd, log_msg) => {
            if let Some(msg) = log_msg {
                state.add_log(LogEntry::info(msg));
            }
            Some(cmd)
        }
        guards::ToggleActionResult::None => None,
    }
}

pub(super) fn start_processing(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Select {
        return None;
    }

    match processing_logic::collect_start_processing_targets(&state.changes, state.parallel_mode) {
        Ok(selected) => {
            processing_logic::mark_changes_queued(&mut state.changes, &selected);
            processing_logic::sync_queue_intent(
                state.shared_orchestrator_state.as_ref(),
                &selected,
            );
            state.reset_for_run();
            state.mode = AppMode::Running;
            state.add_log(LogEntry::info(format!(
                "Starting processing {} change(s)",
                selected.len()
            )));
            Some(processing_logic::build_start_command(selected))
        }
        Err(message) => {
            state.warning_message = Some(message);
            None
        }
    }
}

pub(super) fn resume_processing(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Stopped {
        return None;
    }

    match processing_logic::collect_resume_processing_targets(&state.changes) {
        Ok(marked_ids) => {
            processing_logic::mark_changes_queued(&mut state.changes, &marked_ids);
            processing_logic::sync_queue_intent(
                state.shared_orchestrator_state.as_ref(),
                &marked_ids,
            );
            state.reset_for_run();
            state.mode = AppMode::Running;
            state.add_log(LogEntry::info(format!(
                "Resuming processing {} change(s)...",
                marked_ids.len()
            )));
            Some(processing_logic::build_start_command(marked_ids))
        }
        Err(message) => {
            state.warning_message = Some(message);
            None
        }
    }
}

pub(super) fn retry_error_changes(state: &mut AppState) -> Option<TuiCommand> {
    if state.mode != AppMode::Error {
        return None;
    }

    let error_ids = processing_logic::collect_retry_error_targets(&state.changes);
    if error_ids.is_empty() {
        return None;
    }

    processing_logic::mark_changes_queued(&mut state.changes, &error_ids);
    processing_logic::sync_queue_intent(state.shared_orchestrator_state.as_ref(), &error_ids);
    for entry in processing_logic::emit_retry_logs(&error_ids) {
        state.add_log(entry);
    }

    state.reset_for_run();
    state.mode = AppMode::Running;
    Some(processing_logic::build_start_command(error_ids))
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
