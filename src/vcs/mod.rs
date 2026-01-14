//! VCS backend abstraction for parallel execution.
//!
//! This module provides a trait-based abstraction for VCS operations,
//! allowing parallel execution to work with Git worktrees.
//!
//! ## Module Structure
//!
//! - `mod.rs` - Public API, traits, and VcsError
//! - `commands.rs` - Common command execution helpers
//! - `git/` - Git-specific implementation

pub mod commands;
pub mod git;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use tracing::{debug, info};

/// VCS-specific error type.
///
/// Wraps all VCS-related errors with backend context for better error messages.
#[derive(Error, Debug)]
pub enum VcsError {
    #[error("{backend} command failed: {message}")]
    Command {
        backend: VcsBackend,
        message: String,
    },

    #[error("Merge conflict in {backend}: {details}")]
    Conflict {
        backend: VcsBackend,
        details: String,
    },

    #[error("{backend} not available: {reason}")]
    #[allow(dead_code)] // Reserved for future VCS availability checks
    NotAvailable { backend: VcsBackend, reason: String },

    #[error("Uncommitted changes detected: {0}")]
    #[allow(dead_code)]
    UncommittedChanges(String),

    #[error("No VCS backend available for parallel execution")]
    NoBackend,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl VcsError {
    /// Create a command error for Git backend.
    pub fn git_command(message: impl Into<String>) -> Self {
        VcsError::Command {
            backend: VcsBackend::Git,
            message: message.into(),
        }
    }

    /// Create a conflict error for Git backend.
    pub fn git_conflict(details: impl Into<String>) -> Self {
        VcsError::Conflict {
            backend: VcsBackend::Git,
            details: details.into(),
        }
    }
}

/// Result type for VCS operations.
pub type VcsResult<T> = std::result::Result<T, VcsError>;

/// Warning information emitted by VCS checks.
#[derive(Debug, Clone)]
pub struct VcsWarning {
    pub title: String,
    pub message: String,
}

/// VCS backend type for parallel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VcsBackend {
    /// Automatically detect VCS (Git worktree)
    #[default]
    Auto,
    /// Git VCS
    Git,
}

impl std::fmt::Display for VcsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VcsBackend::Auto => write!(f, "auto"),
            VcsBackend::Git => write!(f, "git"),
        }
    }
}

impl std::str::FromStr for VcsBackend {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(VcsBackend::Auto),
            "git" => Ok(VcsBackend::Git),
            _ => Err(format!(
                "Invalid VCS backend: {}. Valid values: auto, git",
                s
            )),
        }
    }
}

/// Status of a workspace (shared between VCS implementations)
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Variants used in workspace state tracking
pub enum WorkspaceStatus {
    /// Workspace created but not yet used
    Created,
    /// Apply command is running
    Applying,
    /// Apply completed successfully with resulting revision
    Applied(String),
    /// Apply failed with error message
    Failed(String),
    /// Workspace merged into main
    Merged,
    /// Workspace cleaned up
    Cleaned,
}

/// Generic workspace information
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used in workspace state tracking
pub struct Workspace {
    /// Workspace name (used by VCS)
    pub name: String,
    /// Path to workspace directory
    pub path: PathBuf,
    /// Associated OpenSpec change ID
    pub change_id: String,
    /// Base revision workspace was created from
    pub base_revision: String,
    /// Current status
    pub status: WorkspaceStatus,
}

/// Information about an existing workspace found during resume detection.
///
/// This struct contains the minimal information needed to decide whether
/// to reuse an existing workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Path to the workspace directory
    pub path: PathBuf,
    /// Associated OpenSpec change ID (extracted from workspace name)
    pub change_id: String,
    /// Workspace name (used by VCS)
    pub workspace_name: String,
    /// Last modification time of the workspace directory
    pub last_modified: SystemTime,
}

/// Trait for VCS workspace management.
///
/// This trait abstracts VCS-specific operations needed for parallel execution,
/// allowing Git worktrees to be used interchangeably.
#[async_trait]
#[allow(dead_code)] // Some trait methods are reserved for future use
pub trait WorkspaceManager: Send + Sync {
    /// Get the VCS backend type
    fn backend_type(&self) -> VcsBackend;

    /// Check if this VCS is available for parallel execution
    async fn check_available(&self) -> VcsResult<bool>;

    /// Prepare for parallel execution.
    ///
    /// For Git: Verifies working directory is clean and returns a warning if not.
    async fn prepare_for_parallel(&self) -> VcsResult<Option<VcsWarning>>;

    /// Get the current revision/commit
    async fn get_current_revision(&self) -> VcsResult<String>;

    /// Create a new workspace for a change
    async fn create_workspace(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> VcsResult<Workspace>;

    /// Update workspace status
    fn update_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus);

    /// Merge multiple workspace revisions into main.
    ///
    /// Returns the final revision after merge.
    async fn merge_workspaces(&self, revisions: &[String]) -> VcsResult<String>;

