//! Parallel execution coordinator for jj workspace-based parallel change application.
//!
//! This module manages the parallel execution of changes using jj workspaces,
//! including workspace creation, apply command execution, merge, and cleanup.

use crate::agent::AgentRunner;
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

/// RAII guard for workspace cleanup on partial failures.
///
/// This guard tracks created workspaces and ensures they are cleaned up
/// on drop if not explicitly committed. This prevents workspace leaks
/// when errors occur during workspace creation or apply phases.
struct WorkspaceCleanupGuard {
    /// Workspace names to clean up
    workspace_names: Vec<String>,
    /// Repository root for cleanup commands
    repo_root: PathBuf,
    /// Whether cleanup has been committed (skipped)
    committed: bool,
}

impl WorkspaceCleanupGuard {
    /// Create a new cleanup guard
    fn new(repo_root: PathBuf) -> Self {
        Self {
            workspace_names: Vec::new(),
            repo_root,
            committed: false,
        }
    }

    /// Add a workspace to be tracked for cleanup
    fn track(&mut self, workspace_name: String) {
        self.workspace_names.push(workspace_name);
    }

    /// Commit the guard, preventing cleanup on drop
    ///
    /// Call this when all workspaces have been successfully processed
    /// and cleanup will be handled through the normal path.
    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for WorkspaceCleanupGuard {
    fn drop(&mut self) {
        if self.committed || self.workspace_names.is_empty() {
            return;
        }

        warn!(
            "Cleaning up {} workspace(s) due to early error",
            self.workspace_names.len()
        );

        // Use synchronous cleanup since we're in Drop
        // This is a best-effort cleanup - errors are logged but not propagated
        for workspace_name in &self.workspace_names {
            debug!(
                "Emergency cleanup: forgetting workspace '{}'",
                workspace_name
            );

            // Forget the workspace in jj
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
    }
}

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
    /// Progress updated for a change (task completion tracking)
    ProgressUpdated {
        change_id: String,
        completed: u32,
        total: u32,
    },
    /// Apply completed in a workspace
    ApplyCompleted { change_id: String, revision: String },
    /// Apply failed in a workspace
    ApplyFailed { change_id: String, error: String },
    /// Archive started for a change
    ArchiveStarted { change_id: String },
    /// Archive output (streaming)
    ArchiveOutput { change_id: String, output: String },
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
    /// Analysis started for remaining changes
    AnalysisStarted { remaining_changes: usize },
    /// Analysis output (streaming)
    AnalysisOutput { output: String },
    /// Analysis completed
    AnalysisCompleted { groups_found: usize },
    /// Resolve output (streaming)
    ResolveOutput { output: String },
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

    /// Execute changes with dynamic re-analysis after each group completes.
    ///
    /// This method analyzes the remaining changes after each group completes,
    /// allowing the LLM to reconsider dependencies based on the current state.
    pub async fn execute_with_reanalysis<F>(
        &mut self,
        mut changes: Vec<crate::openspec::Change>,
        analyzer: F,
    ) -> Result<()>
    where
        F: Fn(
                &[crate::openspec::Change],
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Vec<ParallelGroup>> + Send + '_>,
            > + Send
            + Sync,
    {
        if changes.is_empty() {
            self.send_event(ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!(
            "Starting execution with re-analysis for {} changes",
            changes.len()
        );

        // Snapshot working copy
        info!("Snapshotting working copy for parallel execution...");
        if let Err(e) = self.workspace_manager.snapshot_working_copy().await {
            let error_msg = format!("Failed to snapshot working copy: {}", e);
            error!("{}", error_msg);
            self.send_event(ParallelEvent::Error { message: error_msg })
                .await;
            return Err(e);
        }
        info!("Working copy snapshot complete");

        let mut group_counter = 1u32;

        while !changes.is_empty() {
            // Analyze remaining changes to get the next group
            info!(
                "Analyzing {} remaining changes for next group",
                changes.len()
            );
            self.send_event(ParallelEvent::AnalysisStarted {
                remaining_changes: changes.len(),
            })
            .await;

            let groups = analyzer(&changes).await;

            if groups.is_empty() {
                warn!("No groups returned from analysis");
                break;
            }

            // Execute only the first group (no dependencies)
            let first_group = ParallelGroup {
                id: group_counter,
                changes: groups[0].changes.clone(),
                depends_on: Vec::new(),
            };

            info!(
                "Executing group {} with {} changes: {:?}",
                first_group.id,
                first_group.changes.len(),
                first_group.changes
            );

            self.execute_group(&first_group).await?;

            // Remove completed changes from the list
            let completed_set: std::collections::HashSet<_> = first_group.changes.iter().collect();
            changes.retain(|c| !completed_set.contains(&c.id));

            group_counter += 1;
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

        // Create cleanup guard to ensure workspaces are cleaned up on early errors
        let mut cleanup_guard = WorkspaceCleanupGuard::new(self.repo_root.clone());

        // Create workspaces for all changes in the group from the SAME base revision
        // This ensures all changes in the group branch from the same point (true parallel)
        let mut workspaces: Vec<JjWorkspace> = Vec::new();
        for change_id in &group.changes {
            match self
                .workspace_manager
                .create_workspace_from(change_id, Some(&base_revision))
                .await
            {
                Ok(workspace) => {
                    // Track workspace in cleanup guard before adding to list
                    cleanup_guard.track(workspace.name.clone());

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
                    // cleanup_guard will clean up previously created workspaces on drop
                    return Err(e);
                }
            }
        }

        // Execute apply + archive in parallel with concurrency limit
        // Each task: apply -> (if success) -> archive
        let results = match self.execute_apply_and_archive_parallel(&workspaces).await {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("Failed to execute applies: {}", e);
                error!("{}", error_msg);
                self.send_event(ParallelEvent::Error { message: error_msg })
                    .await;
                return Err(e);
            }
        };

        // Collect successful results (those with final_revision set)
        let successful: Vec<&WorkspaceResult> = results
            .iter()
            .filter(|r| r.final_revision.is_some())
            .collect();
        let failed: Vec<&WorkspaceResult> = results.iter().filter(|r| r.error.is_some()).collect();

        // Report failures (already reported in parallel execution, but log summary)
        for result in &failed {
            if let Some(ref err) = result.error {
                error!("Failed for {}: {}", result.change_id, err);
            }
        }

        // If all failed, return error
        if successful.is_empty() {
            return Err(OrchestratorError::AgentCommand(
                "All changes in group failed".to_string(),
            ));
        }

        // Collect final revisions (post-archive revisions)
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

        // Commit the cleanup guard since normal cleanup succeeded
        // This prevents double-cleanup on drop
        cleanup_guard.commit();

        self.send_event(ParallelEvent::GroupCompleted { group_id: group.id })
            .await;

        Ok(())
    }

    /// Execute apply + archive in parallel across workspaces
    /// Each task: apply -> (if success) -> archive
    /// Archive starts immediately after apply completes for each change
    async fn execute_apply_and_archive_parallel(
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
            let archive_cmd = self.archive_command.clone();
            let config = self.config.clone();
            let event_tx = self.event_tx.clone();

            // Update status
            self.workspace_manager
                .update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

            join_set.spawn(async move {
                // Acquire semaphore inside spawn to allow all tasks to be created
                let _permit = sem.acquire_owned().await.unwrap();

                // Step 1: Execute apply
                let apply_result = Self::execute_apply_in_workspace(
                    &change_id,
                    &workspace_path,
                    &apply_cmd,
                    event_tx.clone(),
                )
                .await;

                match apply_result {
                    Ok(apply_revision) => {
                        // Send ApplyCompleted event
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ApplyCompleted {
                                    change_id: change_id.clone(),
                                    revision: apply_revision.clone(),
                                })
                                .await;
                        }

                        // Step 2: Execute archive immediately after apply succeeds
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ArchiveStarted {
                                    change_id: change_id.clone(),
                                })
                                .await;
                        }

                        let archive_result = Self::execute_archive_in_workspace(
                            &change_id,
                            &workspace_path,
                            &archive_cmd,
                            &config,
                            event_tx.clone(),
                        )
                        .await;

                        match archive_result {
                            Ok(archive_revision) => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeArchived {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                }
                                WorkspaceResult {
                                    change_id,
                                    workspace_name,
                                    final_revision: Some(archive_revision),
                                    error: None,
                                }
                            }
                            Err(e) => {
                                warn!("Archive failed for {}: {}", change_id, e);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ArchiveFailed {
                                            change_id: change_id.clone(),
                                            error: e.to_string(),
                                        })
                                        .await;
                                }
                                // Fall back to apply revision if archive fails
                                WorkspaceResult {
                                    change_id,
                                    workspace_name,
                                    final_revision: Some(apply_revision),
                                    error: None,
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ApplyFailed {
                                    change_id: change_id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                        WorkspaceResult {
                            change_id,
                            workspace_name,
                            final_revision: None,
                            error: Some(e.to_string()),
                        }
                    }
                }
                // _permit is dropped here, releasing semaphore
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

    /// Execute archive command in a workspace with streaming output
    async fn execute_archive_in_workspace(
        change_id: &str,
        workspace_path: &PathBuf,
        archive_cmd_template: &str,
        config: &OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
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
        let command = OrchestratorConfig::expand_change_id(archive_cmd_template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, config.get_archive_prompt());

        debug!("Archive command in workspace: {}", command);

        // Execute command with streaming output
        use std::process::Stdio;
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(workspace_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to spawn archive command: {}", e))
            })?;

        // Stream stdout
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let change_id_clone = change_id.to_string();
        let event_tx_clone = event_tx.clone();

        let stdout_handle = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(ref tx) = event_tx_clone {
                        let _ = tx
                            .send(ParallelEvent::ArchiveOutput {
                                change_id: change_id_clone.clone(),
                                output: line,
                            })
                            .await;
                    }
                }
            }
        });

