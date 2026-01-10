//! Event and command types for TUI communication
//!
//! Contains types for communication between TUI and orchestrator.

use crate::openspec::Change;
use chrono::Local;
use ratatui::style::Color;

/// Log entry for the TUI
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: String,
    /// Log message
    pub message: String,
    /// Log level color
    pub color: Color,
}

impl LogEntry {
    /// Create a new info log entry
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::White,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Green,
        }
    }

    /// Create a new warning log entry
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Yellow,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message: message.into(),
            color: Color::Red,
        }
    }
}

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically
    AddToQueue(String),
    /// Remove a change from the queue dynamically
    RemoveFromQueue(String),
    /// Approve a change and add it to the queue (used in select/stopped/completed mode)
    ApproveAndQueue(String),
    /// Approve a change without adding to queue (used in running mode)
    ApproveOnly(String),
    /// Unapprove a change and remove it from the queue (used in running/completed mode)
    UnapproveAndDequeue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
}

/// Events sent from orchestrator to TUI
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Processing started for a change
    ProcessingStarted(String),
    /// Progress updated for a change
    #[allow(dead_code)]
    ProgressUpdated {
        id: String,
        completed: u32,
        total: u32,
    },
    /// Processing completed for a change
    ProcessingCompleted(String),
    /// Change archived
    ChangeArchived(String),
    /// Error occurred for a change
    ProcessingError { id: String, error: String },
    /// All processing completed
    AllCompleted,
    /// Processing stopped (graceful stop completed)
    Stopped,
    /// Log message
    Log(LogEntry),
    /// Changes list refreshed
    ChangesRefreshed(Vec<Change>),
}
