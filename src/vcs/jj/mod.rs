//! Jujutsu (jj) workspace management for parallel change execution.
//!
//! This module provides workspace creation, merge, and cleanup functionality
//! to enable parallel execution of changes in isolated workspaces.

pub mod commands;

use crate::config::OrchestratorConfig;
use crate::vcs::{
    VcsBackend, VcsError, VcsResult, Workspace, WorkspaceInfo, WorkspaceManager, WorkspaceStatus,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::SystemTime;
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
    #[allow(dead_code)]
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Get list of active workspaces
    #[allow(dead_code)]
    pub fn jj_workspaces(&self) -> &[JjWorkspace] {
        &self.workspaces
    }

    /// Check if jj is available and the repo is a jj repository
    #[allow(dead_code)]
    pub async fn check_jj_available(&self) -> VcsResult<bool> {
        commands::check_jj_repo(&self.repo_root).await
    }

    /// Snapshot the current working copy to ensure changes are visible in new workspaces.
    ///
    /// In jj, uncommitted changes in the working copy are not visible in new workspaces.
    /// This function creates a snapshot by running `jj new` to start a new change,
    /// which effectively commits the current state and makes it available to workspaces.
    pub async fn snapshot_working_copy(&self) -> VcsResult<()> {
        info!("Snapshotting working copy for parallel execution");

        // First, check if there are any changes to snapshot
        let status_output = Command::new("jj")
            .args(["status"])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| VcsError::jj_command(format!("Failed to check status: {}", e)))?;

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
                .map_err(|e| VcsError::jj_command(format!("Failed to create snapshot: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(VcsError::jj_command(format!(
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
    pub async fn get_current_revision(&self) -> VcsResult<String> {
        commands::get_current_revision(&self.repo_root).await
    }

    /// Create a new workspace for a change from a specific base revision
    ///
    /// If `base_revision` is provided, the workspace will be created from that revision.
    /// Otherwise, it uses the current `@` revision.
    pub async fn create_workspace_from(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> VcsResult<JjWorkspace> {
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
            std::fs::create_dir_all(&self.base_dir)?;
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
            .map_err(|e| VcsError::jj_command(format!("Failed to create workspace: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VcsError::jj_command(format!(
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
            .map_err(|e| VcsError::jj_command(format!("Failed to initialize workspace: {}", e)))?;

        if !init_output.status.success() {
            let stderr = String::from_utf8_lossy(&init_output.stderr);
            return Err(VcsError::jj_command(format!(
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
    pub async fn get_workspace_revision(&self, workspace: &JjWorkspace) -> VcsResult<String> {
        commands::get_current_revision(&workspace.path).await
    }

    /// Update workspace status
    pub fn update_jj_workspace_status(&mut self, workspace_name: &str, status: WorkspaceStatus) {
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
    pub async fn merge_jj_workspaces(&self, revisions: &[String]) -> VcsResult<String> {
        if revisions.is_empty() {
            return Err(VcsError::jj_command("No revisions to merge"));
        }

        // Always create a merge commit to integrate changes into the current workspace
        // This ensures all changes are properly integrated into git_head()

        info!("Merging {} revisions", revisions.len());
        debug!("Revisions to merge: {:?}", revisions);

        // Get current @ as base for merge
        // This ensures all individual merges use the same base (the snapshot)
        let base_rev = self.get_current_revision().await?;
        let base_short = &base_rev[..8.min(base_rev.len())];
        info!("Using base revision for merge: {}", base_short);

        // For multiple revisions, create a merge commit with `jj new --no-edit`
        // Using --no-edit prevents creating an additional empty working copy commit
        // Always include base revision first to ensure consistent merge base
        let mut args = vec!["new", "--no-edit", &base_rev];
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
            .map_err(|e| VcsError::jj_command(format!("Merge failed: {}", e)))?;

        // Parse stderr for conflict detection and commit parsing
        // Note: jj outputs "Created new commit" to stderr, not stdout
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            // Check for conflicts in failure case
            if stderr.contains("conflict") || stderr.contains("Conflict") {
                return Err(VcsError::jj_conflict(stderr.to_string()));
            }

            return Err(VcsError::jj_command(format!("Merge failed: {}", stderr)));
        }

        // IMPORTANT: jj may succeed (exit code 0) even when conflicts occur.
        // Check stderr for conflict markers on success as well.
        if stderr.contains("conflict") || stderr.contains("Conflict") {
            return Err(VcsError::jj_conflict(stderr.to_string()));
        }
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
            .map_err(|e| VcsError::jj_command(format!("Failed to edit merge commit: {}", e)))?;

        if !edit_output.status.success() {
            let stderr = String::from_utf8_lossy(&edit_output.stderr);
            return Err(VcsError::jj_command(format!(
                "Failed to edit merge commit: {}",
                stderr
            )));
        }

        Ok(merge_rev)
    }

    /// Parse the change_id from `jj new --no-edit` output
    /// Output format: "Created new commit <change_id> <commit_id> ..."
    async fn parse_created_commit_id(&self, output: &str) -> VcsResult<String> {
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
                "-T",
                "change_id ++ \"\\n\"",
                "-l",
                "1",
            ])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| VcsError::jj_command(format!("Failed to get merge revision: {}", e)))?;

        if !log_output.status.success() {
            return Err(VcsError::jj_command("Failed to get merge revision"));
        }

        let rev = String::from_utf8_lossy(&log_output.stdout)
            .trim()
            .to_string();
        if rev.is_empty() {
            return Err(VcsError::jj_command("Could not determine merge revision"));
        }

        Ok(rev)
    }

    /// Cleanup a single workspace (forget + delete directory)
    pub async fn cleanup_jj_workspace(&mut self, workspace_name: &str) -> VcsResult<()> {
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
        self.update_jj_workspace_status(workspace_name, WorkspaceStatus::Cleaned);

        debug!("Workspace '{}' cleaned up", workspace_name);
        Ok(())
    }

    /// Cleanup all workspaces
    pub async fn cleanup_all_jj_workspaces(&mut self) -> VcsResult<()> {
        let workspace_names: Vec<String> = self.workspaces.iter().map(|w| w.name.clone()).collect();

        for name in workspace_names {
            let _ = self.cleanup_jj_workspace(&name).await;
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
    pub async fn list_jj_workspaces(&self) -> VcsResult<Vec<String>> {
        let output = commands::run_jj(&["workspace", "list"], &self.repo_root).await?;
        let workspaces: Vec<String> = output
            .lines()
            .filter_map(|line| {
                // Parse workspace list output (format: "name: path")
                line.split(':').next().map(|s| s.trim().to_string())
            })
            .collect();

        Ok(workspaces)
    }

    /// Extract the change_id from a workspace name.
    ///
    /// Workspace names are in format: "ws-{sanitized_change_id}-{unique_suffix}"
    /// This function extracts the change_id portion.
    fn extract_change_id_from_workspace_name(workspace_name: &str) -> Option<String> {
        // Expected format: ws-{change_id}-{hex_suffix}
        if !workspace_name.starts_with("ws-") {
            return None;
        }

        let without_prefix = &workspace_name[3..]; // Remove "ws-"

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

        // Fallback: return everything after "ws-" (workspace might not have suffix)
        Some(without_prefix.to_string())
    }

    /// Find all existing workspaces for the given change_id.
    ///
    /// Returns workspace info for all matching workspaces, sorted by last_modified (newest first).
    async fn find_all_workspaces_for_change(
        &self,
        change_id: &str,
    ) -> VcsResult<Vec<WorkspaceInfo>> {
        let output = commands::run_jj(&["workspace", "list"], &self.repo_root).await?;
        let sanitized_change_id = change_id.replace(['/', '\\', ' '], "-");

        let mut candidates = Vec::new();

        for line in output.lines() {
            // Parse workspace list output (format: "name: path")
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }

            let workspace_name = parts[0].trim();
            let workspace_path_str = parts[1].trim();

            // Check if this workspace matches our change_id
            if let Some(extracted_change_id) =
                Self::extract_change_id_from_workspace_name(workspace_name)
            {
                if extracted_change_id == sanitized_change_id {
                    let workspace_path = PathBuf::from(workspace_path_str);

                    // Only include workspaces whose directory actually exists
                    if !workspace_path.exists() {
                        debug!(
                            "Skipping workspace '{}' - directory does not exist: {:?}",
                            workspace_name, workspace_path
                        );
                        continue;
                    }

                    // Get last modified time
                    let last_modified = workspace_path
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);

                    candidates.push(WorkspaceInfo {
                        path: workspace_path,
                        change_id: change_id.to_string(),
                        workspace_name: workspace_name.to_string(),
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
impl WorkspaceManager for JjWorkspaceManager {
    fn backend_type(&self) -> VcsBackend {
        VcsBackend::Jj
    }

    async fn check_available(&self) -> VcsResult<bool> {
        self.check_jj_available().await
    }

    async fn prepare_for_parallel(&self) -> VcsResult<()> {
        // jj snapshots working copy changes automatically
        self.snapshot_working_copy().await
    }

    async fn get_current_revision(&self) -> VcsResult<String> {
        JjWorkspaceManager::get_current_revision(self).await
    }

    async fn create_workspace(
        &mut self,
        change_id: &str,
        base_revision: Option<&str>,
    ) -> VcsResult<Workspace> {
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
        self.update_jj_workspace_status(workspace_name, status);
    }

    async fn merge_workspaces(&self, revisions: &[String]) -> VcsResult<String> {
        self.merge_jj_workspaces(revisions).await
    }

    async fn cleanup_workspace(&mut self, workspace_name: &str) -> VcsResult<()> {
        self.cleanup_jj_workspace(workspace_name).await
    }

    async fn cleanup_all(&mut self) -> VcsResult<()> {
        self.cleanup_all_jj_workspaces().await
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

    async fn snapshot_working_copy(&self, workspace_path: &Path) -> VcsResult<()> {
        // jj snapshots working copy changes when running status
        let _ = tokio::process::Command::new("jj")
            .arg("status")
            .current_dir(workspace_path)
            .output()
            .await;
        Ok(())
    }

    async fn set_commit_message(&self, workspace_path: &Path, message: &str) -> VcsResult<()> {
        let output = tokio::process::Command::new("jj")
            .args(["describe", "-m", message])
            .current_dir(workspace_path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| VcsError::jj_command(format!("Failed to describe: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set commit message: {}", stderr);
        }
        Ok(())
    }

    async fn get_revision_in_workspace(&self, workspace_path: &Path) -> VcsResult<String> {
        let output = tokio::process::Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "change_id"])
            .current_dir(workspace_path)
            .output()
            .await
            .map_err(|e| VcsError::jj_command(format!("Failed to get revision: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VcsError::jj_command(format!(
                "Failed to get workspace revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn get_status(&self) -> VcsResult<String> {
        commands::get_status(&self.repo_root).await
    }

    async fn get_log_for_revisions(&self, revisions: &[String]) -> VcsResult<String> {
        commands::get_log_for_revisions(revisions, &self.repo_root).await
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
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

    async fn find_existing_workspace(
        &mut self,
        change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        let mut candidates = self.find_all_workspaces_for_change(change_id).await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        // Take the newest workspace (first in sorted list)
        let newest = candidates.remove(0);

        // Clean up older workspaces
        for old_ws in candidates {
            info!(
                "Cleaning up older workspace '{}' for change '{}'",
                old_ws.workspace_name, change_id
            );

            // Forget the workspace in jj
            let forget_result = Command::new("jj")
                .args(["workspace", "forget", &old_ws.workspace_name])
                .current_dir(&self.repo_root)
                .stdin(Stdio::null())
                .output()
                .await;

            if let Err(e) = forget_result {
                warn!(
                    "Failed to forget workspace '{}': {}",
                    old_ws.workspace_name, e
                );
            }

            // Remove the directory if it exists
            if old_ws.path.exists() {
                if let Err(e) = std::fs::remove_dir_all(&old_ws.path) {
                    warn!(
                        "Failed to remove workspace directory {:?}: {}",
                        old_ws.path, e
                    );
                }
            }
        }

        debug!(
            "Found existing workspace '{}' for change '{}' (last modified: {:?})",
            newest.workspace_name, change_id, newest.last_modified
        );

        Ok(Some(newest))
    }

    async fn reuse_workspace(&mut self, workspace_info: &WorkspaceInfo) -> VcsResult<Workspace> {
        info!(
            "Reusing existing workspace '{}' at {:?}",
            workspace_info.workspace_name, workspace_info.path
        );

        // Verify the workspace directory exists
        if !workspace_info.path.exists() {
            return Err(VcsError::jj_command(format!(
                "Workspace directory does not exist: {:?}",
                workspace_info.path
            )));
        }

        // Get the current revision in the workspace
        let base_revision = commands::get_current_revision(&workspace_info.path)
            .await
            .unwrap_or_else(|_| "unknown".to_string());

        let workspace = JjWorkspace {
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
        assert!(manager.jj_workspaces().is_empty());
    }

    #[test]
    fn test_workspace_name_sanitization() {
        // Test that change IDs with special characters are sanitized
        let change_id = "feature/add-login";
        let sanitized = format!("ws-{}", change_id.replace(['/', '\\', ' '], "-"));
        assert_eq!(sanitized, "ws-feature-add-login");
    }

    #[test]
    fn test_conflict_detection_in_stderr() {
        // Test that conflict detection logic correctly identifies conflicts in stderr
        // This verifies the string matching used in merge_jj_workspaces

        // Test cases that should be detected as conflicts
        // Note: jj outputs "conflict" (lowercase) or "Conflict" (capitalized), not "CONFLICT"
        let conflict_cases = vec![
            "New conflicts in: src/main.rs",
            "Conflict detected in file.txt",
            "conflict markers found",
            "There are unresolved conflicts at these paths:",
        ];

        for case in conflict_cases {
            assert!(
                case.contains("conflict") || case.contains("Conflict"),
                "Should detect conflict in: {}",
                case
            );
        }

        // Test cases that should NOT be detected as conflicts
        let non_conflict_cases = vec![
            "Created new commit abc123 def456",
            "Successfully merged",
            "Working copy now at: xyz",
        ];

        for case in non_conflict_cases {
            assert!(
                !case.contains("conflict") && !case.contains("Conflict"),
                "Should not detect conflict in: {}",
                case
            );
        }
    }

    #[test]
    fn test_jj_conflict_error_creation() {
        // Verify VcsError::jj_conflict creates the correct error type
        let err = VcsError::jj_conflict("New conflicts in: src/main.rs");
        assert!(matches!(
            err,
            VcsError::Conflict {
                backend: VcsBackend::Jj,
                ..
            }
        ));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_standard() {
        // Standard workspace name with hex suffix
        let result =
            JjWorkspaceManager::extract_change_id_from_workspace_name("ws-my-change-1234abcd");
        assert_eq!(result, Some("my-change".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_with_dashes() {
        // Change ID with dashes, plus hex suffix
        let result =
            JjWorkspaceManager::extract_change_id_from_workspace_name("ws-add-user-auth-abcdef12");
        assert_eq!(result, Some("add-user-auth".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_no_suffix() {
        // Workspace name without hex suffix (fallback case)
        let result = JjWorkspaceManager::extract_change_id_from_workspace_name("ws-my-change");
        assert_eq!(result, Some("my-change".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_not_matching_prefix() {
        // Not a workspace name (doesn't start with "ws-")
        let result = JjWorkspaceManager::extract_change_id_from_workspace_name("default");
        assert_eq!(result, None);

        let result2 = JjWorkspaceManager::extract_change_id_from_workspace_name("workspace-test");
        assert_eq!(result2, None);
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_short_suffix() {
        // Suffix too short to be a hex timestamp (< 7 chars)
        let result = JjWorkspaceManager::extract_change_id_from_workspace_name("ws-change-abc");
        // Falls through to the else branch, returns "change-abc"
        assert_eq!(result, Some("change-abc".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_non_hex_suffix() {
        // Non-hex characters in suffix
        let result =
            JjWorkspaceManager::extract_change_id_from_workspace_name("ws-change-notahex!");
        // Falls through because suffix contains non-hex chars
        assert_eq!(result, Some("change-notahex!".to_string()));
    }

    #[test]
    fn test_extract_change_id_from_workspace_name_path_chars() {
        // Change ID that was sanitized from path characters
        // e.g., "feature/login" -> "feature-login"
        let result =
            JjWorkspaceManager::extract_change_id_from_workspace_name("ws-feature-login-fedcba98");
        assert_eq!(result, Some("feature-login".to_string()));
    }
}