        let change_id_clone2 = change_id.to_string();
        let event_tx_clone2 = event_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(ref tx) = event_tx_clone2 {
                        let _ = tx
                            .send(ParallelEvent::ArchiveOutput {
                                change_id: change_id_clone2.clone(),
                                output: line,
                            })
                            .await;
                    }
                }
            }
        });

        // Wait for streams to complete
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Archive command failed: {}", e))
        })?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive command failed with exit code: {:?}",
                status.code()
            )));
        }

        // Get the current revision after archive
        let revision_output = Command::new("jj")
            .args(["log", "-r", "@", "--no-graph", "-T", "change_id"])
            .current_dir(workspace_path)
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to get revision: {}", e)))?;

        if !revision_output.status.success() {
            let stderr = String::from_utf8_lossy(&revision_output.stderr);
            return Err(OrchestratorError::JjCommand(format!(
                "Failed to get revision: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&revision_output.stdout)
            .trim()
            .to_string())
    }

    /// Check task progress for a change in the given workspace.
    ///
    /// Reads and parses the tasks.md file to determine completion status.
    /// Returns default progress if the file doesn't exist or can't be parsed.
    fn check_task_progress(
        workspace_path: &PathBuf,
        change_id: &str,
    ) -> crate::task_parser::TaskProgress {
        let tasks_path = workspace_path
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md");

        debug!("Checking tasks at: {:?}", tasks_path);

        if tasks_path.exists() {
            let progress = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
            debug!(
                "Tasks file found: {}/{} complete",
                progress.completed, progress.total
            );
            progress
        } else {
            debug!("Tasks file not found at {:?}", tasks_path);
            crate::task_parser::TaskProgress::default()
        }
    }

    /// Summarize command output for logging and event reporting.
    ///
    /// If output exceeds max_lines, returns the last few lines with a count prefix.
    fn summarize_output(output: &str, max_lines: usize) -> String {
        if output.is_empty() {
            return String::new();
        }

        let lines: Vec<&str> = output.lines().collect();
        if lines.len() > max_lines {
            // Show last 5 lines with total count
            let tail_lines = 5.min(lines.len());
            format!(
                "... ({} lines) ...\n{}",
                lines.len(),
                lines[lines.len() - tail_lines..].join("\n")
            )
        } else {
            output.to_string()
        }
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

            // Check current task progress using helper
            let progress = Self::check_task_progress(workspace_path, change_id);

            // Send progress event
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: progress.completed,
                        total: progress.total,
                    })
                    .await;
            }

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

            // Combine stdout and stderr for summary
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            let combined_output = if !stdout_str.is_empty() && !stderr_str.is_empty() {
                format!("{}\n{}", stdout_str, stderr_str)
            } else if !stdout_str.is_empty() {
                stdout_str.to_string()
            } else {
                stderr_str.to_string()
            };

            // Send summarized output through event channel
            let summary = Self::summarize_output(&combined_output, 10);
            if !summary.is_empty() {
                let line_count = combined_output.lines().count();
                info!(
                    "Sending ApplyOutput for {} ({} lines)",
                    change_id, line_count
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

            // Check task progress after apply using helper
            let new_progress = Self::check_task_progress(workspace_path, change_id);

            // Send progress event after apply
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: new_progress.completed,
                        total: new_progress.total,
                    })
                    .await;
            }

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

        // Set a meaningful commit message for the completed change
        use std::process::Stdio as StdStdio;
        let describe_output = Command::new("jj")
            .args(["describe", "-m", &format!("Apply: {}", change_id)])
            .current_dir(workspace_path)
            .stdin(StdStdio::null())
            .output()
            .await;

        if let Err(e) = describe_output {
            warn!("Failed to set commit message for {}: {}", change_id, e);
        } else if let Ok(output) = describe_output {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to set commit message for {}: {}", change_id, stderr);
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

                // Attempt automatic resolution with jj context
                self.resolve_conflicts_with_retry(revisions, &conflict_info)
                    .await
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

    /// Get jj status output for context
    async fn get_jj_status(&self) -> Result<String> {
        let output = Command::new("jj")
            .arg("status")
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to run jj status: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get jj log for specific revisions
    async fn get_jj_log_for_revisions(&self, revisions: &[String]) -> Result<String> {
        if revisions.is_empty() {
            return Ok(String::new());
        }

        // Build revset for the revisions
        let revset = revisions.join(" | ");

        let output = Command::new("jj")
            .args(["log", "-r", &revset, "--no-graph"])
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| OrchestratorError::JjCommand(format!("Failed to run jj log: {}", e)))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Attempt to resolve conflicts with retries using the configured resolve command
    async fn resolve_conflicts_with_retry(
        &self,
        revisions: &[String],
        jj_error: &str,
    ) -> Result<()> {
        self.send_event(ParallelEvent::ConflictResolutionStarted)
            .await;

        // Get conflict files for the resolve command
        let conflict_files = self.detect_conflicts().await?;
        let conflict_files_str = conflict_files.join(", ");

        // Get jj status for context
        let jj_status = self.get_jj_status().await.unwrap_or_default();

        // Get jj log for the conflicting revisions
        let jj_log = self
            .get_jj_log_for_revisions(revisions)
            .await
            .unwrap_or_default();

        for attempt in 1..=self.max_conflict_retries {
            info!(
                "Conflict resolution attempt {}/{} for files: {}",
                attempt, self.max_conflict_retries, conflict_files_str
            );

            // Build the resolve prompt with full jj context
            let resolve_prompt = format!(
                "This project uses jj (Jujutsu) for version control, not git.\n\n\
                 A merge conflict occurred while trying to merge the following revisions:\n\
                 {}\n\n\
                 jj error output:\n\
                 {}\n\n\
                 Current jj status:\n\
                 {}\n\n\
                 jj log for conflicting changes:\n\
                 {}\n\n\
                 Conflicting files: {}\n\n\
                 Please resolve the merge conflicts in the listed files. \
                 The files contain jj conflict markers. After editing, jj will automatically detect the resolution.",
                revisions.join(", "),
                jj_error,
                jj_status,
                jj_log,
                conflict_files_str
            );

            // Use AgentRunner for streaming resolve command execution
            let agent = AgentRunner::new(self.config.clone());
            let (mut child, mut rx) = agent.run_resolve_streaming(&resolve_prompt).await?;

            // Stream output to events
            while let Some(line) = rx.recv().await {
                let text = match &line {
                    crate::agent::OutputLine::Stdout(s) | crate::agent::OutputLine::Stderr(s) => {
                        s.clone()
                    }
                };
                self.send_event(ParallelEvent::ResolveOutput {
                    output: text.clone(),
                })
                .await;
            }

            // Wait for process to complete
            let status = child.wait().await.map_err(|e| {
                OrchestratorError::AgentCommand(format!("Resolve command failed: {}", e))
            })?;

            if status.success() {
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
                warn!(
                    "Resolution attempt {} failed with exit code: {:?}",
                    attempt,
                    status.code()
                );
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
