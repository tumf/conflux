//! Mode-related methods for AppState
//!
//! Contains mode switching logic like start_processing and toggle_parallel_mode.

use std::time::Instant;

use super::super::events::{LogEntry, TuiCommand};
use super::super::types::{AppMode, QueueStatus, StopMode};
use super::AppState;

impl AppState {
    /// Toggle parallel mode (only if git is available)
    ///
    /// Returns true if the mode was toggled, false if git is not available
    /// or if the mode cannot be changed in current state.
    pub fn toggle_parallel_mode(&mut self) -> bool {
        // Only allow toggling in Select or Stopped mode
        if !matches!(self.mode, AppMode::Select | AppMode::Stopped) {
            self.warning_message = Some("Cannot toggle parallel mode while processing".to_string());
            return false;
        }

        // Check if parallel execution is available (git)
        if !self.parallel_available {
            self.warning_message = Some("Parallel mode not available (requires git)".to_string());
            return false;
        }

        self.parallel_mode = !self.parallel_mode;
        let status = if self.parallel_mode {
            "enabled"
        } else {
            "disabled"
        };

        if self.parallel_mode {
            let mut removed = Vec::new();
            for change in &mut self.changes {
                if !change.is_parallel_eligible && change.selected {
                    change.selected = false;
                    if matches!(change.queue_status, QueueStatus::Queued) {
                        change.queue_status = QueueStatus::NotQueued;
                    }
                    removed.push(change.id.clone());
                }
            }
            if !removed.is_empty() {
                self.warning_message = Some(format!(
                    "Removed uncommitted changes from queue in parallel mode: {}",
                    removed.join(", ")
                ));
            }
        }

        self.add_log(LogEntry::info(format!("Parallel mode {}", status)));
        true
    }

    /// Reset stop/cancel state before a new run
    pub fn reset_for_run(&mut self) {
        self.stop_mode = StopMode::None;
        self.current_change = None;
        self.error_change_id = None;
        self.orchestration_started_at = Some(Instant::now());
        self.orchestration_elapsed = None;
    }

    /// Start processing selected changes
    pub fn start_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Select {
            return None;
        }

