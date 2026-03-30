//! Type definitions for the TUI module
//!
//! Contains enums and basic structs used throughout the TUI.
//! Method implementations are in `type_impls.rs`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

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

/// Information about a git worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct WorktreeInfo {
    /// Path to the worktree
    #[cfg_attr(feature = "web-monitoring", schema(value_type = String))]
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
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
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
