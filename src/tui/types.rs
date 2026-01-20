//! Type definitions for the TUI module
//!
//! Contains enums and basic structs used throughout the TUI.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// View mode for TUI navigation
#[derive(Debug, Clone, PartialEq, Default)]
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
    /// Currently being processed
    Processing,
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
            QueueStatus::Accepting => "accepting",
            QueueStatus::Archiving => "archiving",
            QueueStatus::Archived => "archived",
            QueueStatus::Merged => "merged",
            QueueStatus::MergeWait => "merge wait",
            QueueStatus::Resolving => "resolving",
            QueueStatus::Error(_) => "error",
        }
    }

    /// Get the color for the queue status
    pub fn color(&self) -> Color {
        match self {
            QueueStatus::NotQueued => Color::DarkGray,
            QueueStatus::Queued => Color::Yellow,
            QueueStatus::Processing => Color::Cyan,
            QueueStatus::Accepting => Color::LightGreen,
            QueueStatus::Archiving => Color::Magenta,
            QueueStatus::Archived => Color::Blue,
            QueueStatus::Merged => Color::LightBlue,
            QueueStatus::MergeWait => Color::LightMagenta,
            QueueStatus::Resolving => Color::LightCyan,
            QueueStatus::Error(_) => Color::Red,
        }
    }

    /// Check if the queue status represents an active processing state
    ///
    /// Active states are those where work is currently being performed.
    /// This is used for counting "Running N" in the TUI header.
    ///
    /// Active: Queued, Processing, Accepting, Archiving, Resolving
    /// Inactive: NotQueued, MergeWait, Archived, Merged, Error
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            QueueStatus::Queued
                | QueueStatus::Processing
                | QueueStatus::Accepting
                | QueueStatus::Archiving
                | QueueStatus::Resolving
        )
    }
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

impl WorktreeInfo {
    /// Get display label for the worktree (basename of path)
    pub fn display_label(&self) -> String {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)")
            .to_string()
    }

    /// Get display branch name (or "(detached)" if detached HEAD)
    pub fn display_branch(&self) -> String {
        if self.is_detached {
            format!("(detached: {})", &self.head)
        } else if self.branch.is_empty() {
            "(no branch)".to_string()
        } else {
            self.branch.clone()
        }
    }

    /// Check if worktree has merge conflicts
    pub fn has_merge_conflict(&self) -> bool {
        self.merge_conflict
            .as_ref()
            .map(|c| !c.conflict_files.is_empty())
            .unwrap_or(false)
    }

    /// Get count of conflicting files
    pub fn conflict_file_count(&self) -> usize {
        self.merge_conflict
            .as_ref()
            .map(|c| c.conflict_files.len())
            .unwrap_or(0)
    }

    /// Get merge status label for display
    /// Returns "merging" if merge in progress, "merged" if not ahead of base, empty otherwise
    pub fn merge_status_label(&self) -> &str {
        if self.is_merging {
            "merging"
        } else if !self.has_commits_ahead && !self.is_main && !self.is_detached {
            "merged"
        } else {
            ""
        }
    }
}

/// Merge conflict information for a worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeConflictInfo {
    /// List of files with merge conflicts
    pub conflict_files: Vec<String>,
}

/// Action to perform on a worktree
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum WorktreeAction {
    /// Delete the worktree
    Delete,
    /// Open editor in the worktree directory
    OpenEditor,
    /// Open shell in the worktree directory
    OpenShell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_status_display() {
        assert_eq!(QueueStatus::NotQueued.display(), "not queued");
        assert_eq!(QueueStatus::Queued.display(), "queued");
        assert_eq!(QueueStatus::Processing.display(), "processing");
        assert_eq!(QueueStatus::Accepting.display(), "accepting");
        assert_eq!(QueueStatus::Archiving.display(), "archiving");
        assert_eq!(QueueStatus::Archived.display(), "archived");
        assert_eq!(QueueStatus::Merged.display(), "merged");
        assert_eq!(QueueStatus::MergeWait.display(), "merge wait");
        assert_eq!(QueueStatus::Resolving.display(), "resolving");
        assert_eq!(QueueStatus::Error("err".to_string()).display(), "error");
    }

    #[test]
    fn test_queue_status_color() {
        assert_eq!(QueueStatus::NotQueued.color(), Color::DarkGray);
        assert_eq!(QueueStatus::Queued.color(), Color::Yellow);
        assert_eq!(QueueStatus::Processing.color(), Color::Cyan);
        assert_eq!(QueueStatus::Accepting.color(), Color::LightGreen);
        assert_eq!(QueueStatus::Archiving.color(), Color::Magenta);
        assert_eq!(QueueStatus::Archived.color(), Color::Blue);
        assert_eq!(QueueStatus::Merged.color(), Color::LightBlue);
        assert_eq!(QueueStatus::MergeWait.color(), Color::LightMagenta);
        assert_eq!(QueueStatus::Resolving.color(), Color::LightCyan);
        assert_eq!(QueueStatus::Error("err".to_string()).color(), Color::Red);
    }

    #[test]
    fn test_queue_status_merged_display() {
        assert_eq!(QueueStatus::Merged.display(), "merged");
    }

    #[test]
    fn test_queue_status_merged_color() {
        assert_eq!(QueueStatus::Merged.color(), Color::LightBlue);
    }

    #[test]
    fn test_queue_status_is_active() {
        assert!(QueueStatus::Queued.is_active());
        assert!(QueueStatus::Processing.is_active());
        assert!(QueueStatus::Accepting.is_active());
        assert!(QueueStatus::Archiving.is_active());
        assert!(QueueStatus::Resolving.is_active());

        assert!(!QueueStatus::NotQueued.is_active());
        assert!(!QueueStatus::MergeWait.is_active());
        assert!(!QueueStatus::Archived.is_active());
        assert!(!QueueStatus::Merged.is_active());
        assert!(!QueueStatus::Error("err".to_string()).is_active());
    }

    #[test]
    fn test_worktree_info_display_label() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };
        assert_eq!(wt.display_label(), "worktree");
    }

    #[test]
    fn test_worktree_info_display_branch_normal() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature-branch".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        };
        assert_eq!(wt.display_branch(), "feature-branch");
    }

    #[test]
    fn test_worktree_info_display_branch_detached() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "".to_string(),
            is_detached: true,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };
        assert_eq!(wt.display_branch(), "(detached: abc123)");
    }

    #[test]
    fn test_worktree_info_has_merge_conflict() {
        let wt_no_conflict = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "main".to_string(),
            is_detached: false,
            is_main: true,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        };
        assert!(!wt_no_conflict.has_merge_conflict());

        let wt_with_conflict = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(MergeConflictInfo {
                conflict_files: vec!["file.rs".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
        };
        assert!(wt_with_conflict.has_merge_conflict());
    }

    #[test]
    fn test_worktree_info_conflict_file_count() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/path/to/worktree"),
            head: "abc123".to_string(),
            branch: "feature".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: Some(MergeConflictInfo {
                conflict_files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            }),
            has_commits_ahead: true,
            is_merging: false,
        };
        assert_eq!(wt.conflict_file_count(), 2);
    }
}