    /// Cleanup a single workspace
    async fn cleanup_workspace(&mut self, workspace_name: &str) -> VcsResult<()>;

    /// Cleanup all workspaces
    async fn cleanup_all(&mut self) -> VcsResult<()>;

    /// Get the maximum concurrent workspaces limit
    fn max_concurrent(&self) -> usize;

    /// Get the list of active workspaces
    fn workspaces(&self) -> Vec<Workspace>;

    /// List change IDs that currently have worktrees.
    async fn list_worktree_change_ids(&self) -> VcsResult<HashSet<String>>;

    /// Get the conflict resolution prompt prefix for this VCS.
    ///
    /// Returns VCS-specific instructions for conflict resolution.
    fn conflict_resolution_prompt(&self) -> &'static str;

    /// Snapshot working copy changes.
    ///
    /// For Git: No-op (Git doesn't auto-snapshot).
    async fn snapshot_working_copy(&self, workspace_path: &Path) -> VcsResult<()>;

    /// Set the commit message for a workspace.
    ///
    /// For Git: `git commit --amend -m <message>` (if there's a commit)
    async fn set_commit_message(&self, workspace_path: &Path, message: &str) -> VcsResult<()>;

    /// Create an iteration snapshot with WIP commit message.
    ///
    /// For Git: Stage all changes and create/amend WIP commit with iteration number.
    async fn create_iteration_snapshot(
        &self,
        workspace_path: &Path,
        change_id: &str,
        iteration: u32,
        completed: u32,
        total: u32,
    ) -> VcsResult<()>;

    /// Squash all WIP snapshots into a single Apply commit.
    ///
    /// For Git: Use `git reset --soft` and `git commit` to squash.
    async fn squash_wip_commits(
        &self,
        workspace_path: &Path,
        change_id: &str,
        final_iteration: u32,
    ) -> VcsResult<()>;

    /// Get the current revision in a workspace.
    ///
    /// For Git: `git rev-parse HEAD`
    async fn get_revision_in_workspace(&self, workspace_path: &Path) -> VcsResult<String>;

    /// Get VCS status output for context in error messages.
    async fn get_status(&self) -> VcsResult<String>;

    /// Get log output for specific revisions (used for conflict resolution context).
    async fn get_log_for_revisions(&self, revisions: &[String]) -> VcsResult<String>;

    /// Detect conflicted files.
    ///
    /// Returns a list of file paths that have conflicts.
    async fn detect_conflicts(&self) -> VcsResult<Vec<String>>;

    /// Forget/cleanup a workspace by name (used in emergency cleanup).
    ///
    /// This is a synchronous operation for use in Drop implementations.
    fn forget_workspace_sync(&self, workspace_name: &str);

    /// Get the repository root path.
    fn repo_root(&self) -> &Path;

    /// Get the original branch name captured for parallel execution.
    ///
    /// Returns None if the branch has not been captured yet.
    fn original_branch(&self) -> Option<String>;

    /// Find an existing workspace for the given change ID.
    ///
    /// If multiple workspaces exist for the same change_id, returns the newest
    /// one (by last_modified time) and cleans up older ones.
    ///
    /// Returns None if no workspace exists for the given change_id.
    async fn find_existing_workspace(
        &mut self,
        change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>>;

    /// Reuse an existing workspace, registering it with the workspace manager.
    ///
    /// This is called after `find_existing_workspace` returns a workspace to reuse.
    /// It registers the workspace with the manager so it can be tracked, merged, and cleaned up.
    async fn reuse_workspace(&mut self, workspace_info: &WorkspaceInfo) -> VcsResult<Workspace>;
}

/// Detect the VCS backend to use based on configuration and repository state.
///
/// Detection order:
/// 1. If explicit backend is specified (not Auto), use that
/// 2. Check for .git directory → Git backend
/// 3. Return error if no VCS found
#[allow(dead_code)] // Reserved for future use in workspace initialization
pub async fn detect_vcs_backend<P: AsRef<Path>>(
    requested: VcsBackend,
    cwd: P,
) -> VcsResult<VcsBackend> {
    let cwd = cwd.as_ref();

    match requested {
        VcsBackend::Git => {
            // Explicit Git requested, verify it's available
            if git::commands::check_git_repo(cwd).await? {
                info!("Using explicitly requested Git backend");
                Ok(VcsBackend::Git)
            } else {
                Err(VcsError::NoBackend)
            }
        }
        VcsBackend::Auto => {
            // Auto-detect: Git only
            debug!("Auto-detecting VCS backend...");

            if git::commands::check_git_repo(cwd).await? {
                info!("Auto-detected Git backend");
                Ok(VcsBackend::Git)
            } else {
                Err(VcsError::NoBackend)
            }
        }
    }
}

// Re-export workspace managers for convenience
pub use git::GitWorkspaceManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcs_backend_from_str() {
        assert_eq!("auto".parse::<VcsBackend>().unwrap(), VcsBackend::Auto);
        assert_eq!("git".parse::<VcsBackend>().unwrap(), VcsBackend::Git);
        assert_eq!("Git".parse::<VcsBackend>().unwrap(), VcsBackend::Git);
        assert!("invalid".parse::<VcsBackend>().is_err());
    }

