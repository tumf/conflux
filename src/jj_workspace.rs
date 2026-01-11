//! jj (Jujutsu) workspace management for parallel change execution.
//!
//! This module provides workspace creation, merge, and cleanup functionality
//! to enable parallel execution of changes in isolated workspaces.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Status of a jj workspace
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
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

/// Represents a jj workspace for parallel execution
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct JjWorkspace {
    /// Workspace name (used by jj)
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

/// Result of a workspace execution
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// OpenSpec change ID
    pub change_id: String,
    /// Workspace name
    pub workspace_name: String,
    /// Final revision if successful
    pub final_revision: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Result of parallel execution
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParallelExecutionResult {
    /// Successfully completed workspaces
    pub successful: Vec<WorkspaceResult>,
    /// Failed workspaces
    pub failed: Vec<WorkspaceResult>,
    /// Merge revision if merge was performed
    pub merge_revision: Option<String>,
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
    /// Configuration for resolve command
    #[allow(dead_code)]
    config: OrchestratorConfig,
}

impl JjWorkspaceManager {
    /// Create a new workspace manager
    pub fn new(
        base_dir: PathBuf,
        repo_root: PathBuf,
        max_concurrent: usize,
        config: OrchestratorConfig,
    ) -> Self {
        Self {
            base_dir,
            repo_root,
            workspaces: Vec::new(),
            max_concurrent,
            config,
        }
    }

    /// Get the maximum concurrent workspaces limit
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Get list of active workspaces
    #[allow(dead_code)]
    pub fn workspaces(&self) -> &[JjWorkspace] {
        &self.workspaces
    }

    /// Check if jj is available and the repo is a jj repository
    pub async fn check_jj_available(&self) -> Result<bool> {
        let output = Command::new("jj")
            .arg("--version")
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                // Check if current directory is a jj repo
                let root_output = Command::new("jj")
                    .arg("root")
                    .current_dir(&self.repo_root)
                    .stdin(Stdio::null())
                    .output()
                    .await;

                match root_output {
                    Ok(out) if out.status.success() => Ok(true),
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
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
        let output = Command::new("jj")
            .args([
                "log",
                "-r",
                "@",
                "--no-graph",
                "--ignore-working-copy",
                "-T",
                "change_id",
            ])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to get revision: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Create a new workspace for a change
    pub async fn create_workspace(&mut self, change_id: &str) -> Result<JjWorkspace> {
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

        // Create workspace: jj workspace add <path> --name <name>
        let output = Command::new("jj")
            .args([
                "workspace",
                "add",
                workspace_path.to_str().unwrap(),
                "--name",
                &workspace_name,
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
        let output = Command::new("jj")
            .args([
                "log",
                "-r",
                "@",
                "--no-graph",
                "--ignore-working-copy",
                "-T",
                "change_id",
            ])
            .current_dir(&workspace.path)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to get workspace revision: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get workspace revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
    /// Creates a merge commit with all successful workspace revisions as parents.
    /// For single revision, updates the working copy to point to that revision.
    pub async fn merge_workspaces(&self, revisions: &[String]) -> Result<String> {
        if revisions.is_empty() {
            return Err(OrchestratorError::JjCommand(
                "No revisions to merge".to_string(),
            ));
        }

        if revisions.len() == 1 {
            // Single revision - update working copy to this revision
            // This ensures subsequent groups can access this revision's changes
            info!(
                "Single revision, updating working copy to: {}",
                &revisions[0][..8.min(revisions[0].len())]
            );
            let output = Command::new("jj")
                .args(["new", &revisions[0], "-m", "Continue from workspace change"])
                .current_dir(&self.repo_root)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to update to revision: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OrchestratorError::JjCommand(format!(
                    "Failed to update to revision: {}",
                    stderr
                )));
            }

            // Return the new current revision (which is a child of the workspace revision)
            return self.get_current_revision().await;
        }

        info!("Merging {} revisions", revisions.len());
        debug!("Revisions to merge: {:?}", revisions);

        // Create merge commit: jj new <rev1> <rev2> ... -m "Merge parallel changes"
        let mut args = vec!["new"];
        for rev in revisions {
            args.push(rev.as_str());
        }
        args.extend(&["-m", "Merge parallel changes from openspec-orchestrator"]);

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

        // Get the new merge revision
        self.get_current_revision().await
    }

    /// Resolve conflicts using the configured resolve command
    #[allow(dead_code)]
    pub async fn resolve_conflicts(&self, conflict_info: &str) -> Result<()> {
        let resolve_command = match &self.config.resolve_command {
            Some(cmd) => cmd.clone(),
            None => {
                return Err(OrchestratorError::JjConflict(
                    "No resolve_command configured".to_string(),
                ))
            }
        };

        let command = resolve_command.replace("{conflict_info}", conflict_info);
        info!("Running resolve command: {}", command);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Resolve command failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Resolve command failed: {}",
                stderr
            )));
        }

        Ok(())
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
        let output = Command::new("jj")
            .args(["workspace", "list"])
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::JjCommand(format!("Failed to list workspaces: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to list workspaces: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let workspaces: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                // Parse workspace list output (format: "name: path")
                line.split(':').next().map(|s| s.trim().to_string())
            })
            .collect();

        Ok(workspaces)
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
