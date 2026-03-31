//! Merge operations for parallel execution.
//!
//! This module handles:
//! - Merge attempt logic (checking base branch state)
//! - Merge execution and conflict resolution
//! - Merge verification

use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;
use crate::vcs::{VcsBackend, VcsError};
use std::path::Path;
use std::path::PathBuf;

use super::conflict;
use super::events::send_event;
use super::ParallelEvent;
use super::ParallelExecutor;

/// Check if the base branch is dirty (has uncommitted changes or merge in progress).
///
/// Returns `Ok(None)` if the base branch is clean, or `Ok(Some(reason))` with a description
/// of why the base branch is dirty.
pub async fn base_dirty_reason(repo_root: &Path) -> Result<Option<String>> {
    let is_git_repo = git_commands::check_git_repo(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if !is_git_repo {
        return Ok(None);
    }

    let merge_in_progress = git_commands::is_merge_in_progress(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if merge_in_progress {
        return Ok(Some("Merge in progress (MERGE_HEAD exists)".to_string()));
    }

    let (has_changes, status) = git_commands::has_uncommitted_changes(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if has_changes {
        let trimmed = status.trim();
        let reason = if trimmed.is_empty() {
            "Working tree has uncommitted changes".to_string()
        } else {
            format!("Working tree has uncommitted changes:\n{}", trimmed)
        };
        return Ok(Some(reason));
    }

    Ok(None)
}

/// Result of a merge attempt
#[derive(Debug)]
pub enum MergeAttempt {
    /// Merge succeeded, includes the merge revision
    Merged { revision: String },
    /// Merge deferred with reason (e.g., base dirty, archive not complete)
    Deferred(String),
}

impl ParallelExecutor {
    /// Handle merge attempt and cleanup after successful archive.
    ///
    /// # Arguments
    /// * `workspace_result` - Result from archived workspace
    pub(super) async fn handle_merge_and_cleanup(
        &mut self,
        workspace_result: super::types::WorkspaceResult,
    ) -> Result<()> {
        let revisions = vec![workspace_result.workspace_name.clone()];
        let change_ids = vec![workspace_result.change_id.clone()];

        // Find workspace path for archive verification
        let workspace_path = self
            .workspace_manager
            .workspaces()
            .iter()
            .find(|workspace| workspace.name == workspace_result.workspace_name)
            .map(|workspace| workspace.path.clone());

        if let Some(path) = workspace_path {
            let archive_paths = vec![path];

            tracing::info!(
                "Merging archived {} (workspace: {})",
                workspace_result.change_id,
                workspace_result.workspace_name
            );

            match self
                .attempt_merge(&revisions, &change_ids, &archive_paths)
                .await
            {
                Ok(MergeAttempt::Merged { revision }) => {
                    // Run on_merged hook before merged status transition (MergeCompleted event)
                    if let Some(ref hooks) = self.hooks {
                        // Fetch actual task counts from change data
                        let (completed_tasks, total_tasks) =
                            match crate::openspec::list_changes_native() {
                                Ok(changes) => changes
                                    .iter()
                                    .find(|c| c.id == workspace_result.change_id)
                                    .map(|c| (c.completed_tasks, c.total_tasks))
                                    .unwrap_or((0, 0)),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to fetch task counts for on_merged hook: {}",
                                        e
                                    );
                                    (0, 0)
                                }
                            };

                        // Find workspace path
                        let workspace_path = self
                            .workspace_manager
                            .workspaces()
                            .iter()
                            .find(|w| w.name == workspace_result.workspace_name)
                            .map(|w| w.path.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let hook_context = crate::hooks::HookContext::new(
                            0, // changes_processed not easily available here
                            0, // total_changes not easily available here
                            0, // remaining_changes not easily available here
                            false,
                        )
                        .with_change(&workspace_result.change_id, completed_tasks, total_tasks)
                        .with_apply_count(0)
                        .with_parallel_context(&workspace_path, None);

                        if let Err(e) = hooks
                            .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
                            .await
                        {
                            tracing::warn!(
                                "on_merged hook failed for {}: {}",
                                workspace_result.change_id,
                                e
                            );
                        }
                    }

                    // Send MergeCompleted after on_merged hook (triggers merged status transition)
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeCompleted {
                            change_id: workspace_result.change_id.clone(),
                            revision: revision.clone(),
                        },
                    )
                    .await;

                    // Merge succeeded, cleanup workspace
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupStarted {
                            workspace: workspace_result.workspace_name.clone(),
                        },
                    )
                    .await;

                    if let Err(err) = self
                        .workspace_manager
                        .cleanup_workspace(&workspace_result.workspace_name)
                        .await
                    {
                        tracing::warn!(
                            "Failed to cleanup worktree '{}' after merge: {}",
                            workspace_result.workspace_name,
                            err
                        );
                    } else {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace_result.workspace_name.clone(),
                            },
                        )
                        .await;
                    }
                }
                Ok(MergeAttempt::Deferred(reason)) => {
                    // Merge deferred: only resolve-in-progress reasons are auto-resumable.
                    let auto_resumable = reason.contains("Resolve in progress");
                    if auto_resumable {
                        self.resolve_wait_changes
                            .insert(workspace_result.change_id.clone());
                        self.merge_wait_changes.remove(&workspace_result.change_id);
                    } else {
                        self.merge_wait_changes
                            .insert(workspace_result.change_id.clone());
                        self.resolve_wait_changes
                            .remove(&workspace_result.change_id);
                    }

                    // Update workspace status to MergeWait so it's no longer counted as active
                    self.workspace_manager.update_workspace_status(
                        &workspace_result.workspace_name,
                        crate::vcs::WorkspaceStatus::MergeWait,
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: workspace_result.change_id.clone(),
                            reason,
                            auto_resumable,
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceStatusUpdated {
                            workspace_name: workspace_result.workspace_name.clone(),
                            status: crate::vcs::WorkspaceStatus::MergeWait,
                        },
                    )
                    .await;
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to merge archived {} (workspace: {}): {}",
                        workspace_result.change_id, workspace_result.workspace_name, e
                    );
                    tracing::error!("{}", error_msg);
                    send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                }
            }
        } else {
            tracing::warn!(
                "Workspace '{}' not found after archive completion, skipping merge",
                workspace_result.workspace_name
            );
        }

        Ok(())
    }

    pub(super) async fn attempt_merge(
        &self,
        revisions: &[String],
        change_ids: &[String],
        archive_paths: &[PathBuf],
    ) -> Result<MergeAttempt> {
        use crate::execution::archive::is_archive_commit_complete;

        let _merge_guard = super::global_merge_lock().lock().await;

        let auto_resolve_count = self
            .auto_resolve_count
            .load(std::sync::atomic::Ordering::SeqCst);
        let manual_resolve_count = self
            .manual_resolve_count
            .as_ref()
            .map(|counter| counter.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap_or(0);
        if auto_resolve_count.saturating_add(manual_resolve_count) > 0 {
            return Ok(MergeAttempt::Deferred(
                "Resolve in progress for another change".to_string(),
            ));
        }

        if let Some(reason) = base_dirty_reason(&self.repo_root).await? {
            return Ok(MergeAttempt::Deferred(reason));
        }

        if change_ids.len() != archive_paths.len() {
            return Err(OrchestratorError::GitCommand(format!(
                "Expected {} archive paths for {} changes",
                change_ids.len(),
                archive_paths.len()
            )));
        }

        // Verify that all changes are actually archived before attempting merge
        for (change_id, archive_path) in change_ids.iter().zip(archive_paths.iter()) {
            match is_archive_commit_complete(change_id, Some(archive_path)).await {
                Ok(true) => {
                    // Archive is complete, continue
                }
                Ok(false) => {
                    // Archive is incomplete, defer merge with detailed reason
                    let reason = format!(
                        "Archive incomplete for '{}': worktree may be dirty, openspec/changes/{} may still exist, or archive entry may be missing",
                        change_id, change_id
                    );
                    tracing::warn!("{}", reason);
                    return Ok(MergeAttempt::Deferred(reason));
                }
                Err(e) => {
                    let reason = format!(
                        "Failed to verify archive completion for '{}': {}",
                        change_id, e
                    );
                    tracing::warn!("{}", reason);
                    return Ok(MergeAttempt::Deferred(reason));
                }
            }
        }

        let revision = self.merge_and_resolve(revisions, change_ids).await?;
        Ok(MergeAttempt::Merged { revision })
    }

    pub async fn resolve_merge_for_change(&mut self, change_id: &str) -> Result<()> {
        let workspace_info = self
            .workspace_manager
            .find_existing_workspace(change_id)
            .await
            .map_err(OrchestratorError::from_vcs_error)?
            .ok_or_else(|| OrchestratorError::ChangeNotFound(change_id.to_string()))?;
        let workspace = self
            .workspace_manager
            .reuse_workspace(&workspace_info)
            .await
            .map_err(OrchestratorError::from_vcs_error)?;

        let revisions = vec![workspace.name.clone()];
        let change_ids = vec![change_id.to_string()];

        // ResolveStarted event will be sent from within conflict resolution functions with command string

        let archive_paths = vec![workspace.path.clone()];
        match self
            .attempt_merge(&revisions, &change_ids, &archive_paths)
            .await?
        {
            MergeAttempt::Merged { revision } => {
                // Run on_merged hook before merged status transition (MergeCompleted event)
                if let Some(ref hooks) = self.hooks {
                    // Fetch actual task counts from change data
                    let (completed_tasks, total_tasks) =
                        match crate::openspec::list_changes_native() {
                            Ok(changes) => changes
                                .iter()
                                .find(|c| c.id == *change_id)
                                .map(|c| (c.completed_tasks, c.total_tasks))
                                .unwrap_or((0, 0)),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to fetch task counts for on_merged hook: {}",
                                    e
                                );
                                (0, 0)
                            }
                        };

                    let hook_context = crate::hooks::HookContext::new(
                        0, // changes_processed not easily available here
                        0, // total_changes not easily available here
                        0, // remaining_changes not easily available here
                        false,
                    )
                    .with_change(change_id, completed_tasks, total_tasks)
                    .with_apply_count(0)
                    .with_parallel_context(&workspace.path.to_string_lossy(), None);

                    if let Err(e) = hooks
                        .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
                        .await
                    {
                        tracing::warn!("on_merged hook failed for {}: {}", change_id, e);
                    }
                }

                // Send MergeCompleted after on_merged hook (triggers merged status transition)
                send_event(
                    &self.event_tx,
                    ParallelEvent::MergeCompleted {
                        change_id: change_id.to_string(),
                        revision: revision.clone(),
                    },
                )
                .await;

                send_event(
                    &self.event_tx,
                    ParallelEvent::CleanupStarted {
                        workspace: workspace.name.clone(),
                    },
                )
                .await;
                if let Err(err) = self
                    .workspace_manager
                    .cleanup_workspace(&workspace.name)
                    .await
                {
                    tracing::warn!(
                        "Failed to cleanup worktree '{}' after merge: {}",
                        workspace.name,
                        err
                    );
                } else {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupCompleted {
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;
                }

                // Send ResolveCompleted to update TUI status
                send_event(
                    &self.event_tx,
                    ParallelEvent::ResolveCompleted {
                        change_id: change_id.to_string(),
                        worktree_change_ids: None,
                    },
                )
                .await;

                // A resolve just completed: auto-resumable deferred changes may now be
                // unblocked (the base was dirty because this resolve was in progress).
                self.retry_deferred_merges().await;

                Ok(())
            }
            MergeAttempt::Deferred(reason) => {
                let auto_resumable = reason.contains("Resolve in progress");
                if auto_resumable {
                    // Auto-resumable: another merge/resolve is in progress.
                    // Track as deferred so retry_deferred_merges picks it up.
                    self.resolve_wait_changes.insert(change_id.to_string());

                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: change_id.to_string(),
                            reason: reason.clone(),
                            auto_resumable: true,
                        },
                    )
                    .await;
                } else {
                    // Manual intervention required (e.g. uncommitted changes).
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ResolveFailed {
                            change_id: change_id.to_string(),
                            error: reason.clone(),
                        },
                    )
                    .await;
                }
                Err(OrchestratorError::GitCommand(reason))
            }
        }
    }

    /// Merge revisions and resolve any conflicts
    pub(super) async fn merge_and_resolve(
        &self,
        revisions: &[String],
        change_ids: &[String],
    ) -> Result<String> {
        let change_ids_vec = change_ids.to_vec();
        let shared_stagger_state = self.shared_stagger_state.clone();
        let auto_resolve_count = self.auto_resolve_count.clone();
        self.merge_and_resolve_with(revisions, change_ids, |revisions, details| {
            let change_ids_clone = change_ids_vec.clone();
            let shared_stagger_state_clone = shared_stagger_state.clone();
            let auto_resolve_count_clone = auto_resolve_count.clone();
            async move {
                conflict::resolve_conflicts_with_retry(
                    self.workspace_manager.as_ref(),
                    &self.config,
                    &self.event_tx,
                    &revisions,
                    &change_ids_clone,
                    &details,
                    self.max_conflict_retries,
                    shared_stagger_state_clone,
                    auto_resolve_count_clone,
                )
                .await
            }
        })
        .await
    }

    pub(super) async fn merge_and_resolve_with<'a, F, Fut>(
        &'a self,
        revisions: &'a [String],
        change_ids: &'a [String],
        mut resolve_conflicts: F,
    ) -> Result<String>
    where
        F: FnMut(Vec<String>, String) -> Fut,
        Fut: std::future::Future<Output = Result<()>> + Send + 'a,
    {
        let max_attempts = self.max_conflict_retries.max(1);

        send_event(
            &self.event_tx,
            ParallelEvent::MergeStarted {
                revisions: revisions.to_vec(),
            },
        )
        .await;

        if matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            let base_revision = self.workspace_manager.get_current_revision().await?;
            let target_branch = self.workspace_manager.original_branch().ok_or_else(|| {
                OrchestratorError::GitCommand("Original branch not initialized".to_string())
            })?;

            if change_ids.len() != revisions.len() {
                return Err(OrchestratorError::GitCommand(format!(
                    "Expected {} change_ids for {} revisions",
                    revisions.len(),
                    change_ids.len()
                )));
            }

            conflict::resolve_merges_with_retry(conflict::ResolveMergesWithRetryArgs {
                workspace_manager: self.workspace_manager.as_ref(),
                config: &self.config,
                event_tx: &self.event_tx,
                revisions,
                change_ids,
                target_branch: target_branch.as_str(),
                base_revision: base_revision.as_str(),
                max_retries: max_attempts,
                shared_stagger_state: self.shared_stagger_state.clone(),
                auto_resolve_count: self.auto_resolve_count.clone(),
            })
            .await?;

            self.verify_merge_commits(&base_revision, &target_branch, change_ids)
                .await?;

            let merge_revision = self.workspace_manager.get_current_revision().await?;

            // Note: MergeCompleted event is sent by the caller after running on_merged hook.
            // This ensures on_merged executes before the merged status transition.
            return Ok(merge_revision);
        }

        for attempt in 1..=max_attempts {
            tracing::info!(
                "Merge attempt {}/{} for revisions: {}",
                attempt,
                max_attempts,
                revisions.join(", ")
            );

            let merge_result = self.workspace_manager.merge_workspaces(revisions).await;

            match merge_result {
                Ok(merge_revision) => {
                    if attempt > 1 {
                        tracing::info!("Merge succeeded after {} attempts", attempt);
                    }

                    // Note: MergeCompleted event is sent by the caller after running on_merged hook.
                    // This ensures on_merged executes before the merged status transition.
                    return Ok(merge_revision);
                }
                Err(VcsError::Conflict { details, .. }) => {
                    let conflict_files =
                        conflict::detect_conflicts(self.workspace_manager.as_ref()).await?;
                    tracing::warn!(
                        "Merge conflict detected on attempt {}/{}",
                        attempt,
                        max_attempts
                    );
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeConflict {
                            files: conflict_files,
                        },
                    )
                    .await;

                    if attempt >= max_attempts {
                        let error_msg = format!(
                            "Merge conflict unresolved after {} attempts: {}",
                            max_attempts, details
                        );
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ConflictResolutionFailed {
                                error: error_msg.clone(),
                            },
                        )
                        .await;

                        // Send ResolveFailed for each change_id to update TUI status
                        for change_id in change_ids {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: change_id.to_string(),
                                    error: error_msg.clone(),
                                },
                            )
                            .await;
                        }

                        return Err(OrchestratorError::from_vcs_error(VcsError::Conflict {
                            backend: self.workspace_manager.backend_type(),
                            details: error_msg,
                        }));
                    }

                    tracing::info!(
                        "Resolving merge conflicts (attempt {}/{}).",
                        attempt,
                        max_attempts
                    );

                    // ResolveStarted event will be sent from within conflict resolution functions with command string

                    if let Err(err) = resolve_conflicts(revisions.to_vec(), details.clone()).await {
                        tracing::warn!(
                            "Conflict resolution failed on attempt {}/{}: {}",
                            attempt,
                            max_attempts,
                            err
                        );

                        // Send ResolveFailed for each change_id to update TUI status
                        for change_id in change_ids {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: change_id.to_string(),
                                    error: err.to_string(),
                                },
                            )
                            .await;
                        }

                        return Err(err);
                    }
                    tracing::info!("Conflict resolution completed, retrying merge");

                    // Note: ResolveCompleted will be sent when the merge succeeds
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Fallback: should not normally reach here
        Err(OrchestratorError::GitCommand(
            "Merge failed: exhausted all attempts without success or error".to_string(),
        ))
    }

    pub(super) async fn verify_merge_commits(
        &self,
        base_revision: &str,
        _target_branch: &str,
        change_ids: &[String],
    ) -> Result<()> {
        if matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            let repo_root = self.workspace_manager.repo_root();
            let missing =
                git_commands::missing_merge_commits_since(repo_root, base_revision, change_ids)
                    .await
                    .map_err(OrchestratorError::from_vcs_error)?;
            if !missing.is_empty() {
                return Err(OrchestratorError::GitCommand(format!(
                    "Missing merge commit message containing change_id(s): {}",
                    missing.join(", ")
                )));
            }
        }

        Ok(())
    }
}

pub async fn resolve_deferred_merge(
    repo_root: PathBuf,
    config: crate::config::OrchestratorConfig,
    change_id: &str,
) -> Result<()> {
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    executor.resolve_merge_for_change(change_id).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_resolve_in_progress_reason_is_auto_resumable() {
        let auto_resumable =
            "Resolve in progress for another change".contains("Resolve in progress");
        assert!(auto_resumable);
    }

    #[test]
    fn test_merge_in_progress_reason_is_not_auto_resumable() {
        let auto_resumable =
            "Merge in progress (MERGE_HEAD exists)".contains("Resolve in progress");
        assert!(!auto_resumable);
    }

    #[test]
    fn test_uncommitted_changes_reason_is_not_auto_resumable() {
        let auto_resumable = "Working tree has uncommitted changes".contains("Resolve in progress");
        assert!(!auto_resumable);
    }

    #[test]
    fn test_archive_incomplete_reason_is_not_auto_resumable() {
        let auto_resumable = "Archive incomplete for 'change-a': worktree may be dirty"
            .contains("Resolve in progress");
        assert!(!auto_resumable);
    }
}
