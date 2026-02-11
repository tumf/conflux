//! Event and command types for TUI communication
//!
//! Contains types for communication between TUI and orchestrator.
//! This module now re-exports the unified ExecutionEvent type.

// Re-export unified event types
pub use crate::events::{ExecutionEvent, LogEntry, LogLevel};

// Alias for backward compatibility
pub type OrchestratorEvent = ExecutionEvent;

use std::path::PathBuf;

/// Commands sent from TUI to orchestrator
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Start processing selected changes
    StartProcessing(Vec<String>),
    /// Add a change to the queue dynamically
    AddToQueue(String),
    /// Remove a change from the queue dynamically
    RemoveFromQueue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
    /// Cancel a pending stop request
    CancelStop,
    /// Force stop immediately
    ForceStop,
    /// Retry error changes
    Retry,
    /// Delete a worktree by path (from worktree view)
    /// The optional String is the branch name to delete after worktree removal
    DeleteWorktreeByPath(PathBuf, Option<String>),
    /// Resolve a deferred merge for a change
    ResolveMerge(String),
    /// Merge a worktree branch into the base branch
    MergeWorktreeBranch {
        worktree_path: PathBuf,
        branch_name: String,
    },
    /// Stop a single active change (during Running mode)
    StopChange(String),
}
