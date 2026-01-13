//! Parallel execution coordinator for VCS workspace-based parallel change application.
//!
//! This module manages the parallel execution of changes using Git worktrees,
//! including workspace creation, apply command execution, merge, and cleanup.

mod cleanup;
mod conflict;
mod events;
mod executor;
mod types;

// Re-export ExecutionEvent as ParallelEvent for backward compatibility
pub use crate::events::ExecutionEvent as ParallelEvent;
pub use types::{FailedChangeTracker, WorkspaceResult};

use crate::analyzer::{extract_change_dependencies, ParallelGroup};
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::archive::is_change_archived;
use crate::vcs::{
    GitWorkspaceManager, VcsBackend, VcsError, Workspace, WorkspaceManager, WorkspaceStatus,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use cleanup::WorkspaceCleanupGuard;
use events::send_event;
use executor::{execute_apply_in_workspace, execute_archive_in_workspace, ParallelHookContext};

use crate::hooks::HookRunner;

/// Parallel executor for running changes in git worktrees
pub struct ParallelExecutor {
    /// Workspace manager (VCS-agnostic)
    workspace_manager: Box<dyn WorkspaceManager>,
    /// Configuration (used for AgentRunner and resolve operations)
    config: OrchestratorConfig,
    /// Apply command template
    apply_command: String,
    /// Archive command template
    archive_command: String,
    /// Event sender
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    /// Maximum retries for conflict resolution
    max_conflict_retries: u32,
    /// Repository root path for archive operations
    repo_root: PathBuf,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
    /// Tracker for failed changes to enable skipping dependent changes
    failed_tracker: FailedChangeTracker,
    /// Hook runner for executing hooks (optional)
    hooks: Option<Arc<HookRunner>>,
}

impl ParallelExecutor {
    /// Create a new parallel executor with automatic VCS detection
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
    ) -> Self {
        // Auto-detect VCS backend
        let vcs_backend = config.get_vcs_backend();
        Self::with_backend(repo_root, config, event_tx, vcs_backend)
    }

    /// Create a new parallel executor with a specific VCS backend
    pub fn with_backend(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
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

        // Resolve the VCS backend (handle Auto)
        let resolved_backend = Self::resolve_backend(vcs_backend, &repo_root);
        info!("Using VCS backend: {:?}", resolved_backend);

        let workspace_manager: Box<dyn WorkspaceManager> = match resolved_backend {
            VcsBackend::Git | VcsBackend::Auto => Box::new(GitWorkspaceManager::new(
                base_dir,
                repo_root.clone(),
                max_concurrent,
                config.clone(),
            )),
        };

        Self {
            workspace_manager,
            config,
            apply_command,
            archive_command,
            event_tx,
            max_conflict_retries: 3,
            repo_root,
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            hooks: None,
        }
    }

    /// Set the hook runner for executing hooks during parallel execution.
    #[allow(dead_code)] // Public API for future integration with CLI/TUI
    pub fn set_hooks(&mut self, hooks: HookRunner) {
        self.hooks = Some(Arc::new(hooks));
    }

    /// Set whether to disable automatic workspace resume.
    ///
    /// When `no_resume` is true, existing workspaces are always deleted
    /// and new ones are created. When false (default), existing workspaces
    /// are reused to resume interrupted work.
    pub fn set_no_resume(&mut self, no_resume: bool) {
        self.no_resume = no_resume;
    }

    /// Resolve VCS backend (convert Auto to concrete backend)
    fn resolve_backend(backend: VcsBackend, _repo_root: &Path) -> VcsBackend {
        match backend {
            VcsBackend::Auto => VcsBackend::Git,
            other => other,
        }
    }

    /// Get the VCS backend type
    #[allow(dead_code)] // Public API for external callers
    pub fn backend_type(&self) -> VcsBackend {
        self.workspace_manager.backend_type()
    }

    /// Check if VCS is available for parallel execution
    #[allow(dead_code)] // Public API, used via ParallelRunService
    pub async fn check_vcs_available(&self) -> Result<bool> {
        self.workspace_manager
            .check_available()
            .await
            .map_err(Into::into)
    }

    /// Execute groups in topological order
    pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
        if groups.is_empty() {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!("Executing {} groups in parallel mode", groups.len());

        // Extract change-level dependencies from groups and set them in the tracker
        let change_deps = extract_change_dependencies(&groups);
        self.failed_tracker.set_dependencies(change_deps);

        // Calculate total changes count
        let total_changes: usize = groups.iter().map(|g| g.changes.len()).sum();
        let mut changes_processed: usize = 0;

        // Prepare for parallel execution (clean check for git)
        info!("Preparing for parallel execution...");
        match self.workspace_manager.prepare_for_parallel().await {
            Ok(Some(warning)) => {
                warn!("{}", warning.message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Warning {
                        title: warning.title,
                        message: warning.message,
                    },
                )
                .await;
            }
            Ok(None) => {}
            Err(e) => {
                let error_msg = format!("Failed to prepare for parallel execution: {}", e);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(e.into());
            }
        }
        info!("Preparation complete");

        for group in groups {
            let group_size = group.changes.len();
            self.execute_group(&group, total_changes, changes_processed)
                .await?;
            changes_processed += group_size;
        }

        send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
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
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!(
            "Starting execution with re-analysis for {} changes",
            changes.len()
        );

        // Prepare for parallel execution (clean check for git)
        info!("Preparing for parallel execution...");
        match self.workspace_manager.prepare_for_parallel().await {
            Ok(Some(warning)) => {
                warn!("{}", warning.message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Warning {
                        title: warning.title,
                        message: warning.message,
                    },
                )
                .await;
            }
            Ok(None) => {}
            Err(e) => {
                let error_msg = format!("Failed to prepare for parallel execution: {}", e);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(e.into());
            }
        }
        info!("Preparation complete");

        let mut group_counter = 1u32;
        let initial_total_changes = changes.len();
        let mut changes_processed: usize = 0;

        while !changes.is_empty() {
            // Filter out changes that depend on failed changes
            let executable_changes: Vec<_> = changes
                .iter()
                .filter(|c| {
                    if let Some(failed_dep) = self.failed_tracker.should_skip(&c.id) {
                        warn!(
                            "Excluding '{}' from analysis: dependency '{}' failed",
                            c.id, failed_dep
                        );
                        // Emit skip event
                        // Note: We can't async here, so we'll emit after filtering
                        false
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            // Emit skip events for filtered changes
            for change in &changes {
                if let Some(failed_dep) = self.failed_tracker.should_skip(&change.id) {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ChangeSkipped {
                            change_id: change.id.clone(),
                            reason: format!("Dependency '{}' failed", failed_dep),
                        },
                    )
                    .await;
                }
            }

            // Update changes to only include executable ones
            changes = executable_changes;

            if changes.is_empty() {
                info!("All remaining changes depend on failed changes, stopping");
                break;
            }

            // Analyze remaining changes to get the next group
            info!(
                "Analyzing {} remaining changes for next group",
                changes.len()
            );
            send_event(
                &self.event_tx,
                ParallelEvent::AnalysisStarted {
                    remaining_changes: changes.len(),
                },
            )
            .await;

            let groups = analyzer(&changes).await;

            if groups.is_empty() {
                warn!("No groups returned from analysis");
                break;
            }

            // Extract change-level dependencies for this iteration
            let change_deps = extract_change_dependencies(&groups);
            self.failed_tracker.set_dependencies(change_deps);

            // Execute only the first group (no dependencies)
            let first_group = ParallelGroup {
                id: group_counter,
                changes: groups[0].changes.clone(),
                depends_on: Vec::new(),
            };

            let group_size = first_group.changes.len();
            info!(
                "Executing group {} with {} changes: {:?}",
                first_group.id, group_size, first_group.changes
            );

            self.execute_group(&first_group, initial_total_changes, changes_processed)
                .await?;

            // Remove completed changes from the list
            let completed_set: std::collections::HashSet<_> = first_group.changes.iter().collect();
            changes.retain(|c| !completed_set.contains(&c.id));

            changes_processed += group_size;
            group_counter += 1;
        }

        send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        Ok(())
    }

    /// Execute a single group of changes
    async fn execute_group(
        &mut self,
        group: &ParallelGroup,
        total_changes: usize,
        changes_processed: usize,
    ) -> Result<()> {
        // First, check which changes should be skipped due to failed dependencies
        let mut changes_to_execute: Vec<String> = Vec::new();
        let mut skipped_changes: Vec<(String, String)> = Vec::new();

        for change_id in &group.changes {
            if let Some(failed_dep) = self.failed_tracker.should_skip(change_id) {
                warn!(
                    "Skipping '{}' because dependency '{}' failed",
                    change_id, failed_dep
                );
                skipped_changes.push((change_id.clone(), failed_dep));
            } else {
                changes_to_execute.push(change_id.clone());
            }
        }

        // Emit events for skipped changes
        for (change_id, failed_dep) in &skipped_changes {
            send_event(
                &self.event_tx,
                ParallelEvent::ChangeSkipped {
                    change_id: change_id.clone(),
                    reason: format!("Dependency '{}' failed", failed_dep),
                },
            )
            .await;
        }

        // If all changes are skipped, we're done with this group
        if changes_to_execute.is_empty() {
            info!(
                "All changes in group {} were skipped due to failed dependencies",
                group.id
            );
            send_event(
                &self.event_tx,
                ParallelEvent::GroupCompleted { group_id: group.id },
            )
            .await;
            return Ok(());
        }

        // Get current base revision for this group's workspaces
        let base_revision = self
            .workspace_manager
            .get_current_revision()
            .await
            .map_err(OrchestratorError::from)?;
        info!(
            "Executing group {} with {} changes: {:?} (base revision: {})",
            group.id,
            changes_to_execute.len(),
            changes_to_execute,
            &base_revision[..8.min(base_revision.len())]
        );

        send_event(
            &self.event_tx,
            ParallelEvent::GroupStarted {
                group_id: group.id,
                changes: changes_to_execute.clone(),
            },
        )
        .await;

        // Create cleanup guard to ensure workspaces are cleaned up on early errors
        let mut cleanup_guard = WorkspaceCleanupGuard::new(
            self.workspace_manager.backend_type(),
            self.repo_root.clone(),
        );

        // Create or reuse workspaces for all changes in the group
        // If resume is enabled (default), try to find existing workspaces first
        let mut workspaces: Vec<Workspace> = Vec::new();
        let mut archived_results: Vec<WorkspaceResult> = Vec::new();
        let mut archived_workspaces: Vec<Workspace> = Vec::new();
        for change_id in &changes_to_execute {
            // Try to find and reuse existing workspace (unless --no-resume is set)
            let workspace = if self.no_resume {
                None
            } else {
                match self
                    .workspace_manager
                    .find_existing_workspace(change_id)
                    .await
                {
                    Ok(Some(workspace_info)) => {
                        // Found existing workspace, reuse it
                        info!(
                            "Resuming existing workspace for '{}' (last modified: {:?})",
                            change_id, workspace_info.last_modified
                        );
                        match self
                            .workspace_manager
                            .reuse_workspace(&workspace_info)
                            .await
                        {
                            Ok(ws) => Some(ws),
                            Err(e) => {
                                warn!(
                                    "Failed to reuse workspace for '{}': {}, creating new",
                                    change_id, e
                                );
                                None
                            }
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        warn!(
                            "Failed to find existing workspace for '{}': {}, creating new",
                            change_id, e
                        );
                        None
                    }
                }
            };

            let (workspace, resumed) = match workspace {
                Some(ws) => {
                    // Track workspace in cleanup guard before adding to list
                    cleanup_guard.track(ws.name.clone());

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceResumed {
                            change_id: change_id.clone(),
                            workspace: ws.name.clone(),
                        },
                    )
                    .await;

                    // Send ProcessingStarted event early to show processing status in TUI
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ProcessingStarted(change_id.clone()),
                    )
                    .await;

                    (ws, true)
                }
                None => {
                    // Create new workspace from the base revision
                    match self
                        .workspace_manager
                        .create_workspace(change_id, Some(&base_revision))
                        .await
                    {
                        Ok(ws) => {
                            // Track workspace in cleanup guard before adding to list
                            cleanup_guard.track(ws.name.clone());

                            send_event(
                                &self.event_tx,
                                ParallelEvent::WorkspaceCreated {
                                    change_id: change_id.clone(),
                                    workspace: ws.name.clone(),
                                },
                            )
                            .await;

                            // Send ProcessingStarted event early to show processing status in TUI
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ProcessingStarted(change_id.clone()),
                            )
                            .await;

                            (ws, false)
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to create workspace: {}", e);
                            error!("{} for {}", error_msg, change_id);
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Error {
                                    message: format!("[{}] {}", change_id, error_msg),
                                },
                            )
                            .await;
                            // cleanup_guard will clean up previously created workspaces on drop
                            return Err(e.into());
                        }
                    }
                }
            };

            if resumed && is_change_archived(change_id, Some(&workspace.path)) {
                info!(
                    "Change '{}' already archived in workspace '{}', skipping apply/archive",
                    change_id, workspace.name
                );

                send_event(
                    &self.event_tx,
                    ParallelEvent::ArchiveStarted(change_id.clone()),
                )
                .await;
                send_event(
                    &self.event_tx,
                    ParallelEvent::ChangeArchived(change_id.clone()),
                )
                .await;

                let revision = self
                    .workspace_manager
                    .get_revision_in_workspace(&workspace.path)
                    .await
                    .map_err(OrchestratorError::from)?;
                self.workspace_manager.update_workspace_status(
                    &workspace.name,
                    WorkspaceStatus::Applied(revision.clone()),
                );

                archived_results.push(WorkspaceResult {
                    change_id: change_id.clone(),
                    workspace_name: workspace.name.clone(),
                    final_revision: Some(revision),
                    error: None,
                });
                archived_workspaces.push(workspace);
                continue;
            }

            workspaces.push(workspace);
        }

        for result in &archived_results {
            if let Some(ref rev) = result.final_revision {
                info!("Merging archived {} (revision: {})", result.change_id, rev);
                if let Err(e) = self.merge_and_resolve(std::slice::from_ref(rev)).await {
                    let error_msg = format!(
                        "Failed to merge archived {} (revision: {}): {}",
                        result.change_id, rev, e
                    );
                    error!("{}", error_msg);
                    send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                    return Err(e);
                }
            }
        }

        // Execute apply + archive in parallel with concurrency limit
        // Each task: apply -> (if success) -> archive
        let mut results = archived_results;
        if !workspaces.is_empty() {
            let apply_results = match self
                .execute_apply_and_archive_parallel(
                    &workspaces,
                    Some(group.id),
                    total_changes,
                    changes_processed,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = format!("Failed to execute applies: {}", e);
                    error!("{}", error_msg);
                    send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                    return Err(e);
                }
            };
            results.extend(apply_results);
        }

        // Collect successful and failed results
        let successful: Vec<&WorkspaceResult> = results
            .iter()
            .filter(|r| r.final_revision.is_some())
            .collect();
        let failed: Vec<&WorkspaceResult> = results.iter().filter(|r| r.error.is_some()).collect();

        // Report failures and mark them in the tracker for dependent skipping
        // Also preserve workspaces for failed changes (do not cleanup)
        for result in &failed {
            if result.error.is_some() {
                error!(
                    "Failed for {}, workspace preserved: {}",
                    result.change_id, result.workspace_name
                );
                info!(
                    "To resume: run with the same change_id, workspace will be automatically detected"
                );
            }
            // Emit WorkspacePreserved event
            send_event(
                &self.event_tx,
                ParallelEvent::WorkspacePreserved {
                    change_id: result.change_id.clone(),
                    workspace_name: result.workspace_name.clone(),
                },
            )
            .await;
            // Mark the failed change so dependent changes will be skipped
            self.failed_tracker.mark_failed(&result.change_id);
        }

        // If all failed, we don't have an error but continue to the next group
        // The dependent changes will be skipped automatically
        if successful.is_empty() && !results.is_empty() {
            warn!(
                "All changes in group {} failed, dependent changes will be skipped",
                group.id
            );
            send_event(
                &self.event_tx,
                ParallelEvent::GroupCompleted { group_id: group.id },
            )
            .await;
            return Ok(());
        }

        // Note: Individual merging is now done in execute_apply_and_archive_parallel
        // immediately after each change is archived. Group-level merge is no longer needed.

        // Cleanup only successful workspaces (preserve failed ones)
        let failed_workspace_names: std::collections::HashSet<_> =
            failed.iter().map(|r| r.workspace_name.clone()).collect();
        let mut cleanup_workspaces = workspaces.clone();
        cleanup_workspaces.extend(archived_workspaces);
        for workspace in &cleanup_workspaces {
            // Skip cleanup for failed workspaces - they are preserved
            if failed_workspace_names.contains(&workspace.name) {
                continue;
            }
            send_event(
                &self.event_tx,
                ParallelEvent::CleanupStarted {
                    workspace: workspace.name.clone(),
                },
            )
            .await;
            self.workspace_manager
                .cleanup_workspace(&workspace.name)
                .await
                .map_err(OrchestratorError::from)?;
            send_event(
                &self.event_tx,
                ParallelEvent::CleanupCompleted {
                    workspace: workspace.name.clone(),
                },
            )
            .await;
        }

        // Commit the cleanup guard since normal cleanup succeeded
        // This prevents double-cleanup on drop
        cleanup_guard.commit();

        send_event(
            &self.event_tx,
            ParallelEvent::GroupCompleted { group_id: group.id },
        )
        .await;

        Ok(())
    }

    /// Execute apply + archive in parallel across workspaces
    /// Each task: apply -> (if success) -> archive
    /// Archive starts immediately after apply completes for each change
    async fn execute_apply_and_archive_parallel(
        &mut self,
        workspaces: &[Workspace],
        group_index: Option<u32>,
        total_changes: usize,
        changes_processed: usize,
    ) -> Result<Vec<WorkspaceResult>> {
        let max_concurrent = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let total_changes_in_group = workspaces.len();

        for workspace in workspaces {
            let sem = semaphore.clone();
            let change_id = workspace.change_id.clone();
            let workspace_path = workspace.path.clone();
            let workspace_name = workspace.name.clone();
            let apply_cmd = self.apply_command.clone();
            let archive_cmd = self.archive_command.clone();
            let config = self.config.clone();
            let event_tx = self.event_tx.clone();
            let vcs_backend = self.workspace_manager.backend_type();
            let hooks = self.hooks.clone();

            // Build parallel hook context
            let parallel_ctx = ParallelHookContext {
                workspace_path: workspace_path.to_string_lossy().to_string(),
                group_index,
                total_changes_in_group,
                total_changes,
                changes_processed,
            };

            // Update status
            self.workspace_manager
                .update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

            join_set.spawn(async move {
                // Acquire semaphore inside spawn to allow all tasks to be created
                let _permit = sem.acquire_owned().await.unwrap();

                // Step 1: Execute apply
                let apply_result = execute_apply_in_workspace(
                    &change_id,
                    &workspace_path,
                    &apply_cmd,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    hooks.as_ref().map(|h| h.as_ref()),
                    Some(&parallel_ctx),
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
                                .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                                .await;
                        }

                        let archive_result = execute_archive_in_workspace(
                            &change_id,
                            &workspace_path,
                            &archive_cmd,
                            &config,
                            event_tx.clone(),
                            vcs_backend,
                            hooks.as_ref().map(|h| h.as_ref()),
                            Some(&parallel_ctx),
                        )
                        .await;

                        match archive_result {
                            Ok(archive_revision) => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeArchived(change_id.clone()))
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
                                // Archive failed - do not merge unarchived changes
                                WorkspaceResult {
                                    change_id,
                                    workspace_name,
                                    final_revision: None,
                                    error: Some(format!("Archive failed: {}", e)),
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

                        // Individual merge: merge immediately after archive completes
                        info!("Merging {} (revision: {})", workspace_result.change_id, rev);
                        send_event(
                            &self.event_tx,
                            ParallelEvent::MergeStarted {
                                revisions: vec![rev.clone()],
                            },
                        )
                        .await;

                        match self.merge_and_resolve(std::slice::from_ref(rev)).await {
                            Ok(_) => {
                                info!(
                                    "Successfully merged {} (revision: {})",
                                    workspace_result.change_id, rev
                                );
                            }
                            Err(e) => {
                                error!(
                                    "Failed to merge {} (revision: {}): {}",
                                    workspace_result.change_id, rev, e
                                );
                                // Merge failure is critical - return error immediately
                                return Err(e);
                            }
                        }
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

    /// Merge revisions and resolve any conflicts
    async fn merge_and_resolve(&self, revisions: &[String]) -> Result<()> {
        send_event(
            &self.event_tx,
            ParallelEvent::MergeStarted {
                revisions: revisions.to_vec(),
            },
        )
        .await;

        // Attempt merge
        let merge_result = self.workspace_manager.merge_workspaces(revisions).await;

        let result = match merge_result {
            Ok(merge_revision) => {
                send_event(
                    &self.event_tx,
                    ParallelEvent::MergeCompleted {
                        revision: merge_revision,
                    },
                )
                .await;
                Ok(())
            }
            Err(VcsError::Conflict { details, .. }) => {
                // Detect conflict files
                let conflict_files =
                    conflict::detect_conflicts(self.workspace_manager.as_ref()).await?;
                send_event(
                    &self.event_tx,
                    ParallelEvent::MergeConflict {
                        files: conflict_files.clone(),
                    },
                )
                .await;

                // Attempt automatic resolution with VCS context
                conflict::resolve_conflicts_with_retry(
                    self.workspace_manager.as_ref(),
                    &self.config,
                    &self.event_tx,
                    revisions,
                    &details,
                    self.max_conflict_retries,
                )
                .await
            }
            Err(e) => Err(e.into()),
        };

        result
    }

    /// Cleanup all workspaces.
    ///
    /// Note: Currently cleanup is handled automatically in execute_group.
    /// This method is provided for manual cleanup in error recovery scenarios.
    #[allow(dead_code)] // Public API for manual cleanup in error recovery
    pub async fn cleanup(&mut self) -> Result<()> {
        self.workspace_manager
            .cleanup_all()
            .await
            .map_err(OrchestratorError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_executor_creation() {
        let config = OrchestratorConfig::default();
        let repo_root = PathBuf::from("/tmp/test-repo");
        let executor = ParallelExecutor::new(repo_root, config, None);

        assert_eq!(executor.max_conflict_retries, 3);
    }
}
