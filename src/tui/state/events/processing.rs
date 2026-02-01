//! Processing lifecycle event handlers
//!
//! Handles ProcessingStarted, ProcessingCompleted, ProcessingError, Stopped, AllCompleted

use std::time::Instant;

use crate::tui::events::LogEntry;
use crate::tui::types::{AppMode, QueueStatus, StopMode};

use super::super::AppState;

impl AppState {
    /// Handle ProcessingStarted event
    pub(super) fn handle_processing_started(&mut self, id: String) {
        self.current_change = Some(id.clone());
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Applying;
            change.started_at = Some(Instant::now());
            change.elapsed_time = None;
        }
        self.add_log(LogEntry::info(format!("Processing: {}", id)));
    }

    /// Handle ProcessingCompleted event
    pub(super) fn handle_processing_completed(&mut self, id: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Archiving;
            // Reload final progress from tasks.md to preserve it
            if let Ok(progress) = crate::task_parser::parse_change(&id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
        self.add_log(LogEntry::success(format!("Completed: {}", id)));
    }

    /// Handle ProcessingError event
    pub(super) fn handle_processing_error(&mut self, id: String, error: String) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
            change.queue_status = QueueStatus::Error(error.clone());
            change.selected = true;
            // Record elapsed time on error
            if let Some(started) = change.started_at {
                change.elapsed_time = Some(started.elapsed());
            }
        }
        self.add_log(LogEntry::error(format!("Error in {}: {}", id, error)));
        // ProcessingError does NOT transition to Error mode (change-level failure only)
        // AppMode remains Running to allow processing continuation
        // error_change_id is set to track which change failed, but mode stays Running
        self.error_change_id = Some(id.clone());
        self.current_change = None;
    }

    /// Handle AllCompleted event
    pub(super) fn handle_all_completed(&mut self) {
        if matches!(self.mode, AppMode::Stopped | AppMode::Error) {
            if let Some(started) = self.orchestration_started_at {
                self.orchestration_elapsed = Some(started.elapsed());
            }
            return;
        }

        self.mode = AppMode::Select;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        // Record final orchestration time
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        self.add_log(LogEntry::success("All changes processed successfully"));
    }

    /// Handle Stopped event
    pub(super) fn handle_stopped(&mut self) {
        self.mode = AppMode::Stopped;
        self.current_change = None;
        self.stop_mode = StopMode::None;
        if let Some(started) = self.orchestration_started_at {
            self.orchestration_elapsed = Some(started.elapsed());
        }
        // Reset queue status to NotQueued while preserving execution mark (selected)
        // This implements the policy: queued only during active execution
        for change in &mut self.changes {
            if matches!(
                change.queue_status,
                QueueStatus::Applying
                    | QueueStatus::Accepting
                    | QueueStatus::Archiving
                    | QueueStatus::Queued
            ) {
                // Record elapsed time before resetting status (for in-flight changes)
                if let Some(started) = change.started_at {
                    change.elapsed_time = Some(started.elapsed());
                }
                // Reset to NotQueued but preserve execution mark (selected field)
                change.queue_status = QueueStatus::NotQueued;
                // Keep change.selected as-is to preserve execution mark
            }
        }
        self.add_log(LogEntry::warn("Processing stopped"));
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
            is_approved: true,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_processing_error_keeps_app_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Running mode
        app.mode = AppMode::Running;
        app.current_change = Some("test-change".to_string());

        // Receive ProcessingError
        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        // AppMode should remain Running (NOT transition to Error)
        assert_eq!(app.mode, AppMode::Running);

        // Change should be marked as Error
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert!(matches!(change.queue_status, QueueStatus::Error(_)));

        // error_change_id should be set
        assert_eq!(app.error_change_id, Some("test-change".to_string()));

        // current_change should be cleared
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn test_processing_error_from_select_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Select mode
        app.mode = AppMode::Select;

        // Receive ProcessingError
        app.handle_processing_error("test-change".to_string(), "Test error message".to_string());

        // AppMode should remain Select (NOT transition to Error)
        assert_eq!(app.mode, AppMode::Select);

        // Change should be marked as Error
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert!(matches!(change.queue_status, QueueStatus::Error(_)));
    }

    #[test]
    fn test_processing_started_sets_state() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_started("test-change".to_string());

        assert_eq!(app.current_change, Some("test-change".to_string()));
        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.queue_status, QueueStatus::Applying);
        assert!(change.started_at.is_some());
    }

    #[test]
    fn test_processing_completed_updates_status() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_processing_completed("test-change".to_string());

        let change = app.changes.iter().find(|c| c.id == "test-change").unwrap();
        assert_eq!(change.queue_status, QueueStatus::Archiving);
    }

    #[test]
    fn test_all_completed_transitions_to_select() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Running;
        app.handle_all_completed();

        assert_eq!(app.mode, AppMode::Select);
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn test_all_completed_preserves_error_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Error;
        app.handle_all_completed();

        // Should remain in Error mode
        assert_eq!(app.mode, AppMode::Error);
    }

    #[test]
    fn test_all_completed_preserves_stopped_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.mode = AppMode::Stopped;
        app.handle_all_completed();

        // Should remain in Stopped mode (not transition to Select)
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[test]
    fn test_stopped_resets_queue_status() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Set to Queued
        app.changes[0].queue_status = QueueStatus::Queued;
        app.changes[0].selected = true;

        app.handle_stopped();

        assert_eq!(app.mode, AppMode::Stopped);
        assert_eq!(app.changes[0].queue_status, QueueStatus::NotQueued);
        // selected should be preserved
        assert!(app.changes[0].selected);
    }
}
