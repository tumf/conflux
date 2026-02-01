//! Type definitions for the TUI module
//!
//! Contains enums and basic structs used throughout the TUI.
//! Method implementations are in `type_impls.rs`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// View mode for TUI navigation
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ViewMode {
    /// Changes view - display and manage changes
    #[default]
    Changes,
    /// Worktrees view - display and manage git worktrees
    Worktrees,
}

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
    /// Blocked by unresolved dependencies
    Blocked,
    /// Currently being applied
    Applying,
    /// Running acceptance tests
    Accepting,
    /// Currently being archived
    Archiving,
    /// Archived after completion
    Archived,
    /// Merged to main branch (parallel mode only)
    Merged,
    /// Waiting for merge resolution
    MergeWait,
    /// Currently resolving a merge
    Resolving,
    /// Waiting for resolve execution after archive completion
    ResolveWait,
    /// Error occurred during processing
    Error(String),
}

/// Information about a git worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Path to the worktree
    pub path: PathBuf,
    /// Current HEAD commit (short hash or symbolic ref)
    pub head: String,
    /// Branch name (empty if detached)
    pub branch: String,
    /// Whether HEAD is detached
    pub is_detached: bool,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Merge conflict information (None if not checked or no conflicts)
    pub merge_conflict: Option<MergeConflictInfo>,
    /// Whether this worktree has commits ahead of the base branch
    pub has_commits_ahead: bool,
    /// Whether a merge operation is in progress for this worktree
    pub is_merging: bool,
}

/// Merge conflict information for a worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeConflictInfo {
    /// List of files with merge conflicts
    pub conflict_files: Vec<String>,
}

/// Action to perform on a worktree
#[derive(Debug, Clone, PartialEq)]
pub enum WorktreeAction {
    /// Delete the worktree
    Delete,
}
