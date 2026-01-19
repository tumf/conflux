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

    /// Handle Error event
    pub(super) fn handle_error(&mut self, message: String) {
        self.add_log(LogEntry::error(message.clone()));
        self.mode = AppMode::Error;
        self.error_change_id = None;
        self.current_change = None;
    }
}
