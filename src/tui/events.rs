//! Event and command types for TUI communication
//!
//! Contains types for communication between TUI and orchestrator.
//! This module now re-exports the unified ExecutionEvent type.

// Re-export unified event types
pub use crate::events::{ExecutionEvent, LogEntry};

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
    /// Approve a change without adding to queue (select/running/stopped modes)
    ApproveOnly(String),
    /// Unapprove a change and remove it from the queue (used in running/completed mode)
    UnapproveAndDequeue(String),
    /// Stop processing (graceful shutdown)
    #[allow(dead_code)]
    Stop,
    /// Delete worktrees associated with a change
    #[allow(dead_code)]
    DeleteWorktree(String),
    /// Delete a worktree by path (from worktree view)
    DeleteWorktreeByPath(PathBuf),
    /// Resolve a deferred merge for a change
    ResolveMerge(String),
    /// Merge a worktree branch into the base branch
    MergeWorktreeBranch {
        worktree_path: PathBuf,
        branch_name: String,
    },
}
