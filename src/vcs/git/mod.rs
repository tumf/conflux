//! Git worktree workspace management for parallel change execution.
//!
//! This module provides workspace creation, merge, and cleanup functionality
//! to enable parallel execution of changes in isolated Git worktrees.

pub mod commands;

use crate::config::OrchestratorConfig;
use crate::vcs::{VcsBackend, VcsError, VcsResult, Workspace, WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Represents a Git worktree for parallel execution
#[derive(Debug, Clone)]
pub struct GitWorkspace {
    /// Workspace name (branch name)
    pub name: String,
    /// Path to worktree directory
    pub path: PathBuf,
    /// Associated OpenSpec change ID
    pub change_id: String,
    /// Base commit workspace was created from
    pub base_revision: String,
    /// Current status
    pub status: WorkspaceStatus,
}

impl From<GitWorkspace> for Workspace {
    fn from(ws: GitWorkspace) -> Self {
        Workspace {
            name: ws.name,
            path: ws.path,
            change_id: ws.change_id,
            base_revision: ws.base_revision,
            status: ws.status,
        }
    }
}

/// Manages Git worktrees for parallel execution
pub struct GitWorkspaceManager {
    /// Base directory for creating worktrees
    base_dir: PathBuf,
    /// Repository root directory
    repo_root: PathBuf,
    /// Active workspaces
    workspaces: Vec<GitWorkspace>,
    /// Maximum concurrent workspaces
    max_concurrent: usize,
    /// Original branch name to return to after operations
    original_branch: Option<String>,
}

impl GitWorkspaceManager {
    /// Create a new Git workspace manager
    pub fn new(
        base_dir: PathBuf,
        repo_root: PathBuf,
        max_concurrent: usize,
        _config: OrchestratorConfig,
    ) -> Self {
        Self {
            base_dir,
            repo_root,
            workspaces: Vec::new(),
            max_concurrent,
            original_branch: None,
        }
    }

    /// Get the list of active workspaces (Git-specific type)
    #[allow(dead_code)]
    pub fn git_workspaces(&self) -> &[GitWorkspace] {
        &self.workspaces
    }

    /// Check if Git is available and the repo is a Git repository
    #[allow(dead_code)]
    pub async fn check_git_available(&self) -> VcsResult<bool> {
        commands::check_git_repo(&self.repo_root).await
    }

    /// Check if working directory is clean (no uncommitted changes or untracked files).
    ///
    /// Returns an error with detailed message if there are uncommitted changes.
    pub async fn check_clean_working_directory(&self) -> VcsResult<()> {
        let (has_changes, status) = commands::has_uncommitted_changes(&self.repo_root).await?;

        if has_changes {
            let mut error_msg =
                String::from("Cannot start parallel execution with uncommitted changes.\n\n");
            error_msg.push_str("The following files have uncommitted changes:\n");
            error_msg.push_str(&status);
            error_msg.push_str(
                "\n\nPlease commit or stash your changes before running parallel execution.\n",
            );
            error_msg.push_str("Commands to resolve:\n");
            error_msg.push_str("  git add -A && git commit -m 'WIP'  # Commit changes\n");
            error_msg.push_str("  git stash                          # Or stash changes\n");

            return Err(VcsError::UncommittedChanges(error_msg));
        }

        Ok(())
    }

    /// Get the current commit hash
    pub async fn get_current_commit(&self) -> VcsResult<String> {
        commands::get_current_commit(&self.repo_root).await
    }

    /// Create a new worktree for a change from a specific base commit
    pub async fn create_worktree(
        &mut self,
        change_id: &str,
        base_commit: Option<&str>,
    ) -> VcsResult<GitWorkspace> {
        // Store original branch if not already stored
        if self.original_branch.is_none() {
            self.original_branch = match commands::get_current_branch(&self.repo_root).await? {
                Some(branch) => Some(branch),
                None => return Err(VcsError::git_command(
                    "Detached HEAD state detected. Checkout a branch before running parallel mode.",
                )),
            };
        }

        // Sanitize change_id and add unique suffix for workspace name
        let unique_suffix = format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u32)
                .unwrap_or(0)
        );
        let branch_name = format!(
            "ws-{}-{}",
            change_id.replace(['/', '\\', ' '], "-"),
            unique_suffix
        );
        let worktree_path = self.base_dir.join(&branch_name);

        // Ensure base directory exists
        if !self.base_dir.exists() {
            std::fs::create_dir_all(&self.base_dir)?;
        }

        // Get base commit (use HEAD if not specified)
        let base = match base_commit {
            Some(commit) => commit.to_string(),
            None => self.get_current_commit().await?,
        };

        info!(
            "Creating worktree '{}' at {:?} from commit {}",
            branch_name,
            worktree_path,
            &base[..8.min(base.len())]
        );

        // Create worktree with new branch
        commands::worktree_add(
            &self.repo_root,
            worktree_path.to_str().unwrap(),
            &branch_name,
            &base,
        )
        .await?;

        let workspace = GitWorkspace {
            name: branch_name,
            path: worktree_path,
            change_id: change_id.to_string(),
            base_revision: base,
            status: WorkspaceStatus::Created,
        };

        self.workspaces.push(workspace.clone());
        debug!("Created worktree: {:?}", workspace.name);

        Ok(workspace)
    }

    /// Update workspace status (Git-specific implementation)
    pub fn update_git_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus) {
        if let Some(ws) = self
            .workspaces
            .iter_mut()
            .find(|w| w.name == workspace_name)
        {
            ws.status = status;
        }
    }

    /// Merge multiple workspace branches into the original branch (sequential merge).
    ///
    /// Returns the final commit hash after all merges.
    pub async fn merge_branches(&self, branch_names: &[String]) -> VcsResult<String> {
        if branch_names.is_empty() {
            return Err(VcsError::git_command("No branches to merge"));
        }

        // Determine the target for merge
        let original = self.original_branch.as_deref().unwrap_or("main");

        // Always merge into the original branch
        info!("Checking out original branch '{}' for merge", original);
        commands::checkout(&self.repo_root, original).await?;

        // Sequential merge: merge each branch one at a time
        for branch_name in branch_names {
            info!("Merging branch '{}'", branch_name);
            commands::merge(&self.repo_root, branch_name).await?;
        }

        // Get the final commit hash
        let final_commit = self.get_current_commit().await?;
        info!(
            "All branches merged successfully. Final commit: {}",
            &final_commit[..8.min(final_commit.len())]
        );

        Ok(final_commit)
    }

    /// Cleanup a single worktree (remove worktree + delete branch)
    pub async fn cleanup_worktree(&mut self, workspace_name: &str) -> VcsResult<()> {
        let workspace = self
            .workspaces
            .iter()
            .find(|w| w.name == workspace_name)
            .cloned();

        let Some(workspace) = workspace else {
            warn!("Worktree '{}' not found for cleanup", workspace_name);
            return Ok(());
        };

        info!("Cleaning up worktree '{}'", workspace_name);

        // Remove worktree
        if workspace.path.exists() {
            if let Err(e) =
                commands::worktree_remove(&self.repo_root, workspace.path.to_str().unwrap()).await
            {
                warn!("Failed to remove worktree '{}': {}", workspace_name, e);
                // Try force removal via filesystem
                if let Err(e) = std::fs::remove_dir_all(&workspace.path) {
                    warn!(
                        "Failed to force remove worktree directory {:?}: {}",
                        workspace.path, e
                    );
                }
            }
        }

        // Delete branch (ignore errors - branch may have been merged)
        if let Err(e) = commands::branch_delete(&self.repo_root, workspace_name).await {
            debug!(
                "Failed to delete branch '{}': {} (may have been merged)",
                workspace_name, e
            );
        }

        // Update status
        self.update_git_workspace_status(workspace_name, WorkspaceStatus::Cleaned);

        debug!("Worktree '{}' cleaned up", workspace_name);
        Ok(())
    }

    /// Cleanup all worktrees
    pub async fn cleanup_all_worktrees(&mut self) -> VcsResult<()> {
        let workspace_names: Vec<String> = self.workspaces.iter().map(|w| w.name.clone()).collect();

        for name in workspace_names {
            let _ = self.cleanup_worktree(&name).await;
        }

        // Clear the workspace list
        self.workspaces.clear();

        // Try to remove the base directory if empty
        if self.base_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
                if entries.count() == 0 {
                    let _ = std::fs::remove_dir(&self.base_dir);
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl WorkspaceManager for GitWorkspaceManager {
    fn backend_type(&self) -> VcsBackend {
        VcsBackend::Git
    }

    async fn check_available(&self) -> VcsResult<bool> {
        self.check_git_available().await
    }

    async fn prepare_for_parallel(&self) -> VcsResult<()> {
        // Git requires a clean working directory
        self.check_clean_working_directory().await
    }

    async fn get_current_revision(&self) -> VcsResult<String> {
        self.get_current_commit().await
    }

    async fn create_workspace(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> VcsResult<Workspace> {
        let git_ws = self.create_worktree(change_id, base_revision).await?;
        Ok(git_ws.into())
    }

    fn update_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus) {
        self.update_git_workspace_status(workspace_name, status);
    }

    async fn merge_workspaces(&self, revisions: &[String]) -> VcsResult<String> {
        // For Git, revisions are branch names
        self.merge_branches(revisions).await
    }

    async fn cleanup_workspace(&mut self, workspace_name: &str) -> VcsResult<()> {
        self.cleanup_worktree(workspace_name).await
    }

    async fn cleanup_all(&mut self) -> VcsResult<()> {
        self.cleanup_all_worktrees().await
    }

    fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    fn workspaces(&self) -> Vec<Workspace> {
        self.workspaces
            .iter()
            .map(|w| Workspace {
                name: w.name.clone(),
                path: w.path.clone(),
                change_id: w.change_id.clone(),
                base_revision: w.base_revision.clone(),
                status: w.status.clone(),
            })
            .collect()
    }

    fn conflict_resolution_prompt(&self) -> &'static str {
        "This project uses Git for version control.\n\n\
         A merge conflict occurred. The conflicting files contain Git conflict markers:\n\
         <<<<<<< HEAD\n\
         [your changes]\n\
         =======\n\
         [incoming changes]\n\
         >>>>>>> [branch]\n\n\
         Please resolve the conflicts by:\n\
         1. Editing the conflicting files to remove conflict markers\n\
         2. Choosing the correct content for each conflict\n\
         3. Running `git add <file>` for each resolved file\n\
         4. Running `git commit` to complete the merge"
    }

    async fn snapshot_working_copy(&self, _workspace_path: &Path) -> VcsResult<()> {
        // Git doesn't have automatic snapshotting like jj
        // No-op for Git
        Ok(())
    }

    async fn set_commit_message(&self, workspace_path: &Path, message: &str) -> VcsResult<()> {
        // First check if there are any changes to commit
        if commands::has_changes_to_commit(workspace_path).await? {
            // Stage all changes and create a commit
            commands::add_and_commit(workspace_path, message).await?;
        } else {
            // Try to amend the last commit with the new message
            let result =
                commands::run_git(&["commit", "--amend", "-m", message], workspace_path).await;
            if let Err(e) = result {
                warn!("Failed to amend commit message: {}", e);
            }
        }
        Ok(())
    }

    async fn get_revision_in_workspace(&self, workspace_path: &Path) -> VcsResult<String> {
        commands::get_current_commit(workspace_path).await
    }

    async fn get_status(&self) -> VcsResult<String> {
        commands::get_status(&self.repo_root).await
    }

    async fn get_log_for_revisions(&self, revisions: &[String]) -> VcsResult<String> {
        if revisions.is_empty() {
            return Ok(String::new());
        }

        // Get log for each revision
        let mut logs = Vec::new();
        for rev in revisions {
            let log = commands::run_git(&["log", "-1", "--oneline", rev], &self.repo_root).await?;
            logs.push(log);
        }

        Ok(logs.join("\n"))
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        commands::get_conflict_files(&self.repo_root).await
    }

    fn forget_workspace_sync(&self, workspace_name: &str) {
        debug!(
            "Emergency cleanup: removing git worktree for '{}'",
            workspace_name
        );

        // Find the workspace path
        if let Some(workspace) = self.workspaces.iter().find(|w| w.name == workspace_name) {
            let path = workspace.path.to_str().unwrap_or("");

            // Try to remove the worktree
            let result = std::process::Command::new("git")
                .args(["worktree", "remove", path, "--force"])
                .current_dir(&self.repo_root)
                .output();

            match result {
                Ok(output) if !output.status.success() => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    debug!("Failed to remove worktree '{}': {}", workspace_name, stderr);
                    // Try to remove the directory directly
                    if workspace.path.exists() {
                        let _ = std::fs::remove_dir_all(&workspace.path);
                    }
                }
                Err(e) => {
                    debug!("Failed to run git worktree remove: {}", e);
                    // Try to remove the directory directly
                    if workspace.path.exists() {
                        let _ = std::fs::remove_dir_all(&workspace.path);
                    }
                }
                _ => {
                    debug!("Successfully removed worktree '{}'", workspace_name);
                }
            }

            // Try to delete the branch
            let _ = std::process::Command::new("git")
                .args(["branch", "-D", workspace_name])
                .current_dir(&self.repo_root)
                .output();
        }
    }

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (GitWorkspaceManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join("worktrees");
        let repo_root = temp_dir.path().to_path_buf();
        let config = OrchestratorConfig::default();

        let manager = GitWorkspaceManager::new(base_dir, repo_root, 3, config);
        (manager, temp_dir)
    }

    #[test]
    fn test_manager_creation() {
        let (manager, _temp) = create_test_manager();
        assert_eq!(manager.max_concurrent, 3);
        assert!(manager.workspaces.is_empty());
    }

    #[test]
    fn test_workspace_name_sanitization() {
        let change_id = "feature/add-login";
        let sanitized = format!("ws-{}", change_id.replace(['/', '\\', ' '], "-"));
        assert_eq!(sanitized, "ws-feature-add-login");
    }

    #[test]
    fn test_backend_type() {
        let (manager, _temp) = create_test_manager();
        assert_eq!(manager.backend_type(), VcsBackend::Git);
    }
}