        let selected: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected)
            .map(|c| c.id.clone())
            .collect();

        if self.parallel_mode {
            let ineligible: Vec<String> = self
                .changes
                .iter()
                .filter(|c| c.selected && !c.is_parallel_eligible)
                .map(|c| c.id.clone())
                .collect();
            if !ineligible.is_empty() {
                self.warning_message = Some(format!(
                    "Parallel mode requires committed changes. Uncommitted: {}",
                    ineligible.join(", ")
                ));
                return None;
            }
        }

        if selected.is_empty() {
            self.warning_message = Some("No changes selected".to_string());
            return None;
        }

        // Mark selected changes as queued
        for change in &mut self.changes {
            if change.selected {
                change.queue_status = QueueStatus::Queued;
            }
        }

        self.reset_for_run();
        self.mode = AppMode::Running;
        self.add_log(LogEntry::info(format!(
            "Starting processing {} change(s)",
            selected.len()
        )));

        Some(TuiCommand::StartProcessing(selected))
    }

    /// Resume processing from Stopped mode
    /// Converts execution-marked (selected) changes to Queued and starts processing
    pub fn resume_processing(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Stopped {
            return None;
        }

        // Find execution-marked changes (selected=true, queue_status=NotQueued)
        let marked_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| c.selected && matches!(c.queue_status, QueueStatus::NotQueued))
            .map(|c| c.id.clone())
            .collect();

        if marked_ids.is_empty() {
            self.warning_message = Some("No changes marked for execution".to_string());
            return None;
        }

        // Convert execution-marked changes to Queued
        for change in &mut self.changes {
            if marked_ids.contains(&change.id) {
                change.queue_status = QueueStatus::Queued;
            }
        }

        self.reset_for_run();
        self.mode = AppMode::Running;
        self.add_log(LogEntry::info(format!(
            "Resuming processing {} change(s)...",
            marked_ids.len()
        )));

        Some(TuiCommand::StartProcessing(marked_ids))
    }

    /// Retry error changes - resets error changes to queued and returns their IDs
    /// Returns None if not in Error mode or no error changes found
    pub fn retry_error_changes(&mut self) -> Option<TuiCommand> {
        if self.mode != AppMode::Error {
            return None;
        }

        // Collect error change IDs
        let error_ids: Vec<String> = self
            .changes
            .iter()
            .filter(|c| matches!(c.queue_status, QueueStatus::Error(_)))
            .map(|c| c.id.clone())
            .collect();

        if error_ids.is_empty() {
            return None;
        }

        // Reset error changes to queued
        for change in &mut self.changes {
            if matches!(change.queue_status, QueueStatus::Error(_)) {
                change.queue_status = QueueStatus::Queued;
                change.selected = true;
            }
        }

        // Add retry log messages
        for id in &error_ids {
            self.add_log(LogEntry::info(format!("Retrying: {}", id)));
        }

        // Reset error state and transition to Running
        self.reset_for_run();
        self.mode = AppMode::Running;

        Some(TuiCommand::StartProcessing(error_ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::Change;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: false,
            dependencies: Vec::new(),
        }
    }

    fn create_approved_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_start_processing_with_selection() {
        let changes = vec![create_approved_change("a", 0, 1)];

        let mut app = AppState::new(changes);

        let cmd = app.start_processing();
        assert!(cmd.is_some());
        assert_eq!(app.mode, AppMode::Running);
    }

    #[test]
    fn test_start_processing_resets_stop_state() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.stop_mode = StopMode::ForceStopped;
        app.error_change_id = Some("stale".to_string());
        app.orchestration_elapsed = Some(std::time::Duration::from_secs(5));

        let cmd = app.start_processing();

        assert!(cmd.is_some());
        assert_eq!(app.stop_mode, StopMode::None);
        assert!(app.orchestration_started_at.is_some());
        assert!(app.orchestration_elapsed.is_none());
        assert!(app.error_change_id.is_none());
    }

    #[test]
    fn test_resume_processing_resets_stop_state() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Stopped;
        app.stop_mode = StopMode::ForceStopped;
        app.error_change_id = Some("stale".to_string());
        app.orchestration_elapsed = Some(std::time::Duration::from_secs(4));
        // In Stopped mode: queue_status is NotQueued, execution mark is selected=true
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].selected = true;

        let cmd = app.resume_processing();

        assert!(cmd.is_some());
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.stop_mode, StopMode::None);
        assert!(app.orchestration_started_at.is_some());
        assert!(app.orchestration_elapsed.is_none());
        assert!(app.error_change_id.is_none());
        // After resume, change should be Queued
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
    }

    #[test]
    fn test_resume_processing_preserves_parallel_mode() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Stopped;
        app.parallel_mode = true;
        // In Stopped mode: queue_status is NotQueued, execution mark is selected=true
        app.changes[0].queue_status = QueueStatus::NotQueued;
        app.changes[0].selected = true;

        let cmd = app.resume_processing();

        assert!(cmd.is_some());
        assert!(app.parallel_mode);
        // After resume, change should be Queued
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        if let Some(TuiCommand::StartProcessing(ids)) = cmd {
            assert_eq!(ids, vec!["a".to_string()]);
        } else {
            panic!("Expected StartProcessing command");
        }
    }

    #[test]
    fn test_start_processing_without_selection() {
        let changes = vec![create_test_change("a", 0, 1)];

        let mut app = AppState::new(changes);
        app.changes[0].selected = false;

        let cmd = app.start_processing();
        assert!(cmd.is_none());
        assert_eq!(app.mode, AppMode::Select);
        assert!(app.warning_message.is_some());
    }

    #[test]
    fn test_retry_error_changes_from_error_mode() {
        let changes = vec![
            create_approved_change("a", 0, 1),
            create_approved_change("b", 0, 2),
        ];
        let mut app = AppState::new(changes);

        // Start processing
        app.start_processing();

        // Set one change to error
        app.mode = AppMode::Error;
        app.error_change_id = Some("a".to_string());
        app.changes[0].queue_status = QueueStatus::Error("LLM error".to_string());
        app.changes[1].queue_status = QueueStatus::Completed;

        // Retry should reset error changes
        let cmd = app.retry_error_changes();

        assert!(cmd.is_some());
        if let Some(TuiCommand::StartProcessing(ids)) = cmd {
            assert_eq!(ids, vec!["a".to_string()]);
        } else {
            panic!("Expected StartProcessing command");
        }

        // Mode should be Running
        assert_eq!(app.mode, AppMode::Running);
        assert!(app.error_change_id.is_none());
        assert_eq!(app.changes[0].queue_status, QueueStatus::Queued);
        assert!(app.changes[0].selected);
        // Completed change should remain completed
        assert_eq!(app.changes[1].queue_status, QueueStatus::Completed);
    }

    #[test]
    fn test_retry_error_changes_resets_stop_state() {
        let changes = vec![create_approved_change("error-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Error;
        app.stop_mode = StopMode::GracefulPending;
        app.error_change_id = Some("error-change".to_string());
        app.orchestration_elapsed = Some(std::time::Duration::from_secs(4));
        app.changes[0].queue_status = QueueStatus::Error("test error".to_string());

        let cmd = app.retry_error_changes();

        assert!(cmd.is_some());
        assert_eq!(app.stop_mode, StopMode::None);
        assert!(app.orchestration_started_at.is_some());
        assert!(app.orchestration_elapsed.is_none());
        assert!(app.error_change_id.is_none());
    }

    #[test]
    fn test_retry_error_changes_does_nothing_in_select_mode() {
        let changes = vec![create_test_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Set change to error but mode is Select
        app.changes[0].queue_status = QueueStatus::Error("LLM error".to_string());

        // Retry should do nothing
        let cmd = app.retry_error_changes();
        assert!(cmd.is_none());
        assert_eq!(app.mode, AppMode::Select);
    }

    #[test]
    fn test_retry_error_changes_does_nothing_when_no_errors() {
        let changes = vec![create_approved_change("a", 0, 1)];
        let mut app = AppState::new(changes);

        // Start processing and set mode to Error manually
        app.start_processing();
        app.mode = AppMode::Error;
        // But no changes have error status

        // Retry should do nothing
        let cmd = app.retry_error_changes();
        assert!(cmd.is_none());
    }

    #[test]
    fn test_retry_logs_retrying_message() {
        let changes = vec![create_test_change("error-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set up error state
        app.mode = AppMode::Error;
        app.error_change_id = Some("error-change".to_string());
        app.changes[0].queue_status = QueueStatus::Error("test error".to_string());

        // Clear logs
        app.logs.clear();

        // Retry
        let _ = app.retry_error_changes();

        // Check that log contains retry message
        assert!(app.logs.iter().any(|log| log.message.contains("Retrying")));
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("error-change")));
    }
}
