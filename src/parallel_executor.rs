//! Parallel execution coordinator for jj workspace-based parallel change application.
//!
//! This module manages the parallel execution of changes using jj workspaces,
//! including workspace creation, apply command execution, merge, and cleanup.

use crate::analyzer::ParallelGroup;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::jj_workspace::{JjWorkspace, JjWorkspaceManager, WorkspaceResult, WorkspaceStatus};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

/// Events emitted during parallel execution
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ParallelEvent {
    /// A workspace was created
    WorkspaceCreated {
        change_id: String,
        workspace: String,
    },
    /// Apply started in a workspace
    ApplyStarted { change_id: String },
    /// Apply output (summary of command output)
    ApplyOutput { change_id: String, output: String },
    /// Apply completed in a workspace
    ApplyCompleted { change_id: String, revision: String },
    /// Apply failed in a workspace
    ApplyFailed { change_id: String, error: String },
    /// Change archived successfully
    ChangeArchived { change_id: String },
    /// Change archive failed
    ArchiveFailed { change_id: String, error: String },
    /// Merge started
    MergeStarted { revisions: Vec<String> },
    /// Merge completed
    MergeCompleted { revision: String },
    /// Merge resulted in conflicts
    MergeConflict { files: Vec<String> },
    /// Conflict resolution started
    ConflictResolutionStarted,
    /// Conflict resolution completed
    ConflictResolutionCompleted,
    /// Conflict resolution failed
    ConflictResolutionFailed { error: String },
    /// Workspace cleanup started
    CleanupStarted { workspace: String },
    /// Workspace cleanup completed
    CleanupCompleted { workspace: String },
    /// Group execution started
    GroupStarted { group_id: u32, changes: Vec<String> },
    /// Group execution completed
    GroupCompleted { group_id: u32 },
    /// All groups completed
    AllCompleted,
    /// Error during parallel execution
    Error { message: String },
}

/// Parallel executor for running changes in jj workspaces
#[allow(dead_code)]
pub struct ParallelExecutor {
    /// Workspace manager
    workspace_manager: JjWorkspaceManager,
    /// Configuration (kept for potential future use)
    config: OrchestratorConfig,
    /// Apply command template
    apply_command: String,
    /// Archive command template
    archive_command: String,
    /// Resolve command template
    resolve_command: String,
    /// Event sender
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    /// Maximum retries for conflict resolution
    max_conflict_retries: u32,
    /// Repository root path for archive operations
    repo_root: PathBuf,
}

