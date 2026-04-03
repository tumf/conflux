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

        if reset_ids.is_empty() {
            return;
        }

        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(mut guard) = shared.try_write() {
                for id in &reset_ids {
                    guard.apply_command(
                        crate::orchestration::state::ReducerCommand::RemoveFromQueue(id.clone()),
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

    pub(crate) fn handle_error(&mut self, message: String) {
        self.add_log(LogEntry::error(message.clone()));
        self.mode = crate::tui::types::AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::remote::types::RemoteLogEntry;
    use crate::tui::events::{LogEntry, LogLevel, OrchestratorEvent};
    use crate::tui::types::AppMode;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    #[test]
    fn warning_for_uncommitted_changes_is_logged_only() {
        let changes = vec![create_test_change("change-a", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::Warning {
            title: "Uncommitted Changes Detected".to_string(),
            message: "Warning: Uncommitted changes detected.".to_string(),
        });

        assert!(app.warning_popup.is_none());
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Warning: Uncommitted")));
    }

    #[test]
    fn remote_change_update_keeps_progress_monotonic() {
        let changes = vec![create_test_change("MyProj/feat", 4, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: None,
        });

        assert_eq!(app.changes[0].completed_tasks, 4);
    }

    #[test]
    fn remote_change_update_keeps_iteration_monotonic() {
        let changes = vec![create_test_change("MyProj/feat", 1, 5)];
        let mut app = AppState::new(changes);

        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 2,
            total_tasks: 5,
            status: None,
            iteration_number: Some(3),
        });
        app.handle_orchestrator_event(OrchestratorEvent::RemoteChangeUpdate {
            id: "MyProj/feat".to_string(),
            completed_tasks: 3,
            total_tasks: 5,
            status: None,
            iteration_number: Some(2),
        });

        assert_eq!(app.changes[0].iteration_number, Some(3));
    }

    #[test]
    fn remote_log_event_is_added() {
        let mut app = AppState::new(vec![create_test_change("proj/change-a", 0, 3)]);
        let initial = app.logs.len();

        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            created_at: chrono::Utc::now(),
            message: "remote stdout: cargo build succeeded".to_string(),
            color: ratatui::style::Color::Reset,
            level: LogLevel::Info,
            change_id: Some("change-a".to_string()),
            operation: None,
            iteration: None,
            workspace_path: None,
        };

        app.handle_orchestrator_event(OrchestratorEvent::Log(entry.clone()));

        assert!(app.logs.len() > initial);
        let last = app.logs.last().expect("at least one log entry");
        assert_eq!(last.message, entry.message);
        assert_eq!(last.change_id, entry.change_id);
    }

    #[test]
    fn remote_log_entry_project_id_round_trip() {
        let entry = RemoteLogEntry {
            message: "stdout: tests passed".to_string(),
            level: "info".to_string(),
            change_id: None,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            project_id: Some("proj-abc123".to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(2),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let decoded: RemoteLogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.project_id, entry.project_id);
        assert_eq!(decoded.operation, entry.operation);
        assert_eq!(decoded.iteration, entry.iteration);
    }

    #[test]
    fn parallel_start_rejected_only_clears_target_rows() {
        let changes = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-b", 0, 1),
        ];
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        app.changes[0].display_status_cache = "queued".to_string();
        app.changes[1].display_status_cache = "queued".to_string();

        app.handle_parallel_start_rejected(
            vec!["change-a".to_string()],
            "uncommitted or not in HEAD".to_string(),
        );

        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert_eq!(app.changes[1].display_status_cache, "queued");
    }
}
