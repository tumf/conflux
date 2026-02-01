//! Log management for AppState
//!
//! Contains log-related methods and constants for the TUI state.

use super::super::events::{LogEntry, LogLevel};
use super::AppState;
use tracing::{error, info, warn};

/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 1000;

impl AppState {
    /// Get the latest log entry for a specific change_id
    ///
    /// Returns the most recent log entry that matches the given change_id.
    /// Used for displaying log previews in the change list.
    pub fn get_latest_log_for_change(&self, change_id: &str) -> Option<&LogEntry> {
        self.logs
            .iter()
            .rev()
            .find(|entry| entry.change_id.as_deref() == Some(change_id))
    }

    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        // Send to tracing for debug file output (if --logs enabled)
        // Include change_id, operation, iteration, and workspace_path in tracing output for context matching
        let change_id = entry.change_id.as_deref().unwrap_or("-");
        let operation = entry.operation.as_deref().unwrap_or("-");
        let iteration = entry.iteration.unwrap_or(0);
        let workspace_path = entry.workspace_path.as_deref().unwrap_or("-");

        match entry.level {
            LogLevel::Info | LogLevel::Success => {
                info!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
            LogLevel::Warn => {
                warn!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
            LogLevel::Error => {
                error!(
                    target: "tui_log",
                    change_id = change_id,
                    operation = operation,
                    iteration = iteration,
                    workspace_path = workspace_path,
                    "{}",
                    entry.message
                );
            }
        }

        self.logs.push(entry);

        // Handle buffer trimming when exceeding max entries
        if self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.remove(0);
        }

        // Auto-scroll to bottom if enabled, otherwise freeze view position
        if self.log_auto_scroll {
            self.log_scroll_offset = 0;
        } else {
            // When auto-scroll is disabled, freeze the displayed log range
            // by incrementing offset for new log additions
            self.log_scroll_offset += 1;

            // When buffer is trimmed, we don't decrement offset because we want
            // to keep showing the same log content (freeze position)
            // However, if trimming pushed us out of range, clamp to oldest available
            let max_offset = self.logs.len().saturating_sub(1);
            if self.log_scroll_offset > max_offset {
                self.log_scroll_offset = max_offset;
            }
        }
    }

    /// Scroll logs up by a page (show older entries)
    pub fn scroll_logs_up(&mut self, page_size: usize) {
        let max_offset = self.logs.len().saturating_sub(1);
        self.log_scroll_offset = (self.log_scroll_offset + page_size).min(max_offset);
        // Disable auto-scroll when user scrolls up
        self.log_auto_scroll = false;
    }

    /// Scroll logs down by a page (show newer entries)
    pub fn scroll_logs_down(&mut self, page_size: usize) {
        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(page_size);
        // Re-enable auto-scroll when at bottom
        if self.log_scroll_offset == 0 {
            self.log_auto_scroll = true;
        }
    }

    /// Jump to the oldest log entry (top of history)
    pub fn scroll_logs_to_top(&mut self) {
        let max_offset = self.logs.len().saturating_sub(1);
        self.log_scroll_offset = max_offset;
        self.log_auto_scroll = false;
    }

    /// Jump to the newest log entry (bottom) and re-enable auto-scroll
    pub fn scroll_logs_to_bottom(&mut self) {
        self.log_scroll_offset = 0;
        self.log_auto_scroll = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::state::AppState;
    use ratatui::style::Color;

    #[test]
    fn test_log_wrap_preserves_full_message() {
        let mut app = AppState::new(vec![]);

        // Create a very long log message (longer than typical terminal width)
        let long_message = "This is a very long error message that would normally be truncated \
                           in the TUI display, but with the new wrapping feature it should be \
                           preserved in its entirety and wrapped across multiple lines when \
                           rendered. This allows users to see all the diagnostic information \
                           without losing important details at the end of error messages.";

        let now_local = chrono::Local::now();
        let now_utc = chrono::Utc::now();
        let entry = LogEntry {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            level: LogLevel::Error,
            message: long_message.to_string(),
            change_id: Some("test-change".to_string()),
            operation: Some("apply".to_string()),
            iteration: Some(1),
            workspace_path: None,
            color: Color::Red,
        };

        app.add_log(entry);

        // Verify the full message is preserved in the log buffer
        assert_eq!(app.logs.len(), 1);
        assert_eq!(app.logs[0].message, long_message);
        assert_eq!(app.logs[0].message.len(), long_message.len());

        // Ensure no truncation occurred
        assert!(app.logs[0].message.contains("important details at the end"));
    }

    #[test]
    fn test_tui_log_wrap_scroll() {
        let mut app = AppState::new(vec![]);

        // Add multiple log entries, including some long ones that would wrap
        for i in 0..10 {
            let message = if i % 2 == 0 {
                format!("Short log entry {}", i)
            } else {
                format!(
                    "Long log entry {} with lots of extra text that would normally wrap across \
                     multiple lines in the terminal display but should not affect the scroll \
                     position calculation since wrapping is handled by the Paragraph widget",
                    i
                )
            };

            let now_local = chrono::Local::now();
            let now_utc = chrono::Utc::now();
            let entry = LogEntry {
                timestamp: now_local.format("%H:%M:%S").to_string(),
                created_at: now_utc,
                level: LogLevel::Info,
                message,
                change_id: None,
                operation: None,
                iteration: None,
                workspace_path: None,
                color: ratatui::style::Color::White,
            };

            app.add_log(entry);
        }

        // Verify we have 10 logs
        assert_eq!(app.logs.len(), 10);

        // Initial state: auto-scroll enabled, offset 0
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);

        // Scroll up by 5 (Page Up behavior)
        app.scroll_logs_up(5);
        assert_eq!(app.log_scroll_offset, 5);
        assert!(!app.log_auto_scroll); // Auto-scroll disabled when scrolling up

        // Scroll down by 2
        app.scroll_logs_down(2);
        assert_eq!(app.log_scroll_offset, 3);
        assert!(!app.log_auto_scroll); // Still disabled

        // Scroll to bottom
        app.scroll_logs_to_bottom();
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll); // Re-enabled at bottom

        // Scroll to top
        app.scroll_logs_to_top();
        assert_eq!(app.log_scroll_offset, 9); // At oldest entry
        assert!(!app.log_auto_scroll);
    }
}