impl ParallelExecutor {
    /// Create a new parallel executor
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
    ) -> Self {
        // Create a unique temp directory for this execution
        let base_dir = if let Some(configured_dir) = config.get_workspace_base_dir() {
            // User configured a specific directory
            PathBuf::from(configured_dir)
        } else {
            // Use tempfile to create a unique temp directory
            match tempfile::Builder::new().prefix("openspec-ws-").tempdir() {
                Ok(temp_dir) => {
                    // Keep the path but leak the TempDir so it doesn't get cleaned up immediately
                    let path = temp_dir.path().to_path_buf();
                    std::mem::forget(temp_dir);
                    path
                }
                Err(e) => {
                    error!("Failed to create temp directory: {}", e);
                    // Fallback to a fixed temp directory
                    std::env::temp_dir().join("openspec-workspaces-fallback")
                }
            }
        };
        info!("Using workspace base directory: {:?}", base_dir);

        let max_concurrent = config.get_max_concurrent_workspaces();
        let apply_command = config.get_apply_command().to_string();
        let archive_command = config.get_archive_command().to_string();
        let resolve_command = config.get_resolve_command().to_string();

        let workspace_manager =
            JjWorkspaceManager::new(base_dir, repo_root.clone(), max_concurrent, config.clone());

        Self {
            workspace_manager,
            config,
            apply_command,
            archive_command,
            resolve_command,
            event_tx,
            max_conflict_retries: 3,
            repo_root,
        }
    }

    /// Check if jj is available for parallel execution
    pub async fn check_jj_available(&self) -> Result<bool> {
        self.workspace_manager.check_jj_available().await
    }

    /// Execute groups in topological order
    pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
        if groups.is_empty() {
            self.send_event(ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!("Executing {} groups in parallel mode", groups.len());

        // Snapshot working copy to ensure all changes are visible in workspaces
        // In jj, uncommitted changes are not visible in new workspaces
        info!("Snapshotting working copy for parallel execution...");
        if let Err(e) = self.workspace_manager.snapshot_working_copy().await {
            let error_msg = format!("Failed to snapshot working copy: {}", e);
            error!("{}", error_msg);
            self.send_event(ParallelEvent::Error { message: error_msg })
                .await;
            return Err(e);
        }
        info!("Working copy snapshot complete");

        for group in groups {
            self.execute_group(&group).await?;
        }

        self.send_event(ParallelEvent::AllCompleted).await;
        Ok(())
    }

    /// Execute a single group of changes
    async fn execute_group(&mut self, group: &ParallelGroup) -> Result<()> {
        // Get current base revision for this group's workspaces
        let base_revision = self.workspace_manager.get_current_revision().await?;
        info!(
            "Executing group {} with {} changes: {:?} (base revision: {})",
            group.id,
            group.changes.len(),
            group.changes,
            &base_revision[..8.min(base_revision.len())]
        );

        self.send_event(ParallelEvent::GroupStarted {
            group_id: group.id,
            changes: group.changes.clone(),
        })
        .await;

        // Create workspaces for all changes in the group (from current base revision)
        let mut workspaces: Vec<JjWorkspace> = Vec::new();
        for change_id in &group.changes {
            match self.workspace_manager.create_workspace(change_id).await {
                Ok(workspace) => {
                    self.send_event(ParallelEvent::WorkspaceCreated {
                        change_id: change_id.clone(),
                        workspace: workspace.name.clone(),
                    })
                    .await;
                    workspaces.push(workspace);
                }
                Err(e) => {
                    let error_msg = format!("Failed to create workspace: {}", e);
                    error!("{} for {}", error_msg, change_id);
                    self.send_event(ParallelEvent::Error {
                        message: format!("[{}] {}", change_id, error_msg),
                    })
                    .await;
                    return Err(e);
                }
            }
        }

        // Execute apply commands in parallel with concurrency limit
        let results = match self.execute_applies_parallel(&workspaces).await {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Failed to execute applies: {}", e);
                error!("{}", error_msg);
                self.send_event(ParallelEvent::Error { message: error_msg })
                    .await;
                return Err(e);
            }
        };

        // Collect successful results
        let successful: Vec<WorkspaceResult> = results
            .iter()
            .filter(|r| r.error.is_none())
            .cloned()
            .collect();
        let failed: Vec<WorkspaceResult> = results
            .iter()
            .filter(|r| r.error.is_some())
            .cloned()
            .collect();

        // Report failures
        for result in &failed {
            if let Some(ref err) = result.error {
                error!("Apply failed for {}: {}", result.change_id, err);
                self.send_event(ParallelEvent::ApplyFailed {
                    change_id: result.change_id.clone(),
                    error: err.clone(),
                })
                .await;
            }
        }

        // If all failed, return error
        if successful.is_empty() {
            return Err(OrchestratorError::AgentCommand(
                "All changes in group failed".to_string(),
            ));
        }

        // Archive successful changes in their workspaces BEFORE merge
        // This ensures tasks.md is complete when archive runs
        // We need to collect the final revisions AFTER archive, not before
        let mut final_revisions: Vec<String> = Vec::new();

        for result in &successful {
            // Find the workspace for this change
            let workspace = workspaces.iter().find(|w| w.change_id == result.change_id);

            if let Some(ws) = workspace {
                info!(
                    "Archiving completed change: {} in workspace {}",
                    result.change_id, ws.name
                );
                match self
                    .archive_change_in_workspace(&result.change_id, &ws.path)
                    .await
                {
                    Ok(new_revision) => {
                        self.send_event(ParallelEvent::ChangeArchived {
                            change_id: result.change_id.clone(),
                        })
                        .await;
                        // Use the post-archive revision which includes archive changes
                        final_revisions.push(new_revision);
                    }
                    Err(e) => {
                        warn!("Failed to archive {}: {}", result.change_id, e);
                        self.send_event(ParallelEvent::ArchiveFailed {
                            change_id: result.change_id.clone(),
                            error: e.to_string(),
                        })
                        .await;
                        // Fall back to the apply revision if archive failed
                        if let Some(ref rev) = result.final_revision {
                            final_revisions.push(rev.clone());
                        }
                    }
                }
            } else if let Some(ref rev) = result.final_revision {
                // No workspace found, use the apply revision
                final_revisions.push(rev.clone());
            }
        }

        // Merge using the post-archive revisions
        // These revisions include both apply and archive changes
        let revisions = final_revisions;

        if !revisions.is_empty() {
            self.merge_and_resolve(&revisions).await?;
        }

        // Cleanup workspaces
        for workspace in &workspaces {
            self.send_event(ParallelEvent::CleanupStarted {
                workspace: workspace.name.clone(),
            })
            .await;
            self.workspace_manager
                .cleanup_workspace(&workspace.name)
                .await?;
            self.send_event(ParallelEvent::CleanupCompleted {
                workspace: workspace.name.clone(),
            })
            .await;
        }

        self.send_event(ParallelEvent::GroupCompleted { group_id: group.id })
            .await;

        Ok(())
    }

    /// Execute apply commands in parallel across workspaces
    async fn execute_applies_parallel(
        &mut self,
        workspaces: &[JjWorkspace],
    ) -> Result<Vec<WorkspaceResult>> {
        let max_concurrent = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();

        for workspace in workspaces {
            let sem = semaphore.clone();
            let change_id = workspace.change_id.clone();
            let workspace_path = workspace.path.clone();
            let workspace_name = workspace.name.clone();
            let apply_cmd = self.apply_command.clone();
            let event_tx = self.event_tx.clone();

            // Update status
            self.workspace_manager
                .update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

            join_set.spawn(async move {
                // Acquire semaphore inside spawn to allow all tasks to be created
                let _permit = sem.acquire_owned().await.unwrap();

                let result = Self::execute_apply_in_workspace(
                    &change_id,
                    &workspace_path,
                    &apply_cmd,
                    event_tx,
                )
                .await;
                // _permit is dropped here, releasing semaphore

                match result {
                    Ok(revision) => WorkspaceResult {
                        change_id,
                        workspace_name,
                        final_revision: Some(revision),
                        error: None,
                    },
                    Err(e) => WorkspaceResult {
                        change_id,
                        workspace_name,
                        final_revision: None,
                        error: Some(e.to_string()),
                    },
                }
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(workspace_result) => {
                    // Update workspace status
                    if workspace_result.error.is_some() {
                        self.workspace_manager.update_workspace_status(
                            &workspace_result.workspace_name,
                            WorkspaceStatus::Failed(
                                workspace_result.error.clone().unwrap_or_default(),
                            ),
                        );
                    } else if let Some(ref rev) = workspace_result.final_revision {
                        self.workspace_manager.update_workspace_status(
                            &workspace_result.workspace_name,
                            WorkspaceStatus::Applied(rev.clone()),
                        );
                        self.send_event(ParallelEvent::ApplyCompleted {
                            change_id: workspace_result.change_id.clone(),
                            revision: rev.clone(),
                        })
                        .await;
                    }
                    results.push(workspace_result);
                }
                Err(e) => {
                    warn!("Task join error: {}", e);
                }
            }
        }

        Ok(results)
    }

    /// Execute apply command in a single workspace, repeating until tasks are 100% complete
    async fn execute_apply_in_workspace(
        change_id: &str,
        workspace_path: &PathBuf,
        apply_cmd_template: &str,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
    ) -> Result<String> {
        const MAX_ITERATIONS: u32 = 50;
        let mut iteration = 0;
        let mut first_apply = true;

        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Max iterations ({}) reached for change {}",
                    MAX_ITERATIONS, change_id
                )));
            }

            // Check current task progress in workspace
            let tasks_path = workspace_path
                .join("openspec/changes")
                .join(change_id)
                .join("tasks.md");

            debug!("Checking tasks at: {:?}", tasks_path);

            let progress = if tasks_path.exists() {
                let p = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
                debug!("Tasks file found: {}/{} complete", p.completed, p.total);
                p
            } else {
                debug!("Tasks file not found at {:?}", tasks_path);
                crate::task_parser::TaskProgress::default()
            };

            // Check if already complete
            if progress.total > 0 && progress.completed == progress.total {
                info!(
                    "Change {} is already complete ({}/{})",
                    change_id, progress.completed, progress.total
                );
                break;
            }

            info!(
                "Executing apply #{} for {} in workspace ({}/{} tasks)",
                iteration, change_id, progress.completed, progress.total
            );

            // Send ApplyStarted event on first apply
            if first_apply {
                first_apply = false;
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ApplyStarted {
                            change_id: change_id.to_string(),
                        })
                        .await;
                }
            }

            // Expand change_id and prompt in command
            let command = OrchestratorConfig::expand_change_id(apply_cmd_template, change_id);
            let command = OrchestratorConfig::expand_prompt(&command, "");
            debug!("Workspace path: {:?}", workspace_path);
            debug!("Apply command: {}", command);

            // Execute command in workspace directory
            // Use null stdin to prevent any interactive behavior
            use std::process::Stdio;
            let output = Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(workspace_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to execute: {}", e))
                })?;

            // Send command output summary through event channel
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);

            // Combine stdout and stderr for summary
            let combined_output = if !stdout_str.is_empty() && !stderr_str.is_empty() {
                format!("{}\n{}", stdout_str, stderr_str)
            } else if !stdout_str.is_empty() {
                stdout_str.to_string()
            } else {
                stderr_str.to_string()
            };

            if !combined_output.is_empty() {
                // Extract last few lines for summary
                let lines: Vec<&str> = combined_output.lines().collect();
                let summary = if lines.len() > 10 {
                    format!(
                        "... ({} lines) ...\n{}",
                        lines.len(),
                        lines[lines.len() - 5..].join("\n")
                    )
                } else {
                    combined_output.clone()
                };
                info!(
                    "Sending ApplyOutput for {} ({} lines)",
                    change_id,
                    lines.len()
                );
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ApplyOutput {
                            change_id: change_id.to_string(),
                            output: summary.clone(),
                        })
                        .await;
                }
                debug!("Apply output for {}: {}", change_id, summary);
            } else {
                info!("No output captured for {} apply command", change_id);
            }

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OrchestratorError::AgentCommand(format!(
                    "Apply command failed: {}",
                    stderr
                )));
            }

            // Run jj status to snapshot working copy changes
            // This ensures file modifications are visible for task progress check
            let _ = Command::new("jj")
                .arg("status")
                .current_dir(workspace_path)
                .output()
                .await;

            // Check task progress after apply
            let new_progress = if tasks_path.exists() {
                crate::task_parser::parse_file(&tasks_path).unwrap_or_default()
            } else {
                crate::task_parser::TaskProgress::default()
            };

            info!(
                "After apply #{}: {}/{} tasks complete",
                iteration, new_progress.completed, new_progress.total
            );

            // Check if complete
            if new_progress.total > 0 && new_progress.completed == new_progress.total {
                info!(
                    "Change {} completed after {} iteration(s)",
                    change_id, iteration
                );
                break;
            }

            // Check for progress (avoid infinite loops)
            if new_progress.completed <= progress.completed && iteration > 1 {
                warn!(
                    "No progress made for {} (still {}/{}), continuing...",
                    change_id, new_progress.completed, new_progress.total
                );
            }
        }

        // Get the resulting revision
        // Use --ignore-working-copy to avoid triggering automatic snapshot
        let revision_output = Command::new("jj")
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

        if !revision_output.status.success() {
            let stderr = String::from_utf8_lossy(&revision_output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get workspace revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&revision_output.stdout)
            .trim()
            .to_string())
    }

    /// Merge revisions and resolve any conflicts
    async fn merge_and_resolve(&self, revisions: &[String]) -> Result<()> {
        self.send_event(ParallelEvent::MergeStarted {
            revisions: revisions.to_vec(),
        })
        .await;

        // Attempt merge
        let merge_result = self.workspace_manager.merge_workspaces(revisions).await;

        match merge_result {
            Ok(merge_revision) => {
                self.send_event(ParallelEvent::MergeCompleted {
                    revision: merge_revision,
                })
                .await;
                Ok(())
            }
            Err(OrchestratorError::JjConflict(conflict_info)) => {
                // Detect conflict files
                let conflict_files = self.detect_conflicts().await?;
                self.send_event(ParallelEvent::MergeConflict {
                    files: conflict_files.clone(),
                })
                .await;

                // Attempt automatic resolution
                self.resolve_conflicts_with_retry(&conflict_info).await
            }
            Err(e) => Err(e),
        }
    }

    /// Detect conflicted files from jj status
    async fn detect_conflicts(&self) -> Result<Vec<String>> {
        let output = Command::new("jj")
            .arg("status")
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to run jj status: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
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

    /// Attempt to resolve conflicts with retries using the configured resolve command
    async fn resolve_conflicts_with_retry(&self, _conflict_info: &str) -> Result<()> {
        self.send_event(ParallelEvent::ConflictResolutionStarted)
            .await;

        // Get conflict files for the resolve command
        let conflict_files = self.detect_conflicts().await?;
        let conflict_files_str = conflict_files.join(", ");

        for attempt in 1..=self.max_conflict_retries {
            info!(
                "Conflict resolution attempt {}/{} for files: {}",
                attempt, self.max_conflict_retries, conflict_files_str
            );

            // Expand {conflict_files} placeholder in resolve command
            let command = OrchestratorConfig::expand_conflict_files(
                &self.resolve_command,
                &conflict_files_str,
            );

            debug!("Resolve command: {}", command);

            // Execute resolve command using sh -c
            let output = Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(&self.repo_root)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to run resolve command: {}", e))
                })?;

            if output.status.success() {
                // Verify resolution
                let remaining_conflicts = self.detect_conflicts().await?;
                if remaining_conflicts.is_empty() {
                    self.send_event(ParallelEvent::ConflictResolutionCompleted)
                        .await;
                    return Ok(());
                }
                warn!(
                    "Conflicts still present after resolution attempt: {:?}",
                    remaining_conflicts
                );
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Resolution attempt {} failed: {}", attempt, stderr);
            }
        }

        let error_msg = format!(
            "Failed to resolve conflicts after {} attempts",
            self.max_conflict_retries
        );
        self.send_event(ParallelEvent::ConflictResolutionFailed {
            error: error_msg.clone(),
        })
        .await;

        Err(OrchestratorError::JjConflict(error_msg))
    }

    /// Archive a completed change in a workspace directory
    /// Returns the new revision after archive (includes archive changes)
    async fn archive_change_in_workspace(
        &self,
        change_id: &str,
        workspace_path: &PathBuf,
    ) -> Result<String> {
        // Verify task completion before archiving
        let tasks_path = workspace_path
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md");

        if tasks_path.exists() {
            let progress = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
            if progress.total > 0 && progress.completed < progress.total {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive {}: tasks not complete ({}/{})",
                    change_id, progress.completed, progress.total
                )));
            }
            info!(
                "Task verification passed for {}: {}/{}",
                change_id, progress.completed, progress.total
            );
        } else {
            warn!(
                "Tasks file not found for {} in workspace, proceeding with archive",
                change_id
            );
        }

        // Expand change_id and prompt in archive command
        let command = OrchestratorConfig::expand_change_id(&self.archive_command, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, "");

        debug!("Archive command in workspace: {}", command);

        // Execute command in the workspace directory (where tasks are complete)
        use std::process::Stdio;
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(workspace_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to run archive command: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive failed for {}: {}",
                change_id, stderr
            )));
        }

        info!("Archived change {} in workspace", change_id);

        // Get the current revision after archive (includes archive changes)
        // Run jj status to snapshot the archive changes first
        let _ = Command::new("jj")
            .arg("status")
            .current_dir(workspace_path)
            .stdin(Stdio::null())
            .output()
            .await;

        // Get the new revision
        // Use --ignore-working-copy to avoid triggering automatic snapshot
        let revision_output = Command::new("jj")
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
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to get revision: {}", e)))?;

        if !revision_output.status.success() {
            let stderr = String::from_utf8_lossy(&revision_output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get workspace revision after archive: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&revision_output.stdout)
            .trim()
            .to_string())
    }

    /// Archive a completed change using the configured archive command (in main repo)
    #[allow(dead_code)]
    async fn archive_change(&self, change_id: &str) -> Result<()> {
        // Verify task completion before archiving
        let tasks_path = self
            .repo_root
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md");

        if tasks_path.exists() {
            let progress = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
            if progress.total > 0 && progress.completed < progress.total {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive {}: tasks not complete ({}/{})",
                    change_id, progress.completed, progress.total
                )));
            }
            info!(
                "Task verification passed for {}: {}/{}",
                change_id, progress.completed, progress.total
            );
        } else {
            warn!(
                "Tasks file not found for {}, proceeding with archive",
                change_id
            );
        }

        // Expand change_id and prompt in archive command
        let command = OrchestratorConfig::expand_change_id(&self.archive_command, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, "");

        debug!("Archive command: {}", command);

        // Execute command using sh -c (same as apply command)
        use std::process::Stdio;
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&self.repo_root)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to run archive command: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive failed for {}: {}",
                change_id, stderr
            )));
        }

        info!("Archived change: {}", change_id);
        Ok(())
    }

    /// Send an event to the event channel
    async fn send_event(&self, event: ParallelEvent) {
        if let Some(ref tx) = self.event_tx {
            if let Err(e) = tx.send(event).await {
                debug!("Failed to send parallel event: {}", e);
            }
        }
    }

    /// Cleanup all workspaces
    #[allow(dead_code)]
    pub async fn cleanup(&mut self) -> Result<()> {
        self.workspace_manager.cleanup_all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_event_debug() {
        let event = ParallelEvent::WorkspaceCreated {
            change_id: "test".to_string(),
            workspace: "ws-test".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("WorkspaceCreated"));
    }

    #[test]
    fn test_parallel_executor_creation() {
        let config = OrchestratorConfig::default();
        let repo_root = PathBuf::from("/tmp/test-repo");
        let executor = ParallelExecutor::new(repo_root, config, None);

        assert_eq!(executor.max_conflict_retries, 3);
    }
}