    #[test]
    fn test_vcs_backend_display() {
        assert_eq!(VcsBackend::Auto.to_string(), "auto");
        assert_eq!(VcsBackend::Git.to_string(), "git");
    }

    #[test]
    fn test_workspace_status_equality() {
        assert_eq!(WorkspaceStatus::Created, WorkspaceStatus::Created);
        assert_ne!(WorkspaceStatus::Created, WorkspaceStatus::Applying);
        assert_eq!(
            WorkspaceStatus::Applied("rev1".to_string()),
            WorkspaceStatus::Applied("rev1".to_string())
        );
    }

    #[test]
    fn test_vcs_error_constructors() {
        let err = VcsError::git_command("test error");
        assert!(matches!(
            err,
            VcsError::Command {
                backend: VcsBackend::Git,
                ..
            }
        ));

        let err = VcsError::git_conflict("conflict details");
        assert!(matches!(
            err,
            VcsError::Conflict {
                backend: VcsBackend::Git,
                ..
            }
        ));
    }

    // === Tests for parallel-execution spec (VCS Backend) ===

    #[test]
    fn test_vcs_backend_default_is_auto() {
        let backend: VcsBackend = Default::default();
        assert_eq!(backend, VcsBackend::Auto);
    }

    #[test]
    fn test_vcs_backend_serialization() {
        // Test serde serialization for config files
        let backend = VcsBackend::Git;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"git\"");

        let backend = VcsBackend::Auto;
        let json = serde_json::to_string(&backend).unwrap();
        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn test_vcs_backend_deserialization() {
        let git: VcsBackend = serde_json::from_str("\"git\"").unwrap();
        assert_eq!(git, VcsBackend::Git);

        let auto: VcsBackend = serde_json::from_str("\"auto\"").unwrap();
        assert_eq!(auto, VcsBackend::Auto);
    }

    // === Tests for workspace status lifecycle ===

    #[test]
    fn test_workspace_status_lifecycle() {
        // Test the expected lifecycle of workspace status
        let status = WorkspaceStatus::Created;
        assert_eq!(status, WorkspaceStatus::Created);

        let status = WorkspaceStatus::Applying;
        assert_eq!(status, WorkspaceStatus::Applying);

        let status = WorkspaceStatus::Applied("abc123".to_string());
        assert!(matches!(status, WorkspaceStatus::Applied(ref s) if s == "abc123"));

        let status = WorkspaceStatus::Merged;
        assert_eq!(status, WorkspaceStatus::Merged);

        let status = WorkspaceStatus::Cleaned;
        assert_eq!(status, WorkspaceStatus::Cleaned);
    }

    #[test]
    fn test_workspace_status_failed_includes_message() {
        let status = WorkspaceStatus::Failed("LLM timeout".to_string());
        assert!(matches!(status, WorkspaceStatus::Failed(ref msg) if msg == "LLM timeout"));
    }

    // === Tests for VcsError types ===

    #[test]
    fn test_vcs_error_uncommitted_changes() {
        let err = VcsError::UncommittedChanges("staged files exist".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("staged files exist"));
    }

    #[test]
    fn test_vcs_error_no_backend() {
        let err = VcsError::NoBackend;
        let msg = format!("{}", err);
        assert!(msg.contains("No VCS backend"));
    }

    #[test]
    fn test_vcs_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: VcsError = io_err.into();
        assert!(matches!(err, VcsError::Io(_)));
    }

    // === Tests for Workspace struct ===

    #[test]
    fn test_workspace_creation() {
        let ws = Workspace {
            name: "ws-add-feature-12345".to_string(),
            path: std::path::PathBuf::from("/tmp/workspaces/ws-add-feature-12345"),
            change_id: "add-feature".to_string(),
            base_revision: "abc123def456".to_string(),
            status: WorkspaceStatus::Created,
        };

        assert_eq!(ws.name, "ws-add-feature-12345");
        assert_eq!(ws.change_id, "add-feature");
        assert!(ws.path.to_str().unwrap().contains("ws-add-feature"));
    }

    #[test]
    fn test_workspace_name_sanitization_pattern() {
        // The workspace naming pattern is "ws-{sanitized_change_id}-{timestamp}"
        // Verify the expected pattern structure
        let change_id = "feature/add-login";
        let sanitized = format!("ws-{}-12345", change_id.replace(['/', '\\', ' '], "-"));
        assert_eq!(sanitized, "ws-feature-add-login-12345");
        assert!(!sanitized.contains('/'));
        assert!(!sanitized.contains('\\'));
    }
}
