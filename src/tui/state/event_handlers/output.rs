use crate::tui::events::LogEntry;

use crate::tui::state::WarningPopup;

use super::AppState;

impl AppState {
    pub(crate) fn handle_apply_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "applying") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("apply")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_archive_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: u32,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "archiving") {
                change.update_iteration_monotonic(Some(iteration));
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("archive")
                .with_iteration(iteration),
        );
    }

    pub(crate) fn handle_acceptance_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "accepting") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(change_id)
                .with_operation("acceptance")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_analysis_output(&mut self, output: String, iteration: u32) {
        self.add_log(
            LogEntry::info(output)
                .with_operation("analysis")
                .with_iteration(iteration),
        );
    }

    pub(crate) fn handle_resolve_output(
        &mut self,
        change_id: String,
        output: String,
        iteration: Option<u32>,
    ) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
            if matches!(change.display_status_cache.as_str(), "resolving") {
                change.update_iteration_monotonic(iteration);
            }
        }

        self.add_log(
            LogEntry::info(output)
                .with_change_id(&change_id)
                .with_operation("resolve")
                .with_iteration(iteration.unwrap_or(1)),
        );
    }

    pub(crate) fn handle_log(&mut self, entry: LogEntry) {
        self.add_log(entry);
    }

    pub(crate) fn handle_warning(&mut self, title: String, message: String) {
        if title != "Uncommitted Changes Detected" {
            self.warning_popup = Some(WarningPopup {
                title,
                message: message.clone(),
            });
        }
        self.add_log(LogEntry::warn(message));
    }

    pub(crate) fn handle_parallel_start_rejected(
        &mut self,
        change_ids: Vec<String>,
        reason: String,
    ) {
        let mut reset_ids = Vec::new();
        for change in &mut self.changes {
            if change_ids.contains(&change.id)
                && matches!(change.display_status_cache.as_str(), "queued")
            {
                change.set_display_status_cache("not queued");
                reset_ids.push(change.id.clone());
            }
        }

        if !reset_ids.is_empty() {
            if let Some(shared) = &self.shared_orchestrator_state {
                if let Ok(mut guard) = shared.try_write() {
                    for id in &reset_ids {
                        guard.apply_command(
                            crate::orchestration::state::ReducerCommand::RemoveFromQueue(
                                id.clone(),
                            ),
                        );
                    }
                }
            }
            self.add_log(LogEntry::warn(format!(
                "Not started ({}): {}",
                reason,
                reset_ids.join(", ")
            )));
        }
    }

    pub(crate) fn handle_error(&mut self, message: String) {
        self.add_log(LogEntry::error(message.clone()));
        self.mode = crate::tui::types::AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}
