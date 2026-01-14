//! Type definitions for the TUI module
//!
//! Contains enums and basic structs used throughout the TUI.

use ratatui::style::Color;

/// Stop mode for graceful/force stop handling
#[derive(Debug, Clone, PartialEq, Default)]
pub enum StopMode {
    /// Not stopping, normal operation
    #[default]
    None,
    /// Graceful stop requested, waiting for current process
    GracefulPending,
    /// Force stop executed
    ForceStopped,
}

/// Application mode
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Selection mode - user selects changes to process
    Select,
    /// Running mode - processing selected changes
    Running,
    /// Stopping mode - graceful stop in progress
    Stopping,
    /// Stopped mode - processing halted, can modify queue
    Stopped,
    /// Error mode - an error occurred during processing
    Error,
    /// Proposing mode - user is entering a new proposal
    Proposing,
    /// Confirmation dialog for worktree deletion
    ConfirmWorktreeDelete,
    /// QR popup mode - showing Web UI QR code
    QrPopup,
}

/// Queue status for a change
#[derive(Debug, Clone, PartialEq)]
pub enum QueueStatus {
    /// Not in the execution queue
    NotQueued,
    /// Waiting in the execution queue
    Queued,
    /// Currently being processed
    Processing,
    /// Processing completed, waiting for archive
    Completed,
    /// Currently being archived
    Archiving,
    /// Archived after completion
    Archived,
    /// Error occurred during processing
    Error(String),
}

impl QueueStatus {
    /// Get display string for the queue status
    pub fn display(&self) -> &str {
        match self {
            QueueStatus::NotQueued => "not queued",
            QueueStatus::Queued => "queued",
            QueueStatus::Processing => "processing",
            QueueStatus::Completed => "completed",
            QueueStatus::Archiving => "archiving",
            QueueStatus::Archived => "archived",
            QueueStatus::Error(_) => "error",
        }
    }

    /// Get the color for the queue status
    pub fn color(&self) -> Color {
        match self {
            QueueStatus::NotQueued => Color::DarkGray,
            QueueStatus::Queued => Color::Yellow,
            QueueStatus::Processing => Color::Cyan,
            QueueStatus::Completed => Color::Green,
            QueueStatus::Archiving => Color::Magenta,
            QueueStatus::Archived => Color::Blue,
            QueueStatus::Error(_) => Color::Red,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_status_display() {
        assert_eq!(QueueStatus::NotQueued.display(), "not queued");
        assert_eq!(QueueStatus::Queued.display(), "queued");
        assert_eq!(QueueStatus::Processing.display(), "processing");
        assert_eq!(QueueStatus::Completed.display(), "completed");
        assert_eq!(QueueStatus::Archiving.display(), "archiving");
        assert_eq!(QueueStatus::Archived.display(), "archived");
        assert_eq!(QueueStatus::Error("err".to_string()).display(), "error");
    }

    #[test]
    fn test_queue_status_color() {
        assert_eq!(QueueStatus::NotQueued.color(), Color::DarkGray);
        assert_eq!(QueueStatus::Queued.color(), Color::Yellow);
        assert_eq!(QueueStatus::Processing.color(), Color::Cyan);
        assert_eq!(QueueStatus::Completed.color(), Color::Green);
        assert_eq!(QueueStatus::Archiving.color(), Color::Magenta);
        assert_eq!(QueueStatus::Archived.color(), Color::Blue);
        assert_eq!(QueueStatus::Error("err".to_string()).color(), Color::Red);
    }
}
