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
        // Determine workspace base directory
        let base_dir = config
            .get_workspace_base_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("openspec-workspaces"));

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

        for group in groups {
            self.execute_group(&group).await?;
        }

        self.send_event(ParallelEvent::AllCompleted).await;
        Ok(())
    }

    /// Execute a single group of changes
    async fn execute_group(&mut self, group: &ParallelGroup) -> Result<()> {
        info!(
            "Executing group {} with {} changes: {:?}",
            group.id,
            group.changes.len(),
            group.changes
        );

        self.send_event(ParallelEvent::GroupStarted {
            group_id: group.id,
            changes: group.changes.clone(),
        })
        .await;

        // Create workspaces for all changes in the group
        let mut workspaces: Vec<JjWorkspace> = Vec::new();
        for change_id in &group.changes {
            let workspace = self.workspace_manager.create_workspace(change_id).await?;
            self.send_event(ParallelEvent::WorkspaceCreated {
                change_id: change_id.clone(),
                workspace: workspace.name.clone(),
            })
            .await;
            workspaces.push(workspace);
        }

        // Execute apply commands in parallel with concurrency limit
        let results = self.execute_applies_parallel(&workspaces).await?;

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

        // Archive successful changes before merge
        for result in &successful {
            info!("Archiving completed change: {}", result.change_id);
            match self.archive_change(&result.change_id).await {
                Ok(()) => {
                    self.send_event(ParallelEvent::ChangeArchived {
                        change_id: result.change_id.clone(),
                    })
                    .await;
                }
                Err(e) => {
                    warn!("Failed to archive {}: {}", result.change_id, e);
                    self.send_event(ParallelEvent::ArchiveFailed {
                        change_id: result.change_id.clone(),
                        error: e.to_string(),
                    })
                    .await;
                }
            }
        }

        // Merge successful results
        let revisions: Vec<String> = successful
            .iter()
            .filter_map(|r| r.final_revision.clone())
            .collect();

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

            // Update status
            self.workspace_manager
                .update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

            join_set.spawn(async move {
                // Acquire semaphore inside spawn to allow all tasks to be created
                let _permit = sem.acquire_owned().await.unwrap();

                let result =
                    Self::execute_apply_in_workspace(&change_id, &workspace_path, &apply_cmd).await;
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
    ) -> Result<String> {
        const MAX_ITERATIONS: u32 = 50;
        let mut iteration = 0;

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

            // Expand change_id and prompt in command
            let command = OrchestratorConfig::expand_change_id(apply_cmd_template, change_id);
            let command = OrchestratorConfig::expand_prompt(&command, "");
            debug!("Workspace path: {:?}", workspace_path);
            debug!("Apply command: {}", command);

            // Execute command in workspace directory
            let output = Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to execute: {}", e))
                })?;

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
        let revision_output = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "change_id"])
            .current_dir(workspace_path)
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to get revision: {}", e)))?;

        if !revision_output.status.success() {
            return Err(OrchestratorError::JjCommand(
                "Failed to get workspace revision".to_string(),
            ));
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
            let command =
                OrchestratorConfig::expand_conflict_files(&self.resolve_command, &conflict_files_str);

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

    /// Archive a completed change using the configured archive command
    async fn archive_change(&self, change_id: &str) -> Result<()> {
        // Expand change_id and prompt in archive command
        let command = OrchestratorConfig::expand_change_id(&self.archive_command, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, "");

        debug!("Archive command: {}", command);

        // Execute command using sh -c (same as apply command)
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&self.repo_root)
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
