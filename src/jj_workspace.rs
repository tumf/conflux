//! jj (Jujutsu) workspace management for parallel change execution.
//!
//! This module provides workspace creation, merge, and cleanup functionality
//! to enable parallel execution of changes in isolated workspaces.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::jj_commands;
use crate::vcs_backend::{VcsBackend, Workspace, WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Represents a jj workspace for parallel execution
#[derive(Debug, Clone)]
pub struct JjWorkspace {
    /// Workspace name (used by jj)
    pub name: String,
    /// Path to workspace directory
    pub path: PathBuf,
    /// Associated OpenSpec change ID
    pub change_id: String,
    /// Base revision workspace was created from (used for debugging/logging)
    #[allow(dead_code)]
    pub base_revision: String,
    /// Current status (managed via update_workspace_status)
    pub status: WorkspaceStatus,
}

/// Manages jj workspaces for parallel execution
pub struct JjWorkspaceManager {
    /// Base directory for creating workspaces
    base_dir: PathBuf,
    /// Repository root directory
    repo_root: PathBuf,
    /// Active workspaces
    workspaces: Vec<JjWorkspace>,
    /// Maximum concurrent workspaces
    max_concurrent: usize,
}

impl JjWorkspaceManager {
    /// Create a new workspace manager
    ///
    /// Note: The `_config` parameter is kept for API compatibility but is not used.
    /// Conflict resolution is handled by ParallelExecutor using AgentRunner.
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
        }
    }

    /// Get the maximum concurrent workspaces limit
    #[allow(dead_code)] // Part of workspace manager pattern
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Get list of active workspaces
    #[allow(dead_code)]
    pub fn workspaces(&self) -> &[JjWorkspace] {
        &self.workspaces
    }

    /// Check if jj is available and the repo is a jj repository
    #[allow(dead_code)] // Part of WorkspaceManager trait implementation
    pub async fn check_jj_available(&self) -> Result<bool> {
        jj_commands::check_jj_repo(&self.repo_root).await
    }

    /// Snapshot the current working copy to ensure changes are visible in new workspaces.
    ///
    /// In jj, uncommitted changes in the working copy are not visible in new workspaces.
    /// This function creates a snapshot by running `jj new` to start a new change,
    /// which effectively commits the current state and makes it available to workspaces.
    pub async fn snapshot_working_copy(&self) -> Result<()> {
        info!("Snapshotting working copy for parallel execution");

        // First, check if there are any changes to snapshot
        let status_output = Command::new("jj")
            .args(["status"])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to check status: {}", e)))?;

        let status_str = String::from_utf8_lossy(&status_output.stdout);

        // If there are working copy changes, create a new change
        if status_str.contains("Working copy changes:") {
            debug!("Found working copy changes, creating snapshot");

            // Use `jj new` to create a new change, which snapshots the current state
            let output = Command::new("jj")
                .args([
                    "new",
                    "-m",
                    "Parallel execution snapshot (auto-created by openspec-orchestrator)",
                ])
                .current_dir(&self.repo_root)
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to create snapshot: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OrchestratorError::JjCommand(format!(
                    "Failed to create snapshot: {}",
                    stderr
                )));
            }

            info!("Working copy snapshot created successfully");
        } else {
            debug!("No working copy changes to snapshot");
        }

        Ok(())
    }

    /// Get the current jj revision
    pub async fn get_current_revision(&self) -> Result<String> {
        jj_commands::get_current_revision(&self.repo_root).await
    }

    /// Create a new workspace for a change from a specific base revision
    ///
    /// If `base_revision` is provided, the workspace will be created from that revision.
    /// Otherwise, it uses the current `@` revision.
    pub async fn create_workspace_from(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> Result<JjWorkspace> {
        // Sanitize change_id and add unique suffix for workspace name
        let unique_suffix = format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u32)
                .unwrap_or(0)
        );
        let workspace_name = format!(
            "ws-{}-{}",
            change_id.replace(['/', '\\', ' '], "-"),
            unique_suffix
        );
        let workspace_path = self.base_dir.join(&workspace_name);

        // Ensure base directory exists
        if !self.base_dir.exists() {
            std::fs::create_dir_all(&self.base_dir).map_err(OrchestratorError::Io)?;
        }

        // Check if workspace already exists and clean it up
        if workspace_path.exists() {
            info!(
                "Workspace '{}' already exists, cleaning up...",
                workspace_name
            );

            // Forget the workspace in jj (ignore errors if it doesn't exist in jj)
            let _ = Command::new("jj")
                .args(["workspace", "forget", &workspace_name])
                .current_dir(&self.repo_root)
                .stdin(Stdio::null())
                .output()
                .await;

            // Remove the directory
            if let Err(e) = std::fs::remove_dir_all(&workspace_path) {
                warn!(
                    "Failed to remove existing workspace directory {:?}: {}",
                    workspace_path, e
                );
            }
        }

        // Get current revision before creating workspace
        let base_rev = self.get_current_revision().await?;

        info!(
            "Creating workspace '{}' at {:?}",
            workspace_name, workspace_path
        );

        // Create workspace: jj workspace add <path> --name <name> -r <revision>
        // Using explicit revision ensures all workspaces in a group share the same base
        let revision = base_revision.unwrap_or("@");
        info!(
            "Creating workspace '{}' from revision '{}'",
            workspace_name,
            &revision[..8.min(revision.len())]
        );

        let output = Command::new("jj")
            .args([
                "workspace",
                "add",
                workspace_path.to_str().unwrap(),
                "--name",
                &workspace_name,
                "-r",
                revision,
            ])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to create workspace: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Workspace creation failed: {}",
                stderr
            )));
        }

        // Initialize working copy in the new workspace
        // This ensures the workspace has a working-copy commit
        let init_output = Command::new("jj")
            .arg("status")
            .current_dir(&workspace_path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to initialize workspace: {}", e))
            })?;

        if !init_output.status.success() {
            let stderr = String::from_utf8_lossy(&init_output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Workspace initialization failed: {}",
                stderr
            )));
        }

        let workspace = JjWorkspace {
            name: workspace_name,
            path: workspace_path,
            change_id: change_id.to_string(),
            base_revision: base_rev,
            status: WorkspaceStatus::Created,
        };

        self.workspaces.push(workspace.clone());
        debug!("Created workspace: {:?}", workspace.name);

        Ok(workspace)
    }

    /// Get the current revision in a workspace
    #[allow(dead_code)]
    pub async fn get_workspace_revision(&self, workspace: &JjWorkspace) -> Result<String> {
        jj_commands::get_current_revision(&workspace.path).await
    }

    /// Update workspace status
    pub fn update_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus) {
        if let Some(ws) = self
            .workspaces
            .iter_mut()
            .find(|w| w.name == workspace_name)
        {
            ws.status = status;
        }
    }

    /// Merge multiple workspace revisions into main
    ///
    /// For single revision: Uses `jj edit` to switch to that revision without creating empty commits.
    /// For multiple revisions: Uses `jj new --no-edit` to create a merge commit without creating
    /// an additional empty working copy commit, then uses `jj edit` to switch to the merge commit.
    pub async fn merge_workspaces(&self, revisions: &[String]) -> Result<String> {
        if revisions.is_empty() {
            return Err(OrchestratorError::JjCommand(
                "No revisions to merge".to_string(),
            ));
        }

        if revisions.len() == 1 {
            // Single revision - use `jj edit` to switch to the revision directly
            // This avoids creating empty commits unlike `jj new`
            let rev_short = &revisions[0][..8.min(revisions[0].len())];
            info!("Single revision, editing directly: {}", rev_short);

            let output = Command::new("jj")
                .args(["edit", &revisions[0]])
                .current_dir(&self.repo_root)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to edit revision: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OrchestratorError::JjCommand(format!(
                    "Failed to edit revision: {}",
                    stderr
                )));
            }

            // Return the revision we just switched to
            return Ok(revisions[0].clone());
        }

        info!("Merging {} revisions", revisions.len());
        debug!("Revisions to merge: {:?}", revisions);

        // For multiple revisions, create a merge commit with `jj new --no-edit`
        // Using --no-edit prevents creating an additional empty working copy commit
        let mut args = vec!["new", "--no-edit"];
        for rev in revisions {
            args.push(rev.as_str());
        }
        args.extend(&["-m", "Merge parallel changes"]);

        let output = Command::new("jj")
            .args(&args)
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Merge failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check for conflicts
            if stderr.contains("conflict") || stderr.contains("Conflict") {
                return Err(OrchestratorError::JjConflict(stderr.to_string()));
            }

            return Err(OrchestratorError::JjCommand(format!(
                "Merge failed: {}",
                stderr
            )));
        }

        // Parse the merge commit revision from the output
        // Note: jj outputs "Created new commit" to stderr, not stdout
        // Output format: "Created new commit <change_id> <commit_id> ..."
        let stderr = String::from_utf8_lossy(&output.stderr);
        let merge_rev = self.parse_created_commit_id(&stderr).await?;

        // Switch to the merge commit using `jj edit`
        let edit_output = Command::new("jj")
            .args(["edit", &merge_rev])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to edit merge commit: {}", e))
            })?;

        if !edit_output.status.success() {
            let stderr = String::from_utf8_lossy(&edit_output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to edit merge commit: {}",
                stderr
            )));
        }

        Ok(merge_rev)
    }

    /// Parse the change_id from `jj new --no-edit` output
    /// Output format: "Created new commit <change_id> <commit_id> ..."
    async fn parse_created_commit_id(&self, output: &str) -> Result<String> {
        // Try to parse from output first
        for line in output.lines() {
            if line.contains("Created new commit") {
                // Format: "Created new commit <change_id> <commit_id> ..."
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    return Ok(parts[3].to_string());
                }
            }
        }

        // Fallback: get the latest commit that matches merge pattern
        // This is less reliable but works as a fallback
        debug!("Could not parse commit id from output, using fallback query");
        let log_output = Command::new("jj")
            .args([
                "log",
                "-r",
                "heads(all())",
                "--no-graph",
                "--ignore-working-copy",
                "-T",
                "change_id ++ \"\\n\"",
                "-l",
                "1",
            ])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to get merge revision: {}", e))
            })?;

        if !log_output.status.success() {
            return Err(OrchestratorError::JjCommand(
                "Failed to get merge revision".to_string(),
            ));
        }

        let rev = String::from_utf8_lossy(&log_output.stdout)
            .trim()
            .to_string();
        if rev.is_empty() {
            return Err(OrchestratorError::JjCommand(
                "Could not determine merge revision".to_string(),
            ));
        }

        Ok(rev)
    }

    /// Cleanup a single workspace (forget + delete directory)
    pub async fn cleanup_workspace(&mut self, workspace_name: &str) -> Result<()> {
        let workspace = self
            .workspaces
            .iter()
            .find(|w| w.name == workspace_name)
            .cloned();

        let Some(workspace) = workspace else {
            warn!("Workspace '{}' not found for cleanup", workspace_name);
            return Ok(());
        };

        info!("Cleaning up workspace '{}'", workspace_name);

        // Forget workspace: jj workspace forget <name>
        let forget_result = Command::new("jj")
            .args(["workspace", "forget", workspace_name])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await;

        if let Err(e) = forget_result {
            warn!("Failed to forget workspace '{}': {}", workspace_name, e);
        }

        // Delete directory
        if workspace.path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&workspace.path) {
                warn!(
                    "Failed to delete workspace directory {:?}: {}",
                    workspace.path, e
                );
            }
        }

        // Update status
        self.update_workspace_status(workspace_name, WorkspaceStatus::Cleaned);

        debug!("Workspace '{}' cleaned up", workspace_name);
        Ok(())
    }

    /// Cleanup all workspaces
    pub async fn cleanup_all(&mut self) -> Result<()> {
        let workspace_names: Vec<String> = self.workspaces.iter().map(|w| w.name.clone()).collect();

        for name in workspace_names {
            let _ = self.cleanup_workspace(&name).await;
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

    /// List all jj workspaces in the repository
    #[allow(dead_code)]
    pub async fn list_jj_workspaces(&self) -> Result<Vec<String>> {
        let output = jj_commands::run_jj(&["workspace", "list"], &self.repo_root).await?;
        let workspaces: Vec<String> = output
            .lines()
            .filter_map(|line| {
                // Parse workspace list output (format: "name: path")
                line.split(':').next().map(|s| s.trim().to_string())
            })
            .collect();

        Ok(workspaces)
    }
}

#[async_trait]
impl WorkspaceManager for JjWorkspaceManager {
    fn backend_type(&self) -> VcsBackend {
        VcsBackend::Jj
    }

    async fn check_available(&self) -> Result<bool> {
        self.check_jj_available().await
    }

    async fn prepare_for_parallel(&self) -> Result<()> {
        // jj snapshots working copy changes automatically
        self.snapshot_working_copy().await
    }

    async fn get_current_revision(&self) -> Result<String> {
        JjWorkspaceManager::get_current_revision(self).await
    }

    async fn create_workspace(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> Result<Workspace> {
        let jj_ws = self.create_workspace_from(change_id, base_revision).await?;
        Ok(Workspace {
            name: jj_ws.name,
            path: jj_ws.path,
            change_id: jj_ws.change_id,
            base_revision: jj_ws.base_revision,
            status: jj_ws.status,
        })
    }

    fn update_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus) {
        JjWorkspaceManager::update_workspace_status(self, workspace_name, status);
    }

    async fn merge_workspaces(&self, revisions: &[String]) -> Result<String> {
        JjWorkspaceManager::merge_workspaces(self, revisions).await
    }

    async fn cleanup_workspace(&mut self, workspace_name: &str) -> Result<()> {
        JjWorkspaceManager::cleanup_workspace(self, workspace_name).await
    }

    async fn cleanup_all(&mut self) -> Result<()> {
        JjWorkspaceManager::cleanup_all(self).await
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
        "This project uses jj (Jujutsu) for version control, not git.\n\n\
         A merge conflict occurred. The files contain jj conflict markers.\n\
         After editing the conflicting files to resolve conflicts, \
         jj will automatically detect the resolution."
    }

    async fn snapshot_working_copy(&self, workspace_path: &Path) -> Result<()> {
        // jj snapshots working copy changes when running status
        let _ = tokio::process::Command::new("jj")
            .arg("status")
            .current_dir(workspace_path)
            .output()
            .await;
        Ok(())
    }

    async fn set_commit_message(&self, workspace_path: &Path, message: &str) -> Result<()> {
        use std::process::Stdio;
        let output = tokio::process::Command::new("jj")
            .args(["describe", "-m", message])
            .current_dir(workspace_path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to describe: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set commit message: {}", stderr);
        }
        Ok(())
    }

    async fn get_revision_in_workspace(&self, workspace_path: &Path) -> Result<String> {
        let output = tokio::process::Command::new("jj")
            .args([
                "log",
                "-r",
                "@",
                "--no-graph",
                "--ignore-working-copy",
                "-T",
                "change_id",
            ])
            .current_dir(workspace_path)
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to get revision: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get workspace revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_status(&self) -> Result<String> {
        jj_commands::get_status(&self.repo_root).await
    }

    async fn get_log_for_revisions(&self, revisions: &[String]) -> Result<String> {
        jj_commands::get_log_for_revisions(revisions, &self.repo_root).await
    }

    async fn detect_conflicts(&self) -> Result<Vec<String>> {
        let stdout = self.get_status().await?;
        let mut conflict_files = Vec::new();

        for line in stdout.lines() {
            // jj status shows conflicts with "C " prefix or "Conflict" marker
            if line.contains("Conflict") || line.starts_with("C ") {
                // Extract filename
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(filename) = parts.last() {
                    conflict_files.push(filename.to_string());
                }
            }
        }

        Ok(conflict_files)
    }

    fn forget_workspace_sync(&self, workspace_name: &str) {
        debug!(
            "Emergency cleanup: forgetting jj workspace '{}'",
            workspace_name
        );
        let result = std::process::Command::new("jj")
            .args(["workspace", "forget", workspace_name])
            .current_dir(&self.repo_root)
            .output();

        match result {
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug!(
                    "Failed to forget workspace '{}': {}",
                    workspace_name, stderr
                );
            }
            Err(e) => {
                debug!("Failed to run jj workspace forget: {}", e);
            }
            _ => {
                debug!("Successfully forgot workspace '{}'", workspace_name);
            }
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

    fn create_test_manager() -> (JjWorkspaceManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().join("workspaces");
        let repo_root = temp_dir.path().to_path_buf();
        let config = OrchestratorConfig::default();

        let manager = JjWorkspaceManager::new(base_dir, repo_root, 3, config);
        (manager, temp_dir)
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
    fn test_manager_creation() {
        let (manager, _temp) = create_test_manager();
        assert_eq!(manager.max_concurrent(), 3);
        assert!(manager.workspaces().is_empty());
    }

    #[test]
    fn test_workspace_name_sanitization() {
        // Test that change IDs with special characters are sanitized
        let change_id = "feature/add-login";
        let sanitized = format!("ws-{}", change_id.replace(['/', '\\', ' '], "-"));
        assert_eq!(sanitized, "ws-feature-add-login");
    }
}
