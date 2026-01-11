//! VCS backend abstraction for parallel execution.
//!
//! This module provides a trait-based abstraction for VCS operations,
//! allowing parallel execution to work with both jj and Git.
//!
//! Note: Some trait methods are reserved for future use in workspace
//! orchestration and parallel execution improvements.

// Allow unused trait methods - they are part of the VCS abstraction layer
// and will be used as the implementation matures
#![allow(dead_code)]

use crate::error::{OrchestratorError, Result};
use crate::git_commands;
use crate::jj_commands;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

/// VCS backend type for parallel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VcsBackend {
    /// Automatically detect VCS (jj preferred, then Git)
    #[default]
    Auto,
    /// jj (Jujutsu) VCS
    Jj,
    /// Git VCS
    Git,
}

impl std::fmt::Display for VcsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VcsBackend::Auto => write!(f, "auto"),
            VcsBackend::Jj => write!(f, "jj"),
            VcsBackend::Git => write!(f, "git"),
        }
    }
}

impl std::str::FromStr for VcsBackend {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(VcsBackend::Auto),
            "jj" => Ok(VcsBackend::Jj),
            "git" => Ok(VcsBackend::Git),
            _ => Err(format!(
                "Invalid VCS backend: {}. Valid values: auto, jj, git",
                s
            )),
        }
    }
}

/// Status of a workspace (shared between jj and Git implementations)
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
    pub path: std::path::PathBuf,
    /// Associated OpenSpec change ID
    pub change_id: String,
    /// Base revision workspace was created from
    pub base_revision: String,
    /// Current status
    pub status: WorkspaceStatus,
}

/// Trait for VCS workspace management.
///
/// This trait abstracts VCS-specific operations needed for parallel execution,
/// allowing both jj and Git backends to be used interchangeably.
#[async_trait]
pub trait WorkspaceManager: Send + Sync {
    /// Get the VCS backend type
    fn backend_type(&self) -> VcsBackend;

    /// Check if this VCS is available for parallel execution
    async fn check_available(&self) -> Result<bool>;

    /// Prepare for parallel execution.
    ///
    /// For jj: Creates a snapshot of working copy changes.
    /// For Git: Verifies working directory is clean (returns error if not).
    async fn prepare_for_parallel(&self) -> Result<()>;

    /// Get the current revision/commit
    async fn get_current_revision(&self) -> Result<String>;

    /// Create a new workspace for a change
    async fn create_workspace(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> Result<Workspace>;

    /// Update workspace status
    fn update_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus);

    /// Merge multiple workspace revisions into main.
    ///
    /// Returns the final revision after merge.
    async fn merge_workspaces(&self, revisions: &[String]) -> Result<String>;

    /// Cleanup a single workspace
    async fn cleanup_workspace(&mut self, workspace_name: &str) -> Result<()>;

    /// Cleanup all workspaces
    async fn cleanup_all(&mut self) -> Result<()>;

    /// Get the maximum concurrent workspaces limit
    fn max_concurrent(&self) -> usize;

    /// Get the list of active workspaces
    fn workspaces(&self) -> Vec<Workspace>;

    /// Get the conflict resolution prompt prefix for this VCS.
    ///
    /// Returns VCS-specific instructions for conflict resolution.
    fn conflict_resolution_prompt(&self) -> &'static str;

    /// Snapshot working copy changes.
    ///
    /// For jj: Runs `jj status` to trigger automatic snapshotting.
    /// For Git: No-op (Git doesn't auto-snapshot).
    async fn snapshot_working_copy(&self, workspace_path: &Path) -> Result<()>;

    /// Set the commit message for a workspace.
    ///
    /// For jj: `jj describe -m <message>`
    /// For Git: `git commit --amend -m <message>` (if there's a commit)
    async fn set_commit_message(&self, workspace_path: &Path, message: &str) -> Result<()>;

    /// Get the current revision in a workspace.
    ///
    /// For jj: `jj log -r @ -T change_id`
    /// For Git: `git rev-parse HEAD`
    async fn get_revision_in_workspace(&self, workspace_path: &Path) -> Result<String>;

    /// Get VCS status output for context in error messages.
    async fn get_status(&self) -> Result<String>;

    /// Get log output for specific revisions (used for conflict resolution context).
    async fn get_log_for_revisions(&self, revisions: &[String]) -> Result<String>;

    /// Detect conflicted files.
    ///
    /// Returns a list of file paths that have conflicts.
    async fn detect_conflicts(&self) -> Result<Vec<String>>;

    /// Forget/cleanup a workspace by name (used in emergency cleanup).
    ///
    /// This is a synchronous operation for use in Drop implementations.
    fn forget_workspace_sync(&self, workspace_name: &str);

    /// Get the repository root path.
    fn repo_root(&self) -> &Path;
}

/// Detect the VCS backend to use based on configuration and repository state.
///
/// Detection order:
/// 1. If explicit backend is specified (not Auto), use that
/// 2. Check for .jj directory → jj backend
/// 3. Check for .git directory → Git backend
/// 4. Return error if no VCS found
#[allow(dead_code)] // Reserved for future use in workspace initialization
pub async fn detect_vcs_backend<P: AsRef<Path>>(
    requested: VcsBackend,
    cwd: P,
) -> Result<VcsBackend> {
    let cwd = cwd.as_ref();

    match requested {
        VcsBackend::Jj => {
            // Explicit jj requested, verify it's available
            if jj_commands::check_jj_repo(cwd).await? {
                info!("Using explicitly requested jj backend");
                Ok(VcsBackend::Jj)
            } else {
                Err(OrchestratorError::NoVcsBackend)
            }
        }
        VcsBackend::Git => {
            // Explicit Git requested, verify it's available
            if git_commands::check_git_repo(cwd).await? {
                info!("Using explicitly requested Git backend");
                Ok(VcsBackend::Git)
            } else {
                Err(OrchestratorError::NoVcsBackend)
            }
        }
        VcsBackend::Auto => {
            // Auto-detect: jj first, then Git
            debug!("Auto-detecting VCS backend...");

            // Check for jj first (preferred)
            if jj_commands::check_jj_repo(cwd).await? {
                info!("Auto-detected jj backend");
                return Ok(VcsBackend::Jj);
            }

            // Check for Git
            if git_commands::check_git_repo(cwd).await? {
                info!("Auto-detected Git backend");
                return Ok(VcsBackend::Git);
            }

            // No VCS found
            Err(OrchestratorError::NoVcsBackend)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vcs_backend_from_str() {
        assert_eq!("auto".parse::<VcsBackend>().unwrap(), VcsBackend::Auto);
        assert_eq!("jj".parse::<VcsBackend>().unwrap(), VcsBackend::Jj);
        assert_eq!("git".parse::<VcsBackend>().unwrap(), VcsBackend::Git);
        assert_eq!("Git".parse::<VcsBackend>().unwrap(), VcsBackend::Git);
        assert_eq!("JJ".parse::<VcsBackend>().unwrap(), VcsBackend::Jj);
        assert!("invalid".parse::<VcsBackend>().is_err());
    }

    #[test]
    fn test_vcs_backend_display() {
        assert_eq!(VcsBackend::Auto.to_string(), "auto");
        assert_eq!(VcsBackend::Jj.to_string(), "jj");
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
}
