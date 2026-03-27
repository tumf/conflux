//! Unified event system for OpenSpec Orchestrator
//!
//! This module provides a single event type that unifies the previously separate
//! ParallelEvent (from parallel execution) and OrchestratorEvent (from TUI) types.
//!
//! The ExecutionEvent enum represents all possible events that can occur during
//! change processing, whether in serial or parallel mode.

use std::sync::OnceLock;

use chrono::{Local, Utc};
use ratatui::style::Color;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::debug;

#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

/// Log level for TUI logs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

/// Log entry for the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct LogEntry {
    /// Timestamp (formatted for display)
    pub timestamp: String,
    /// Creation time (actual timestamp for relative time calculation)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Log message
    pub message: String,
    /// Log level color (serialized as RGB string for web)
    #[serde(skip)]
    #[cfg_attr(feature = "web-monitoring", schema(ignore = true))]
    pub color: Color,
    /// Log level
    pub level: LogLevel,
    /// Optional change_id for parallel mode logs
    pub change_id: Option<String>,
    /// Optional operation type (apply, archive, resolve)
    pub operation: Option<String>,
    /// Optional iteration number (for apply operations)
    pub iteration: Option<u32>,
    /// Optional workspace path (for parallel mode logs with workspace context)
    pub workspace_path: Option<String>,
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
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::White,
            level: LogLevel::Info,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new success log entry
    pub fn success(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Green,
            level: LogLevel::Success,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new warning log entry
    pub fn warn(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Yellow,
            level: LogLevel::Warn,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Create a new error log entry
    pub fn error(message: impl Into<String>) -> Self {
        let message = message.into();
        let message = sanitize_log_message(&message);
        let now_local = Local::now();
        let now_utc = Utc::now();
        Self {
            timestamp: now_local.format("%H:%M:%S").to_string(),
            created_at: now_utc,
            message,
            color: Color::Red,
            level: LogLevel::Error,
            change_id: None,
            operation: None,
            iteration: None,
            workspace_path: None,
        }
    }

    /// Set change_id for parallel mode logs
    #[allow(dead_code)]
    pub fn with_change_id(mut self, change_id: impl Into<String>) -> Self {
        self.change_id = Some(change_id.into());
        self
    }

    /// Set operation type (apply, archive, resolve)
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Set iteration number (for apply operations)
    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    /// Set workspace path (for parallel mode logs with workspace context)
    #[allow(dead_code)]
    pub fn with_workspace_path(mut self, workspace_path: impl Into<String>) -> Self {
        self.workspace_path = Some(workspace_path.into());
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
    ApplyStarted { change_id: String, command: String },
    /// Apply completed in a workspace
    ApplyCompleted {
        change_id: String,
        #[allow(dead_code)]
        revision: String,
    },
    /// Apply failed in a workspace
    #[allow(dead_code)]
    ApplyFailed { change_id: String, error: String },
    /// Apply output (summary of command output)
    #[allow(dead_code)]
    ApplyOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

    // Archive events
    /// Archive started for a change
    ArchiveStarted { change_id: String, command: String },
    /// Change archived successfully
    ChangeArchived(String),
    /// Change archive failed
    #[allow(dead_code)]
    ArchiveFailed { change_id: String, error: String },
    /// Archive output (streaming)
    #[allow(dead_code)]
    ArchiveOutput {
        change_id: String,
        output: String,
        iteration: u32,
    },

    // Acceptance events
    /// Acceptance started for a change
    AcceptanceStarted { change_id: String, command: String },
    /// Acceptance completed for a change
    AcceptanceCompleted { change_id: String },
    /// Acceptance failed for a change
    #[allow(dead_code)]
    AcceptanceFailed { change_id: String, error: String },
    /// Acceptance output (streaming)
    #[allow(dead_code)]
    AcceptanceOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

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
    /// Workspace status updated
    WorkspaceStatusUpdated {
        #[allow(dead_code)]
        workspace_name: String,
        #[allow(dead_code)]
        status: crate::vcs::WorkspaceStatus,
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
        change_id: String,
        #[allow(dead_code)]
        revision: String,
    },
    /// Merge deferred due to dirty base or incomplete archive.
    /// `auto_resumable` is `true` when the deferral is caused by a temporary condition
    /// (base dirty, merge in progress) that will resolve automatically once a preceding
    /// merge or resolve completes.  `false` means manual intervention is required.
    #[allow(dead_code)]
    MergeDeferred {
        change_id: String,
        reason: String,
        auto_resumable: bool,
    },
    /// Merge resolution started for a change
    ResolveStarted { change_id: String, command: String },
    /// Merge resolution completed for a change
    ResolveCompleted {
        change_id: String,
        worktree_change_ids: Option<std::collections::HashSet<String>>,
    },
    /// Merge resolution failed for a change
    ResolveFailed { change_id: String, error: String },
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

    /// A change was skipped because a dependency failed
    #[allow(dead_code)]
    ChangeSkipped { change_id: String, reason: String },

    /// A change is blocked waiting for dependencies to be resolved
    DependencyBlocked {
        change_id: String,
        #[allow(dead_code)]
        dependency_ids: Vec<String>,
    },

    /// A change's dependencies were resolved and it can now be queued
    DependencyResolved { change_id: String },

    // Analysis events (parallel mode)
    /// Analysis started for remaining changes
    #[allow(dead_code)]
    AnalysisStarted { remaining_changes: usize },
    /// Analysis output (streaming)
    #[allow(dead_code)]
    AnalysisOutput { output: String, iteration: u32 },
    /// Analysis completed
    #[allow(dead_code)]
    AnalysisCompleted { groups_found: usize },
    /// Resolve output (streaming)
    #[allow(dead_code)]
    ResolveOutput {
        change_id: String,
        output: String,
        iteration: Option<u32>,
    },

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
    /// Changes rejected at parallel start-time eligibility filter
    ///
    /// Sent when backend filtering excludes one or more changes before parallel execution
    /// starts. Callers should use this to restore a consistent non-running state for the
    /// rejected changes (e.g. reset Queued rows in TUI, report zero-start in CLI).
    ParallelStartRejected {
        change_ids: Vec<String>,
        reason: String,
    },
    /// Log message
    Log(LogEntry),
    /// Processing stopping (graceful stop initiated)
    Stopping,
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
        /// Set of change_ids with uncommitted or untracked files under openspec/changes/<change_id>/
        uncommitted_file_change_ids: std::collections::HashSet<String>,
        worktree_change_ids: std::collections::HashSet<String>,
        /// Map of change_id to worktree path for active worktrees
        worktree_paths: std::collections::HashMap<String, std::path::PathBuf>,
        /// Set of change_ids whose worktrees are NOT ahead of base (for auto-clearing MergeWait)
        worktree_not_ahead_ids: std::collections::HashSet<String>,
        /// Set of change_ids in WorkspaceState::Archived (for MergeWait restoration)
        merge_wait_ids: std::collections::HashSet<String>,
    },
    /// Worktrees list refreshed (for worktree view)
    WorktreesRefreshed {
        worktrees: Vec<crate::tui::types::WorktreeInfo>,
    },
    /// Branch merge started (TUI worktree view)
    BranchMergeStarted { branch_name: String },
    /// Branch merge completed successfully (TUI worktree view)
    BranchMergeCompleted { branch_name: String },
    /// Branch merge failed (TUI worktree view)
    BranchMergeFailed { branch_name: String, error: String },
    /// Change stopped successfully (single-change stop)
    ChangeStopped { change_id: String },
    /// Change stop failed (single-change stop)
    #[allow(dead_code)]
    ChangeStopFailed { change_id: String, error: String },
    /// Incremental update from a remote server WebSocket (applies non-regression rule)
    RemoteChangeUpdate {
        /// Change ID as displayed in TUI (may be "project/change-id" for remote mode)
        id: String,
        /// Updated number of completed tasks
        completed_tasks: u32,
        /// Updated total number of tasks
        total_tasks: u32,
        /// Updated remote status (optional)
        status: Option<String>,
        /// Iteration number (applies monotonic non-regression rule)
        iteration_number: Option<u32>,
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

    #[test]
    fn test_log_entry_with_operation() {
        let entry = LogEntry::info("test").with_operation("apply");
        assert_eq!(entry.operation, Some("apply".to_string()));
    }

    #[test]
    fn test_log_entry_with_iteration() {
        let entry = LogEntry::info("test").with_iteration(2);
        assert_eq!(entry.iteration, Some(2));
    }

    #[test]
    fn test_log_entry_with_operation_and_iteration() {
        let entry = LogEntry::info("test")
            .with_change_id("test-change")
            .with_operation("apply")
            .with_iteration(3);
        assert_eq!(entry.change_id, Some("test-change".to_string()));
        assert_eq!(entry.operation, Some("apply".to_string()));
        assert_eq!(entry.iteration, Some(3));
    }

    #[test]
    fn test_log_entry_info_level() {
        let entry = LogEntry::info("test");
        assert_eq!(entry.level, LogLevel::Info);
        assert!(matches!(entry.color, Color::White));
    }

    #[test]
    fn test_log_entry_success_level() {
        let entry = LogEntry::success("test");
        assert_eq!(entry.level, LogLevel::Success);
        assert!(matches!(entry.color, Color::Green));
    }

    #[test]
    fn test_log_entry_warn_level() {
        let entry = LogEntry::warn("test");
        assert_eq!(entry.level, LogLevel::Warn);
        assert!(matches!(entry.color, Color::Yellow));
    }

    #[test]
    fn test_log_entry_error_level() {
        let entry = LogEntry::error("test");
        assert_eq!(entry.level, LogLevel::Error);
        assert!(matches!(entry.color, Color::Red));
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Error);
    }

    #[test]
    fn test_acceptance_started_event_with_command() {
        let event = ExecutionEvent::AcceptanceStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions acceptance test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("AcceptanceStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("acceptance"));
    }

    #[test]
    fn test_archive_started_event_with_command() {
        let event = ExecutionEvent::ArchiveStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions archive test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ArchiveStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("archive"));
    }

    #[test]
    fn test_resolve_started_event_with_command() {
        let event = ExecutionEvent::ResolveStarted {
            change_id: "test-change".to_string(),
            command: "claude --dangerously-skip-permissions resolve test-change".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ResolveStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("resolve"));
    }
}
