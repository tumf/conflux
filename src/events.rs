//! Unified event system for OpenSpec Orchestrator
//!
//! This module provides a single event type that unifies the previously separate
//! ParallelEvent (from parallel execution) and OrchestratorEvent (from TUI) types.
//!
//! The ExecutionEvent enum represents all possible events that can occur during
//! change processing, whether in serial or parallel mode.

use std::sync::OnceLock;

use chrono::Local;
use ratatui::style::Color;
use regex::Regex;
use tokio::sync::mpsc;
use tracing::debug;

/// Log entry for the TUI
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp
    pub timestamp: String,
    /// Log message
    pub message: String,
    /// Log level color
    pub color: Color,
    /// Optional change_id for parallel mode logs
    pub change_id: Option<String>,
}

fn ansi_csi_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").expect("Invalid ANSI CSI regex"))
}

fn ansi_fragment_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\[[0-9;]{1,}m").expect("Invalid ANSI fragment regex"))
}

fn sanitize_log_message(message: &str) -> String {
    let without_ansi = ansi_csi_regex().replace_all(message, "");
    let without_fragments = ansi_fragment_regex().replace_all(&without_ansi, "");
    without_fragments
        .chars()
        .filter(|ch| !ch.is_control())
        .collect()
}

impl LogEntry {
    /// Create a new info log entry
    pub fn info(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message,
            color: Color::White,
            change_id: None,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message,
            color: Color::Green,
            change_id: None,
        }
    }

    /// Create a new warning log entry
    pub fn warn(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message,
            color: Color::Yellow,
            change_id: None,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        Self {
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            message,
            color: Color::Red,
            change_id: None,
        }
    }

    /// Set change_id for parallel mode logs
    #[allow(dead_code)]
    pub fn with_change_id(mut self, change_id: impl Into<String>) -> Self {
        self.change_id = Some(change_id.into());
        self
    }
}

/// Unified event type for all execution events
///
/// This enum combines events from both serial and parallel execution modes,
/// providing a single interface for event handling across the application.
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    // Lifecycle events
    /// Processing started for a change
    ProcessingStarted(String),
    /// Processing completed for a change
    ProcessingCompleted(String),
    /// Error occurred for a change
    ProcessingError { id: String, error: String },

    // Apply events
    /// Apply started in a workspace
    #[allow(dead_code)]
    ApplyStarted { change_id: String },
    /// Apply completed in a workspace
    ApplyCompleted {
        change_id: String,
        #[allow(dead_code)]
        revision: String,
    },
    /// Apply failed in a workspace
    ApplyFailed { change_id: String, error: String },
    /// Apply output (summary of command output)
    #[allow(dead_code)]
    ApplyOutput { change_id: String, output: String },

    // Archive events
    /// Archive started for a change
    ArchiveStarted(String),
    /// Change archived successfully
    ChangeArchived(String),
    /// Change archive failed
    #[allow(dead_code)]
    ArchiveFailed { change_id: String, error: String },
    /// Archive output (streaming)
    #[allow(dead_code)]
    ArchiveOutput { change_id: String, output: String },

    // Progress events
    /// Progress updated for a change (task completion tracking)
    ProgressUpdated {
        change_id: String,
        completed: u32,
        total: u32,
    },

    // Workspace events (parallel mode)
    /// A workspace was created
    #[allow(dead_code)]
    WorkspaceCreated {
        change_id: String,
        workspace: String,
    },
    /// An existing workspace was found and is being reused
    #[allow(dead_code)]
    WorkspaceResumed {
        change_id: String,
        workspace: String,
    },
    /// A workspace was preserved due to an error (not cleaned up)
    #[allow(dead_code)]
    WorkspacePreserved {
        change_id: String,
        workspace_name: String,
    },
    /// Workspace cleanup started
    #[allow(dead_code)]
    CleanupStarted { workspace: String },
    /// Workspace cleanup completed
    CleanupCompleted {
        #[allow(dead_code)]
        workspace: String,
    },

    // Merge events (parallel mode)
    /// Merge started
    #[allow(dead_code)]
    MergeStarted { revisions: Vec<String> },
    /// Merge completed
    MergeCompleted {
        #[allow(dead_code)]
        revision: String,
    },
    /// Merge resulted in conflicts
    #[allow(dead_code)]
    MergeConflict { files: Vec<String> },
    /// Conflict resolution started
    ConflictResolutionStarted,
    /// Conflict resolution completed
    ConflictResolutionCompleted,
    /// Conflict resolution failed
    #[allow(dead_code)]
    ConflictResolutionFailed { error: String },

    // Group execution events (parallel mode)
    /// Group execution started
    GroupStarted { group_id: u32, changes: Vec<String> },
    /// Group execution completed
    GroupCompleted { group_id: u32 },
    /// A change was skipped because a dependency failed
    #[allow(dead_code)]
    ChangeSkipped { change_id: String, reason: String },

    // Analysis events (parallel mode)
    /// Analysis started for remaining changes
    #[allow(dead_code)]
    AnalysisStarted { remaining_changes: usize },
    /// Analysis output (streaming)
    #[allow(dead_code)]
    AnalysisOutput { output: String },
    /// Analysis completed
    #[allow(dead_code)]
    AnalysisCompleted { groups_found: usize },
    /// Resolve output (streaming)
    #[allow(dead_code)]
    ResolveOutput { output: String },

    // Hook events
    /// Hook execution started
    #[allow(dead_code)]
    HookStarted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution completed successfully
    #[allow(dead_code)]
    HookCompleted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution failed
    #[allow(dead_code)]
    HookFailed {
        change_id: String,
        hook_type: String,
        error: String,
    },

    // General events
    /// Warning message (non-fatal)
    Warning { title: String, message: String },
    /// Log message
    Log(LogEntry),
    /// Processing stopped (graceful stop completed)
    Stopped,
    /// All processing completed
    AllCompleted,
    /// Error during execution
    Error { message: String },
    /// Changes list refreshed
    ChangesRefreshed {
        changes: Vec<crate::openspec::Change>,
        committed_change_ids: std::collections::HashSet<String>,
    },
}

