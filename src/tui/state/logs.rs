//! Log management for AppState
//!
//! Contains log-related methods and constants for the TUI state.

use super::super::events::{LogEntry, LogLevel};
use super::AppState;
use tracing::{error, info, warn};

/// Maximum number of log entries to keep
pub const MAX_LOG_ENTRIES: usize = 1000;

impl AppState {
    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        // Send to tracing for debug file output (if --logs enabled)
        match entry.level {
            LogLevel::Info | LogLevel::Success => {
                info!(target: "tui_log", "{}", entry.message);
            }
            LogLevel::Warn => {
                warn!(target: "tui_log", "{}", entry.message);
            }
            LogLevel::Error => {
                error!(target: "tui_log", "{}", entry.message);
            }
        }

        self.logs.push(entry);
        if self.logs.len() > MAX_LOG_ENTRIES {
            self.logs.remove(0);
            // Adjust scroll offset if oldest logs are removed
            if self.log_scroll_offset > 0 {
                self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
            }
        }
        // Auto-scroll to bottom if enabled, otherwise maintain relative position
        if self.log_auto_scroll {
            self.log_scroll_offset = 0;
        } else {
            // When auto-scroll is disabled, increment offset to maintain view position
            // This compensates for the new log entry shifting the display
            self.log_scroll_offset += 1;
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
