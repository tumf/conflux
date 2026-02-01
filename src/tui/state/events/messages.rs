//! Message event handlers
//!
//! Handles Log, Warning, Error events

use crate::tui::events::LogEntry;
use crate::tui::types::AppMode;

use super::super::{AppState, WarningPopup};

impl AppState {
    /// Handle Log event
    pub(super) fn handle_log(&mut self, entry: LogEntry) {
        self.add_log(entry);
    }

    /// Handle Warning event
    pub(super) fn handle_warning(&mut self, title: String, message: String) {
        // For uncommitted changes warnings in TUI, only log without popup
        if title != "Uncommitted Changes Detected" {
            self.warning_popup = Some(WarningPopup {
                title: title.clone(),
                message: message.clone(),
            });
        }
        self.add_log(LogEntry::warn(message));
    }

    /// Handle Error event (fatal orchestrator errors)
    ///
    /// Fatal errors transition the entire TUI to Error mode.
    /// This is distinct from ProcessingError which only marks a change as failed.
    pub(super) fn handle_error(&mut self, message: String) {
        self.add_log(LogEntry::error(message.clone()));
        self.mode = AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::Change;
    use crate::tui::state::AppState;

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
    fn test_fatal_error_transitions_to_error_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Running mode
        app.mode = AppMode::Running;
        app.current_change = Some("test-change".to_string());

        // Receive fatal Error event
        app.handle_error("Fatal orchestrator error".to_string());

        // AppMode should transition to Error
        assert_eq!(app.mode, AppMode::Error);

        // error_change_id should be None (not tied to a specific change)
        assert_eq!(app.error_change_id, None);

        // current_change should be cleared
        assert_eq!(app.current_change, None);
    }

    #[test]
    fn test_fatal_error_from_select_mode() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        // Start in Select mode
        app.mode = AppMode::Select;

        // Receive fatal Error event
        app.handle_error("Fatal orchestrator error".to_string());

        // AppMode should transition to Error
        assert_eq!(app.mode, AppMode::Error);
    }

    #[test]
    fn test_handle_log_adds_entry() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        let entry = LogEntry::info("Test log message".to_string());
        let msg = entry.message.clone();

        app.handle_log(entry);

        // Log should be added
        assert!(app.logs.iter().any(|log| log.message == msg));
    }

    #[test]
    fn test_handle_warning_creates_popup() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_warning("Test Warning".to_string(), "Warning message".to_string());

        // Warning popup should be created (except for uncommitted changes)
        assert!(app.warning_popup.is_some());
        let popup = app.warning_popup.as_ref().unwrap();
        assert_eq!(popup.title, "Test Warning");
        assert_eq!(popup.message, "Warning message");
    }

    #[test]
    fn test_handle_warning_uncommitted_changes_no_popup() {
        let changes = vec![create_test_change("test-change", 0, 1)];
        let mut app = AppState::new(changes);

        app.handle_warning(
            "Uncommitted Changes Detected".to_string(),
            "Uncommitted changes warning".to_string(),
        );

        // No popup should be created for uncommitted changes warnings
        assert!(app.warning_popup.is_none());

        // But log should still be added
        assert!(app
            .logs
            .iter()
            .any(|log| log.message.contains("Uncommitted changes warning")));
    }
}
