//! Git worktree workspace management for parallel change execution.
//!
//! This module provides workspace creation, merge, and cleanup functionality
//! to enable parallel execution of changes in isolated Git worktrees.

pub mod commands;

use crate::config::OrchestratorConfig;
use crate::vcs::{
    VcsBackend, VcsError, VcsResult, VcsWarning, Workspace, WorkspaceInfo, WorkspaceManager,
    WorkspaceStatus,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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
    /// Returns a warning message if there are uncommitted changes, or None if clean.
    pub async fn check_clean_working_directory(&self) -> VcsResult<Option<VcsWarning>> {
        let (has_changes, status) = commands::has_uncommitted_changes(&self.repo_root).await?;

        if has_changes {
            let warning_msg = format!(
                "Warning: Uncommitted changes detected.\n\
                 Parallel mode will continue, but uncommitted changes remain in your working directory.\n\
                 Consider committing or stashing if you need isolated workspaces.\n\n\
                 The following files have uncommitted changes:\n{}",
                if status.trim().is_empty() {
                    " (none listed)".to_string()
                } else {
                    format!("\n{}", status)
                }
            );
            Ok(Some(VcsWarning {
                title: "Uncommitted Changes Detected".to_string(),
                message: warning_msg,
            }))
        } else {
            Ok(None)
        }
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
        let mut workspace_path = None;
        let mut change_id = None;

        if let Some(workspace) = self.workspaces.iter().find(|w| w.name == workspace_name) {
            workspace_path = Some(workspace.path.clone());
            change_id = Some(workspace.change_id.clone());
        }

        if workspace_path.is_none() {
            if let Some(info) = self.find_worktree_by_name(workspace_name).await? {
                workspace_path = Some(info.path);
                change_id = Some(info.change_id);
            }
        }

        if workspace_path.is_none() {
            if let Some(extracted_change_id) =
                Self::extract_change_id_from_worktree_name(workspace_name)
            {
                let candidates = self
                    .find_all_worktrees_for_change(&extracted_change_id)
                    .await?;
                if let Some(matching) = candidates
                    .iter()
                    .find(|candidate| candidate.workspace_name == workspace_name)
                {
                    workspace_path = Some(matching.path.clone());
                    change_id = Some(matching.change_id.clone());
                } else if let Some(newest) = candidates.first() {
                    warn!(
                        "Worktree '{}' not found in tracked list; using newest worktree '{}' for change '{}'",
                        workspace_name, newest.workspace_name, extracted_change_id
                    );
                    workspace_path = Some(newest.path.clone());
                    change_id = Some(newest.change_id.clone());
                }
            }
        }

        let Some(worktree_path) = workspace_path else {
            warn!("Worktree '{}' not found for cleanup", workspace_name);
            return Ok(());
        };

        info!("Cleaning up worktree '{}'", workspace_name);

        // Remove worktree
        if worktree_path.exists() {
            if let Err(e) =
                commands::worktree_remove(&self.repo_root, worktree_path.to_str().unwrap()).await
            {
                warn!("Failed to remove worktree '{}': {}", workspace_name, e);
                // Try force removal via filesystem
                if let Err(e) = std::fs::remove_dir_all(&worktree_path) {
                    warn!(
                        "Failed to force remove worktree directory {:?}: {}",
                        worktree_path, e
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
        if let Some(change_id) = change_id {
            self.update_git_workspace_status(workspace_name, WorkspaceStatus::Cleaned);
            debug!(
                "Worktree '{}' cleaned up for change '{}'",
                workspace_name, change_id
            );
        } else {
            self.update_git_workspace_status(workspace_name, WorkspaceStatus::Cleaned);
            debug!("Worktree '{}' cleaned up", workspace_name);
        }
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

    /// Extract the change_id from a worktree name/branch name.
    ///
    /// Worktree names are in format: "ws-{sanitized_change_id}-{unique_suffix}"
    /// This function extracts the change_id portion.
    fn extract_change_id_from_worktree_name(worktree_name: &str) -> Option<String> {
        // Expected format: ws-{change_id}-{hex_suffix}
        if !worktree_name.starts_with("ws-") {
            return None;
        }

        let without_prefix = &worktree_name[3..]; // Remove "ws-"

        // Find the last dash followed by hex digits (the unique suffix)
        // The suffix is 8 hex characters
        if let Some(last_dash_pos) = without_prefix.rfind('-') {
            let potential_suffix = &without_prefix[last_dash_pos + 1..];
            // Check if suffix looks like a hex timestamp (at least 7 hex chars)
            if potential_suffix.len() >= 7
                && potential_suffix.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Some(without_prefix[..last_dash_pos].to_string());
            }
        }

        // Fallback: return everything after "ws-" (worktree might not have suffix)
        Some(without_prefix.to_string())
    }

    /// Find a worktree by branch name (workspace name).
    async fn find_worktree_by_name(
        &self,
        workspace_name: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        let output =
            commands::run_git(&["worktree", "list", "--porcelain"], &self.repo_root).await?;
        let mut current_worktree_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;

        for line in output.lines() {
            if let Some(worktree_path) = line.strip_prefix("worktree ") {
                current_worktree_path = Some(PathBuf::from(worktree_path));
            } else if let Some(branch_name) = line.strip_prefix("branch refs/heads/") {
                current_branch = Some(branch_name.to_string());
            } else if line.is_empty() {
                if let (Some(path), Some(branch)) = (&current_worktree_path, &current_branch) {
                    if branch == workspace_name {
                        let last_modified = if path.exists() {
                            path.metadata()
                                .and_then(|m| m.modified())
                                .unwrap_or(SystemTime::UNIX_EPOCH)
                        } else {
                            SystemTime::UNIX_EPOCH
                        };
                        let change_id = Self::extract_change_id_from_worktree_name(branch)
                            .unwrap_or_else(|| workspace_name.to_string());
                        return Ok(Some(WorkspaceInfo {
                            path: path.clone(),
                            change_id,
                            workspace_name: branch.clone(),
                            last_modified,
                        }));
                    }
                }

                current_worktree_path = None;
                current_branch = None;
            }
        }

        if let (Some(path), Some(branch)) = (&current_worktree_path, &current_branch) {
            if branch == workspace_name {
                let last_modified = if path.exists() {
                    path.metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH)
                } else {
                    SystemTime::UNIX_EPOCH
                };
                let change_id = Self::extract_change_id_from_worktree_name(branch)
                    .unwrap_or_else(|| workspace_name.to_string());
                return Ok(Some(WorkspaceInfo {
                    path: path.clone(),
                    change_id,
                    workspace_name: branch.clone(),
                    last_modified,
                }));
            }
        }

        Ok(None)
    }

    /// Find all existing worktrees for the given change_id.
    ///
    /// Returns workspace info for all matching worktrees, sorted by last_modified (newest first).
    async fn find_all_worktrees_for_change(
        &self,
        change_id: &str,
    ) -> VcsResult<Vec<WorkspaceInfo>> {
        // Run git worktree list to get all worktrees
        let output =
            commands::run_git(&["worktree", "list", "--porcelain"], &self.repo_root).await?;
        let sanitized_change_id = change_id.replace(['/', '\\', ' '], "-");

        let mut candidates = Vec::new();
        let mut current_worktree_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;

        for line in output.lines() {
            if let Some(worktree_path) = line.strip_prefix("worktree ") {
                // New worktree entry
                current_worktree_path = Some(PathBuf::from(worktree_path));
            } else if let Some(branch_name) = line.strip_prefix("branch refs/heads/") {
                // Branch name
                current_branch = Some(branch_name.to_string());
            } else if line.is_empty() {
                // End of entry, process if we have both path and branch
                if let (Some(path), Some(branch)) = (&current_worktree_path, &current_branch) {
                    // Check if this worktree matches our change_id
                    if let Some(extracted_change_id) =
                        Self::extract_change_id_from_worktree_name(branch)
                    {
                        if extracted_change_id == sanitized_change_id {
                            // Get last modified time
                            let last_modified = if path.exists() {
                                path.metadata()
                                    .and_then(|m| m.modified())
                                    .unwrap_or(SystemTime::UNIX_EPOCH)
                            } else {
                                SystemTime::UNIX_EPOCH
                            };

                            candidates.push(WorkspaceInfo {
                                path: path.clone(),
                                change_id: change_id.to_string(),
                                workspace_name: branch.clone(),
                                last_modified,
                            });
                        }
                    }
                }

                // Reset for next entry
                current_worktree_path = None;
                current_branch = None;
            }
        }

        // Process last entry if exists
        if let (Some(path), Some(branch)) = (&current_worktree_path, &current_branch) {
            if let Some(extracted_change_id) = Self::extract_change_id_from_worktree_name(branch) {
                if extracted_change_id == sanitized_change_id {
                    let last_modified = if path.exists() {
                        path.metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(SystemTime::UNIX_EPOCH)
                    } else {
                        SystemTime::UNIX_EPOCH
                    };

                    candidates.push(WorkspaceInfo {
                        path: path.clone(),
                        change_id: change_id.to_string(),
                        workspace_name: branch.clone(),
                        last_modified,
                    });
                }
            }
        }

        // Sort by last_modified, newest first
        candidates.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

        Ok(candidates)
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

    async fn prepare_for_parallel(&self) -> VcsResult<Option<VcsWarning>> {
        // Git requires a clean working directory - now only warns if not clean
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
        // Git doesn't have automatic snapshotting
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

    async fn create_iteration_snapshot(
        &self,
        workspace_path: &Path,
        change_id: &str,
        iteration: u32,
        completed: u32,
        total: u32,
    ) -> VcsResult<()> {
        let wip_message = format!(
            "WIP: {} ({}/{} tasks, apply#{})",
            change_id, completed, total, iteration
        );

        debug!(
            "Creating iteration snapshot #{} for {}",
            iteration, change_id
        );

        // Stage all changes
        commands::run_git(&["add", "-A"], workspace_path).await?;

        // Create or amend commit with --allow-empty to ensure snapshot is created
        // even if there are no file changes
        let has_commits = commands::run_git(&["rev-parse", "HEAD"], workspace_path)
            .await
            .is_ok();

        if has_commits {
            // Amend existing commit
            let result = commands::run_git(
                &["commit", "--amend", "--allow-empty", "-m", &wip_message],
                workspace_path,
            )
            .await;
            if let Err(e) = result {
                warn!(
                    "Failed to amend WIP commit for iteration {}: {}",
                    iteration, e
                );
            } else {
                debug!(
                    "Iteration snapshot #{} created for {} (amended)",
                    iteration, change_id
                );
            }
        } else {
            // Create initial commit
            let result = commands::run_git(
                &["commit", "--allow-empty", "-m", &wip_message],
                workspace_path,
            )
            .await;
            if let Err(e) = result {
                warn!(
                    "Failed to create initial WIP commit for iteration {}: {}",
                    iteration, e
                );
            } else {
                debug!(
                    "Iteration snapshot #{} created for {} (initial)",
                    iteration, change_id
                );
            }
        }

        Ok(())
    }

    async fn squash_wip_commits(
        &self,
        workspace_path: &Path,
        change_id: &str,
        final_iteration: u32,
    ) -> VcsResult<()> {
        let apply_message = format!("Apply: {} (apply#{})", change_id, final_iteration);

        debug!("Squashing WIP commits for {} into Apply commit", change_id);

        // For Git, we update the commit message to final Apply message
        // Since we've been amending the same commit, we just need to update the message
        let result =
            commands::run_git(&["commit", "--amend", "-m", &apply_message], workspace_path).await;

        if let Err(e) = result {
            return Err(VcsError::git_command(format!(
                "Failed to set Apply message: {}",
                e
            )));
        }

        info!("WIP commits squashed into Apply commit for {}", change_id);
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
            debug!(
                "Executing git command: git worktree remove {} --force (cwd: {:?})",
                path, self.repo_root
            );
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
            debug!(
                "Executing git command: git branch -D {} (cwd: {:?})",
                workspace_name, self.repo_root
            );
            let _ = std::process::Command::new("git")
                .args(["branch", "-D", workspace_name])
                .current_dir(&self.repo_root)
                .output();
        }
    }

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    async fn find_existing_workspace(
        &mut self,
        change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        let mut candidates = self.find_all_worktrees_for_change(change_id).await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        // Take the newest worktree (first in sorted list)
        let newest = candidates.remove(0);

        // Clean up older worktrees
        for old_ws in candidates {
            info!(
                "Cleaning up older worktree '{}' for change '{}'",
                old_ws.workspace_name, change_id
            );

            // Remove the worktree
            if old_ws.path.exists() {
                if let Err(e) =
                    commands::worktree_remove(&self.repo_root, old_ws.path.to_str().unwrap()).await
                {
                    warn!(
                        "Failed to remove worktree '{}': {}",
                        old_ws.workspace_name, e
                    );
                    // Try force removal via filesystem
                    if let Err(e) = std::fs::remove_dir_all(&old_ws.path) {
                        warn!(
                            "Failed to force remove worktree directory {:?}: {}",
                            old_ws.path, e
                        );
                    }
                }
            }

            // Delete the branch
            if let Err(e) = commands::branch_delete(&self.repo_root, &old_ws.workspace_name).await {
                debug!(
                    "Failed to delete branch '{}': {} (may have been merged)",
                    old_ws.workspace_name, e
                );
            }
        }

        debug!(
            "Found existing worktree '{}' for change '{}' (last modified: {:?})",
            newest.workspace_name, change_id, newest.last_modified
        );

        Ok(Some(newest))
    }

    async fn reuse_workspace(&mut self, workspace_info: &WorkspaceInfo) -> VcsResult<Workspace> {
        info!(
            "Reusing existing worktree '{}' at {:?}",
            workspace_info.workspace_name, workspace_info.path
        );

        // Get the current commit in the worktree
        let base_revision = if workspace_info.path.exists() {
            commands::get_current_commit(&workspace_info.path)
                .await
                .unwrap_or_else(|_| "unknown".to_string())
        } else {
            "unknown".to_string()
        };

        let workspace = GitWorkspace {
            name: workspace_info.workspace_name.clone(),
            path: workspace_info.path.clone(),
            change_id: workspace_info.change_id.clone(),
            base_revision,
            status: WorkspaceStatus::Created,
        };

        self.workspaces.push(workspace.clone());

        Ok(Workspace {
            name: workspace.name,
            path: workspace.path,
            change_id: workspace.change_id,
            base_revision: workspace.base_revision,
            status: workspace.status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

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

    #[tokio::test]
    async fn test_check_clean_working_directory_warns_when_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join("worktrees");
        let repo_root = temp_dir.path().to_path_buf();

        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            return;
        }

        std::fs::write(temp_dir.path().join("dirty.txt"), "content").unwrap();

        let manager =
            GitWorkspaceManager::new(base_dir, repo_root, 3, OrchestratorConfig::default());
        let warning = manager.check_clean_working_directory().await.unwrap();
        assert!(warning.is_some());

        let warning = warning.unwrap();
        assert!(warning
            .message
            .contains("Warning: Uncommitted changes detected."));
        assert!(warning.message.contains("Parallel mode will continue"));
        assert_eq!(warning.title, "Uncommitted Changes Detected");
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

    #[test]
    fn test_extract_change_id_from_worktree_name_standard() {
        // Standard worktree name with hex suffix
        let result =
            GitWorkspaceManager::extract_change_id_from_worktree_name("ws-my-change-1234abcd");
        assert_eq!(result, Some("my-change".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_with_dashes() {
        // Change ID with dashes, plus hex suffix
        let result =
            GitWorkspaceManager::extract_change_id_from_worktree_name("ws-add-user-auth-abcdef12");
        assert_eq!(result, Some("add-user-auth".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_no_suffix() {
        // Worktree name without hex suffix (fallback case)
        let result = GitWorkspaceManager::extract_change_id_from_worktree_name("ws-my-change");
        assert_eq!(result, Some("my-change".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_not_matching_prefix() {
        // Not a worktree name (doesn't start with "ws-")
        let result = GitWorkspaceManager::extract_change_id_from_worktree_name("main");
        assert_eq!(result, None);

        let result2 = GitWorkspaceManager::extract_change_id_from_worktree_name("feature-test");
        assert_eq!(result2, None);
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_short_suffix() {
        // Suffix too short to be a hex timestamp (< 7 chars)
        let result = GitWorkspaceManager::extract_change_id_from_worktree_name("ws-change-abc");
        // Falls through to the else branch, returns "change-abc"
        assert_eq!(result, Some("change-abc".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_non_hex_suffix() {
        // Non-hex characters in suffix
        let result =
            GitWorkspaceManager::extract_change_id_from_worktree_name("ws-change-notahex!");
        // Falls through because suffix contains non-hex chars
        assert_eq!(result, Some("change-notahex!".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_worktree_name_path_chars() {
        // Change ID that was sanitized from path characters
        // e.g., "feature/login" -> "feature-login"
        let result =
            GitWorkspaceManager::extract_change_id_from_worktree_name("ws-feature-login-fedcba98");
        assert_eq!(result, Some("feature-login".to_string()));
    }
}