/// Helper to send events through the channel.
///
/// Logs debug message if sending fails (channel closed).
pub async fn send_event(tx: &Option<mpsc::Sender<ExecutionEvent>>, event: ExecutionEvent) {
    if let Some(ref tx) = tx {
        if let Err(e) = tx.send(event).await {
            debug!("Failed to send execution event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_event_debug() {
        let event = ExecutionEvent::WorkspaceCreated {
            change_id: "test".to_string(),
            workspace: "ws-test".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("WorkspaceCreated"));
    }

    #[test]
    fn test_log_entry_info() {
        let entry = LogEntry::info("test message");
        assert_eq!(entry.message, "test message");
        assert!(matches!(entry.color, Color::White));
        assert!(entry.change_id.is_none());
    }

    #[test]
    fn test_log_entry_strips_ansi_sequences() {
        let entry = LogEntry::info("\x1b[96mRead\x1b[0m");
        assert_eq!(entry.message, "Read");
    }

    #[test]
    fn test_log_entry_strips_sgr_fragments() {
        let entry = LogEntry::info("[96m[1m| [0m[90m Read");
        assert_eq!(entry.message, "|  Read");
    }

    #[test]
    fn test_log_entry_with_change_id() {
        let entry = LogEntry::info("test").with_change_id("test-change");
        assert_eq!(entry.change_id, Some("test-change".to_string()));
    }

    #[test]
    fn test_hook_started_event() {
        let event = ExecutionEvent::HookStarted {
            change_id: "test-change".to_string(),
            hook_type: "pre_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("pre_apply"));
    }

    #[test]
    fn test_hook_completed_event() {
        let event = ExecutionEvent::HookCompleted {
            change_id: "test-change".to_string(),
            hook_type: "post_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookCompleted"));
        assert!(debug_str.contains("post_apply"));
    }

    #[test]
    fn test_hook_failed_event() {
        let event = ExecutionEvent::HookFailed {
            change_id: "test-change".to_string(),
            hook_type: "pre_archive".to_string(),
            error: "Hook timed out".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookFailed"));
        assert!(debug_str.contains("pre_archive"));
        assert!(debug_str.contains("Hook timed out"));
    }

    #[test]
    fn test_progress_updated_event() {
        let event = ExecutionEvent::ProgressUpdated {
            change_id: "test-change".to_string(),
            completed: 5,
            total: 10,
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ProgressUpdated"));
        assert!(debug_str.contains("test-change"));
    }
}
