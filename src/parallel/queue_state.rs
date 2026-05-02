//! Queue state management and dispatch coordination for [`super::ParallelExecutor`].
//!
//! This module handles queue monitoring, debounce logic, dependency checking,
//! and the reanalysis/dispatch cycle that drives the parallel execution scheduler.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::error::{OrchestratorError, Result};
use crate::events::{ExecutionEvent, LogEntry};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueReconciliationDiagnosticLevel {
    Info,
    Warn,
}

use super::acceptance_state::delete_acceptance_state;
use super::cleanup::WorkspaceCleanupGuard;
use super::dynamic_queue::ReanalysisReason;
use super::events::send_event;
use super::{MergeResult, ParallelEvent, ParallelExecutor, WorkspaceResult};

impl ParallelExecutor {
    /// Check if debounce period has elapsed for queue changes.
    ///
    /// Returns `true` if:
    /// - No recent queue changes, OR
    /// - 10 seconds have passed since the last queue change
    ///
    /// This prevents immediate re-analysis when the queue changes, giving time for
    /// multiple changes to be queued before triggering expensive re-analysis.
    ///
    /// Note: This is now separated from slot availability check. Re-analysis can
    /// proceed even when available_slots == 0, and the next dispatch will happen
    /// when slots become available.
    pub async fn should_reanalyze(&self, bypass_debounce: bool) -> bool {
        super::dynamic_queue::should_reanalyze_queue(&self.last_queue_change_at, bypass_debounce)
            .await
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
    }

    pub(super) fn sync_resolve_wait_from_shared_state_nonblocking(&mut self) {
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(guard) = shared.try_read() {
                self.resolve_wait_changes = guard.resolve_wait_change_ids().into_iter().collect();
            }
        }
    }

    pub(super) async fn clear_resolve_wait_intent_for_success(&mut self, change_id: &str) {
        self.resolve_wait_changes.remove(change_id);
        self.last_dispatched_resolve_wait_changes.remove(change_id);
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.clear_resolve_wait_intent(change_id);
        }
    }

    pub(super) async fn mark_deferred_merge_completed_in_shared_state(
        &mut self,
        change_id: &str,
        revision: &str,
    ) {
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
                change_id: change_id.to_string(),
                revision: revision.to_string(),
            });
        }
    }

    pub(super) fn trigger_resolve_wait_retry_dispatch(&mut self) {
        self.resolve_wait_retry_triggered = true;
    }

    pub(super) fn should_dispatch_resolve_wait_retry(&self) -> bool {
        if self.resolve_wait_changes.is_empty() {
            return false;
        }

        self.resolve_wait_retry_triggered
            || self.last_dispatched_resolve_wait_changes != self.resolve_wait_changes
    }

    pub(super) async fn maybe_dispatch_resolve_wait_retry(&mut self) {
        if !self.should_dispatch_resolve_wait_retry() {
            return;
        }

        self.retry_deferred_merges().await;
        self.last_dispatched_resolve_wait_changes = self.resolve_wait_changes.clone();
        self.resolve_wait_retry_triggered = false;
    }

    pub(crate) fn has_resolve_wait(&self) -> bool {
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(guard) = shared.try_read() {
                return !guard.resolve_wait_change_ids().is_empty();
            }
        }
        !self.resolve_wait_changes.is_empty()
    }

    #[allow(dead_code)]
    pub(super) fn skip_reason_for_change(&self, change_id: &str) -> Option<String> {
        // Only skip changes with failed dependencies (not merge-wait dependencies).
        // Merge-wait dependencies are handled as blocked/queued status via dependency resolution.
        if let Some(failed_dep) = self.failed_tracker.should_skip(change_id) {
            return Some(format!("Dependency '{}' failed", failed_dep));
        }
        None
    }

    /// Check if a dependency is resolved (merged to base branch).
    ///
    /// A dependency is considered resolved if its archive commit is present in the base branch.
    /// This indicates that the dependency's artifacts are available for dependent changes.
    pub(super) async fn is_dependency_resolved(&self, dep_id: &str) -> Result<bool> {
        let original_branch = self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
            .map_err(OrchestratorError::from_vcs_error)?;

        // Check if the archive commit for this dependency exists in the base branch
        match crate::execution::state::is_merged_to_base(dep_id, &self.repo_root, &original_branch)
            .await
        {
            Ok(is_merged) => Ok(is_merged),
            Err(e) => {
                warn!(
                    "Failed to check if dependency '{}' is merged to base: {}, assuming not resolved",
                    dep_id, e
                );
                Ok(false)
            }
        }
    }

    async fn is_change_already_merged_to_base(&self, change_id: &str) -> bool {
        let original_branch = match self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
        {
            Ok(branch) => branch,
            Err(error) => {
                warn!(
                    "Failed to determine base branch before stale deferred retry check for '{}': {}",
                    change_id, error
                );
                return false;
            }
        };

        match crate::execution::state::is_merged_to_base(
            change_id,
            &self.repo_root,
            &original_branch,
        )
        .await
        {
            Ok(true) => true,
            Ok(false) => false,
            Err(error) => {
                warn!(
                    "Failed to check whether deferred retry '{}' is already merged to base: {}",
                    change_id, error
                );
                false
            }
        }
    }

    /// Calculate available execution slots accounting for in-flight changes and resolves.
    ///
    /// # Arguments
    /// * `max_parallelism` - Maximum number of concurrent slots
    /// * `in_flight` - Set of currently executing changes
    ///
    /// # Returns
    /// Number of available slots for new dispatches
    pub(super) fn manual_resolve_active(&self) -> usize {
        self.manual_resolve_count
            .as_ref()
            .map(|counter| counter.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub(super) fn calculate_available_slots(
        &self,
        max_parallelism: usize,
        in_flight: &HashSet<String>,
    ) -> usize {
        let manual_resolve_count = self.manual_resolve_active();
        let auto_resolve_count = self
            .auto_resolve_count
            .load(std::sync::atomic::Ordering::Relaxed);

        max_parallelism
            .saturating_sub(in_flight.len())
            .saturating_sub(manual_resolve_count)
            .saturating_sub(auto_resolve_count)
    }

    /// Filter queued changes to remove those with failed dependencies.
    ///
    /// # Arguments
    /// * `queued` - List of queued changes to filter
    ///
    /// # Returns
    /// Tuple of (executable_changes, skipped_changes_with_reasons)
    pub(super) fn filter_executable_changes(
        &self,
        queued: &[crate::openspec::Change],
    ) -> (Vec<crate::openspec::Change>, Vec<(String, String)>) {
        let mut executable_changes: Vec<crate::openspec::Change> = Vec::new();
        let mut skipped_changes: Vec<(String, String)> = Vec::new();

        for change in queued {
            if let Some(failed_dep) = self.failed_tracker.should_skip(&change.id) {
                let reason = format!("Dependency '{}' failed", failed_dep);
                warn!(
                    "Skipping change-{} because dependency change-{} failed",
                    change.id, failed_dep
                );
                skipped_changes.push((change.id.clone(), reason));
            } else {
                executable_changes.push(change.clone());
            }
        }

        (executable_changes, skipped_changes)
    }

    /// Select changes to dispatch based on order, available slots, and dependency resolution.
    ///
    /// # Arguments
    /// * `analysis_result` - Result from dependency analysis
    /// * `available_slots` - Number of available execution slots
    ///
    /// # Returns
    /// Vector of selected change IDs ready for dispatch
    pub(super) async fn select_changes_for_dispatch(
        &mut self,
        analysis_result: &crate::analyzer::AnalysisResult,
        available_slots: usize,
    ) -> Vec<String> {
        let mut selected_changes: Vec<String> = Vec::new();

        for change_id in &analysis_result.order {
            // Check if change has unresolved dependencies
            if let Some(deps) = analysis_result.dependencies.get(change_id) {
                let mut unresolved_deps = Vec::new();
                for dep_id in deps {
                    match self.is_dependency_resolved(dep_id).await {
                        Ok(true) => {}
                        Ok(false) => unresolved_deps.push(dep_id.clone()),
                        Err(e) => {
                            error!(
                                "Failed to evaluate dependency resolution for '{}' (dependency '{}'): {}",
                                change_id, dep_id, e
                            );
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Error {
                                    message: format!(
                                        "Failed to evaluate dependency resolution for '{}' (dependency '{}'): {}",
                                        change_id, dep_id, e
                                    ),
                                },
                            )
                            .await;
                            unresolved_deps.push(dep_id.clone());
                        }
                    }
                }

                if !unresolved_deps.is_empty() {
                    info!(
                        "Change '{}' blocked: waiting for dependencies {:?}",
                        change_id, unresolved_deps
                    );
                    // Track this change as blocked
                    self.previously_blocked_changes.insert(change_id.clone());
                    // Send DependencyBlocked event
                    send_event(
                        &self.event_tx,
                        ParallelEvent::DependencyBlocked {
                            change_id: change_id.clone(),
                            dependency_ids: unresolved_deps,
                        },
                    )
                    .await;
                    continue;
                }
            }

            // Check if this change was previously blocked and is now resolved
            if self.previously_blocked_changes.contains(change_id) {
                info!(
                    "Change '{}' dependencies resolved, forcing fresh workspace recreation",
                    change_id
                );
                self.previously_blocked_changes.remove(change_id);
                self.force_recreate_worktree.insert(change_id.clone());
                // Send DependencyResolved event
                send_event(
                    &self.event_tx,
                    ParallelEvent::DependencyResolved {
                        change_id: change_id.clone(),
                    },
                )
                .await;
            }

            if selected_changes.len() < available_slots {
                selected_changes.push(change_id.clone());
            }
        }

        selected_changes
    }

    /// Handle completion of a workspace task (apply+archive).
    ///
    /// This processes success/failure, attempts merge if archived, and triggers cleanup.
    ///
    /// # Arguments
    /// * `workspace_result` - Result from completed workspace task
    /// * `max_parallelism` - Maximum parallelism for logging
    /// * `in_flight` - Set to update (remove completed change)
    /// * `cleanup_guard` - Guard for workspace cleanup tracking
    pub(super) async fn handle_workspace_completion(
        &mut self,
        workspace_result: WorkspaceResult,
        max_parallelism: usize,
        in_flight: &mut HashSet<String>,
        merge_result_tx: &mpsc::Sender<MergeResult>,
    ) {
        // Remove from in-flight
        in_flight.remove(&workspace_result.change_id);

        // Clean up kill token registry
        if let Some(ref queue) = self.dynamic_queue {
            queue
                .unregister_kill_token(&workspace_result.change_id)
                .await;
        }

        info!(
            "Task completed: change='{}', in_flight={}, available_slots={}, error={:?}",
            workspace_result.change_id,
            in_flight.len(),
            max_parallelism.saturating_sub(in_flight.len()),
            workspace_result.error
        );

        // Handle result (failure, rejection, or success)
        if let Some(error) = &workspace_result.error {
            error!("Change '{}' failed: {}", workspace_result.change_id, error);
            self.failed_tracker.mark_failed(&workspace_result.change_id);
            send_event(
                &self.event_tx,
                ParallelEvent::ProcessingError {
                    id: workspace_result.change_id.clone(),
                    error: error.clone(),
                },
            )
            .await;

            // Rejection review failure releases the rejecting lane. If any archived rows are
            // waiting in ResolveWait due to that blocker, retry them immediately.
            self.retry_deferred_merges().await;
        } else if let Some(reason) = &workspace_result.rejected {
            info!(
                "Change '{}' rejected after acceptance blocker: {}",
                workspace_result.change_id, reason
            );
            send_event(
                &self.event_tx,
                ParallelEvent::ChangeRejected {
                    change_id: workspace_result.change_id.clone(),
                    reason: reason.clone(),
                },
            )
            .await;
            // Rejected flow is terminal after base-side REJECTED marker commit
            // and should not proceed to merge. Ensure preserved workspace is cleaned up.
            let workspace_path = self
                .workspace_manager
                .find_existing_workspace(&workspace_result.change_id)
                .await
                .ok()
                .flatten()
                .map(|info| info.path);
            if let Err(e) = self
                .workspace_manager
                .cleanup_workspace(&workspace_result.workspace_name)
                .await
            {
                error!(
                    "Failed to cleanup rejected workspace '{}' for change '{}': {}",
                    workspace_result.workspace_name, workspace_result.change_id, e
                );
            } else if let Some(workspace_path) = workspace_path {
                if let Err(err) = delete_acceptance_state(&workspace_path) {
                    warn!(
                        "Failed to delete acceptance state for rejected change '{}': {}",
                        workspace_result.change_id, err
                    );
                }
            }

            // Rejection review completion (confirm) also releases the rejecting lane;
            // retry deferred merges so ResolveWait rows are not stranded.
            self.retry_deferred_merges().await;
        } else {
            info!(
                "Change '{}' completed successfully",
                workspace_result.change_id
            );

            // Run merge+cleanup in background and report result back to scheduler loop.
            if workspace_result.final_revision.is_some() {
                self.spawn_merge_task(workspace_result, merge_result_tx.clone());
            }
        }
    }

    fn spawn_merge_task(
        &self,
        workspace_result: WorkspaceResult,
        merge_result_tx: mpsc::Sender<MergeResult>,
    ) {
        let mut merge_executor = ParallelExecutor::new(
            self.repo_root.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        merge_executor.max_conflict_retries = self.max_conflict_retries;
        merge_executor.shared_stagger_state = self.shared_stagger_state.clone();
        merge_executor.auto_resolve_count = self.auto_resolve_count.clone();
        merge_executor.pending_merge_count = self.pending_merge_count.clone();
        merge_executor.cancel_token = self.cancel_token.clone();
        merge_executor.manual_resolve_count = self.manual_resolve_count.clone();
        merge_executor.hooks = self.hooks.clone();

        self.pending_merge_count.fetch_add(1, Ordering::Relaxed);

        tokio::spawn(async move {
            let change_id = workspace_result.change_id.clone();
            let workspace_name = workspace_result.workspace_name.clone();
            let outcome = merge_executor
                .handle_merge_and_cleanup(workspace_result)
                .await
                .map_err(|error| error.to_string());

            if let Err(send_error) = merge_result_tx
                .send(MergeResult {
                    change_id,
                    workspace_name,
                    outcome,
                })
                .await
            {
                warn!(
                    "Failed to send merge result to scheduler loop: {}",
                    send_error
                );
            }
        });
    }

    pub(super) async fn handle_merge_result(&mut self, merge_result: MergeResult) {
        self.pending_merge_count.fetch_sub(1, Ordering::Relaxed);

        match merge_result.outcome {
            Ok(()) => {
                info!(
                    "Background merge task completed successfully for '{}'",
                    merge_result.change_id
                );
                self.retry_deferred_merges().await;
            }
            Err(error) => {
                error!(
                    "Background merge task failed for '{}' (workspace '{}'): {}",
                    merge_result.change_id, merge_result.workspace_name, error
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Error {
                        message: format!(
                            "Background merge failed for '{}' (workspace '{}'): {}",
                            merge_result.change_id, merge_result.workspace_name, error
                        ),
                    },
                )
                .await;
            }
        }
    }

    /// Retry merge for all ResolveWait changes (auto-resumable deferred merges).
    ///
    /// Called after a merge or resolve succeeds, since the previously blocking
    /// condition (dirty base, another merge in progress) may now be resolved.
    /// For each change in `resolve_wait_changes`:
    /// - If merge now succeeds → send `MergeCompleted`, run hook, cleanup workspace.
    /// - If still deferred and auto-resumable → keep in ResolveWait.
    /// - If still deferred and manual action required → move to MergeWait.
    /// - On error → log and keep in ResolveWait for the next retry opportunity.
    pub(super) async fn retry_deferred_merges(&mut self) {
        if let Some(shared) = &self.shared_orchestrator_state {
            let guard = shared.read().await;
            self.resolve_wait_changes = guard.resolve_wait_change_ids().into_iter().collect();
        }

        if self.resolve_wait_changes.is_empty() {
            return;
        }

        let deferred: Vec<String> = self.resolve_wait_changes.iter().cloned().collect();

        for change_id in deferred {
            if self.is_change_already_merged_to_base(&change_id).await {
                info!(
                    "Skipping stale deferred merge retry for '{}' because it is already merged to base",
                    change_id
                );
                self.clear_resolve_wait_intent_for_success(&change_id).await;
                continue;
            }

            // Locate the preserved workspace for this change.
            let workspace_info = match self
                .workspace_manager
                .find_existing_workspace(&change_id)
                .await
            {
                Ok(Some(ws)) => ws,
                Ok(None) => {
                    warn!(
                        "No workspace found for deferred change '{}', skipping retry",
                        change_id
                    );
                    // Remove from deferred set; the workspace is gone, nothing to retry.
                    self.clear_resolve_wait_intent_for_success(&change_id).await;
                    continue;
                }
                Err(e) => {
                    warn!(
                        "Failed to find workspace for deferred change '{}': {}",
                        change_id, e
                    );
                    continue;
                }
            };

            info!(
                "Retrying deferred merge for '{}' (workspace: {})",
                change_id, workspace_info.workspace_name
            );

            let revisions = vec![workspace_info.workspace_name.clone()];
            let change_ids = vec![change_id.clone()];
            let archive_paths = vec![workspace_info.path.clone()];

            match self
                .attempt_merge(&revisions, &change_ids, &archive_paths)
                .await
            {
                Ok(super::merge::MergeAttempt::Merged { revision }) => {
                    info!("Deferred merge succeeded for '{}' on retry", change_id);
                    self.clear_resolve_wait_intent_for_success(&change_id).await;

                    // Run on_merged hook before merged status transition (MergeCompleted event).
                    if let Some(ref hooks) = self.hooks {
                        let (completed_tasks, total_tasks) =
                            match crate::openspec::list_changes_native() {
                                Ok(changes) => changes
                                    .iter()
                                    .find(|c| c.id == change_id)
                                    .map(|c| (c.completed_tasks, c.total_tasks))
                                    .unwrap_or((0, 0)),
                                Err(e) => {
                                    warn!("Failed to fetch task counts for on_merged hook: {}", e);
                                    (0, 0)
                                }
                            };
                        let ws_path = workspace_info.path.to_string_lossy().to_string();
                        let hook_ctx = crate::hooks::HookContext::new(0, 0, 0, false)
                            .with_change(&change_id, completed_tasks, total_tasks)
                            .with_apply_count(0)
                            .with_parallel_context(&ws_path, None);
                        if let Err(e) = hooks
                            .run_hook(crate::hooks::HookType::OnMerged, &hook_ctx)
                            .await
                        {
                            warn!(
                                "on_merged hook failed for deferred retry of '{}': {}",
                                change_id, e
                            );
                        }
                    }

                    self.mark_deferred_merge_completed_in_shared_state(&change_id, &revision)
                        .await;

                    // Send MergeCompleted after on_merged hook (triggers merged status transition)
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeCompleted {
                            change_id: change_id.clone(),
                            revision: revision.clone(),
                        },
                    )
                    .await;

                    // Clean up workspace.
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupStarted {
                            workspace: workspace_info.workspace_name.clone(),
                        },
                    )
                    .await;
                    if let Err(e) = self
                        .workspace_manager
                        .cleanup_workspace(&workspace_info.workspace_name)
                        .await
                    {
                        warn!(
                            "Failed to cleanup workspace '{}' after deferred merge retry: {}",
                            workspace_info.workspace_name, e
                        );
                    } else {
                        if let Err(err) = delete_acceptance_state(&workspace_info.path) {
                            warn!(
                                "Failed to delete acceptance state for '{}' after deferred merge cleanup: {}",
                                change_id,
                                err
                            );
                        }
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace_info.workspace_name.clone(),
                            },
                        )
                        .await;
                    }
                }
                Ok(super::merge::MergeAttempt::Deferred(deferred)) => {
                    info!(
                        "Deferred merge still blocked for '{}': {} (auto_resumable={})",
                        change_id, deferred.reason, deferred.auto_resumable
                    );
                    if deferred.auto_resumable {
                        self.merge_wait_changes.remove(&change_id);
                    } else {
                        self.resolve_wait_changes.remove(&change_id);
                        self.merge_wait_changes.insert(change_id.clone());
                    }
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: change_id.clone(),
                            reason: deferred.reason,
                            auto_resumable: deferred.auto_resumable,
                        },
                    )
                    .await;
                }
                Err(e) => {
                    error!("Deferred merge retry error for '{}': {}", change_id, e);
                    // Keep in deferred set; another merge/resolve completion will trigger again.
                }
            }
        }
    }

    /// Check dynamic queue for newly added changes and update queued list.
    ///
    /// # Arguments
    /// * `queued` - Mutable reference to queued changes list
    /// * `in_flight` - Set of currently executing changes
    /// * `reanalysis_reason` - Mutable reference to reanalysis reason tracking
    ///
    /// # Returns
    /// `true` if queue changed and reanalysis should be triggered
    pub(super) async fn check_dynamic_queue_and_add_changes(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
        reanalysis_reason: &mut ReanalysisReason,
    ) -> bool {
        if let Some(queue) = &self.dynamic_queue {
            let mut queue_changed = false;
            while let Some(dynamic_id) = queue.pop().await {
                if !queued.iter().any(|c| c.id == dynamic_id) && !in_flight.contains(&dynamic_id) {
                    match crate::openspec::list_changes_native() {
                        Ok(all_changes) => {
                            if let Some(new_change) =
                                all_changes.into_iter().find(|c| c.id == dynamic_id)
                            {
                                info!("Dynamically adding change to execution: {}", dynamic_id);
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(LogEntry::info(format!(
                                        "Dynamically added to parallel execution: {}",
                                        dynamic_id
                                    ))),
                                )
                                .await;
                                queued.push(new_change);
                                queue_changed = true;
                            } else {
                                warn!(
                                    "Dynamically added change '{}' not found in openspec",
                                    dynamic_id
                                );
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(LogEntry::warn(format!(
                                        "Queue reconciliation pending for '{}': candidate_not_found",
                                        dynamic_id
                                    ))),
                                )
                                .await;
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to load dynamically added change '{}': {}",
                                dynamic_id, e
                            );
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Log(LogEntry::warn(format!(
                                    "Queue reconciliation pending for '{}': candidate_load_failed ({})",
                                    dynamic_id, e
                                ))),
                            )
                            .await;
                        }
                    }
                } else if in_flight.contains(&dynamic_id) {
                    debug!(
                        "Ignoring dynamic queue entry '{}' because it is already in-flight",
                        dynamic_id
                    );
                } else {
                    debug!(
                        "Ignoring dynamic queue entry '{}' because it is already queued",
                        dynamic_id
                    );
                }
            }

            // Update queue change timestamp if items were added
            if queue_changed {
                let mut last_change = self.last_queue_change_at.lock().await;
                *last_change = Some(std::time::Instant::now());
                *reanalysis_reason = ReanalysisReason::QueueNotification;
                info!("Queue changed, scheduler state now requires re-analysis");
            }
            queue_changed
        } else {
            false
        }
    }

    pub(super) fn should_emit_queue_reconciliation_diagnostic(
        &mut self,
        change_id: &str,
        reason: &str,
    ) -> bool {
        self.queue_reconciliation_diagnostics_seen
            .insert((change_id.to_string(), reason.to_string()))
    }

    async fn emit_queue_reconciliation_diagnostic(
        &mut self,
        level: QueueReconciliationDiagnosticLevel,
        change_id: &str,
        reason: &str,
    ) {
        if !self.should_emit_queue_reconciliation_diagnostic(change_id, reason) {
            debug!(
                change_id,
                reason, "Suppressing repeated queue reconciliation diagnostic"
            );
            return;
        }

        let message = match level {
            QueueReconciliationDiagnosticLevel::Info => LogEntry::info(format!(
                "Queue reconciliation deferred for '{}': {}",
                change_id, reason
            )),
            QueueReconciliationDiagnosticLevel::Warn => LogEntry::warn(format!(
                "Queue reconciliation pending for '{}': {}",
                change_id, reason
            )),
        };

        send_event(&self.event_tx, ParallelEvent::Log(message)).await;
    }

    pub(super) async fn reconcile_queued_candidates_from_shared_state(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
    ) -> usize {
        let Some(shared_state) = &self.shared_orchestrator_state else {
            return 0;
        };

        let (queued_intent_ids, active_ids_from_reducer) = match shared_state.try_read() {
            Ok(state) => (state.queued_change_ids(), state.active_change_ids()),
            Err(_) => return 0,
        };

        if queued_intent_ids.is_empty() {
            return 0;
        }

        let reducer_active_set: std::collections::HashSet<String> =
            active_ids_from_reducer.into_iter().collect();

        let mut known_changes = match crate::openspec::list_changes_native() {
            Ok(changes) => changes,
            Err(e) => {
                warn!(
                    "Failed to load OpenSpec changes during queue reconciliation: {}",
                    e
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(format!(
                        "Queue reconciliation skipped: failed_to_load_changes ({})",
                        e
                    ))),
                )
                .await;
                return 0;
            }
        };

        let mut known_by_id: std::collections::HashMap<String, crate::openspec::Change> =
            known_changes
                .drain(..)
                .map(|change| (change.id.clone(), change))
                .collect();

        let mut added = 0usize;

        for queued_id in queued_intent_ids {
            if queued.iter().any(|change| change.id == queued_id) {
                continue;
            }
            if in_flight.contains(&queued_id) || reducer_active_set.contains(&queued_id) {
                self.emit_queue_reconciliation_diagnostic(
                    QueueReconciliationDiagnosticLevel::Info,
                    &queued_id,
                    "already_active",
                )
                .await;
                continue;
            }

            match known_by_id.remove(&queued_id) {
                Some(change) => {
                    info!(
                        "Queue reconciliation adding reducer-queued change candidate: {}",
                        queued_id
                    );
                    queued.push(change);
                    added += 1;
                }
                None => {
                    warn!(
                        "Queue reconciliation could not load reducer-queued change '{}': candidate_not_found",
                        queued_id
                    );
                    self.emit_queue_reconciliation_diagnostic(
                        QueueReconciliationDiagnosticLevel::Warn,
                        &queued_id,
                        "candidate_not_found",
                    )
                    .await;
                }
            }
        }

        if added > 0 {
            let mut last_change = self.last_queue_change_at.lock().await;
            *last_change = Some(std::time::Instant::now());
        }

        added
    }

    pub(super) async fn emit_no_analysis_diagnostic(
        &self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        reason: &str,
    ) {
        let reducer_queued = self
            .shared_orchestrator_state
            .as_ref()
            .and_then(|state| state.try_read().ok())
            .map(|state| state.queued_change_ids())
            .unwrap_or_default();

        if reducer_queued.is_empty() {
            return;
        }

        send_event(
            &self.event_tx,
            ParallelEvent::Log(LogEntry::info(format!(
                "No analysis started despite reducer-visible queued work: reason={}, reducer_queued={:?}, local_queued={}, in_flight={}, max_parallelism={}",
                reason,
                reducer_queued,
                queued.len(),
                in_flight.len(),
                max_parallelism
            ))),
        )
        .await;
    }

    /// Perform reanalysis and dispatch changes if conditions are met.
    ///
    /// # Arguments
    /// * `queued` - Mutable reference to queued changes list
    /// * `in_flight` - Mutable reference to in-flight changes set
    /// * `max_parallelism` - Maximum parallelism level
    /// * `iteration` - Current iteration number
    /// * `reanalysis_reason` - Current reanalysis reason for logging
    /// * `analyzer` - Dependency analyzer function
    /// * `semaphore` - Semaphore for concurrency control
    /// * `join_set` - JoinSet for spawned tasks
    /// * `cleanup_guard` - Guard for workspace cleanup tracking
    ///
    /// # Returns
    /// `Ok((should_break, new_iteration))` - Whether to break loop and new iteration count
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn perform_reanalysis_and_dispatch<F>(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &mut HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
        reanalysis_reason: ReanalysisReason,
        analyzer: &F,
        semaphore: Arc<Semaphore>,
        join_set: &mut JoinSet<WorkspaceResult>,
        cleanup_guard: &mut WorkspaceCleanupGuard,
    ) -> Result<(bool, u32)>
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>,
            > + Send
            + Sync,
    {
        // Gate re-analysis by available execution slots.
        // Track zero->positive transitions so queue-edit debounce can be bypassed when
        // capacity is restored and queued work can run immediately.
        let available_slots = self.calculate_available_slots(max_parallelism, in_flight);
        let previous_available_slots = self.last_available_slots.replace(available_slots);
        let slot_recovered = matches!(previous_available_slots, Some(0)) && available_slots > 0;

        if slot_recovered && matches!(reanalysis_reason, ReanalysisReason::QueueNotification) {
            info!(
                previous_available_slots = previous_available_slots.unwrap_or_default(),
                available_slots,
                queued = queued.len(),
                "Execution capacity recovered; promoting queue re-analysis trigger"
            );
        }

        if available_slots == 0 {
            // No available slots, defer re-analysis until slots become available
            info!(
                "Re-analysis deferred: no available slots (max: {}, in_flight: {}, queued: {})",
                max_parallelism,
                in_flight.len(),
                queued.len()
            );
            self.emit_no_analysis_diagnostic(
                queued,
                in_flight,
                max_parallelism,
                "no_available_slots",
            )
            .await;
            // Re-analysis stays state-driven and will resume once slots free up.
            return Ok((false, iteration));
        }

        let effective_reason =
            if slot_recovered && matches!(reanalysis_reason, ReanalysisReason::QueueNotification) {
                ReanalysisReason::SlotRecovery
            } else {
                reanalysis_reason
            };
        let bypass_debounce = matches!(
            effective_reason,
            ReanalysisReason::SlotRecovery | ReanalysisReason::ResolveCompletion
        );

        // Check debounce (skip on first iteration)
        let should_analyze = if iteration == 1 {
            info!("First iteration, skipping debounce check");
            true
        } else {
            self.should_reanalyze(bypass_debounce).await
        };

        if !should_analyze {
            // Debounce active, wait for timer or queue notification
            info!("Debounce active, waiting for timer or queue notification");
            self.emit_no_analysis_diagnostic(queued, in_flight, max_parallelism, "debounce_active")
                .await;
            return Ok((false, iteration));
        }

        // Filter out changes that depend on failed changes
        let (executable_changes, skipped_changes) = self.filter_executable_changes(queued);

        // Emit skip events
        for (change_id, reason) in skipped_changes {
            send_event(
                &self.event_tx,
                ParallelEvent::ChangeSkipped { change_id, reason },
            )
            .await;
        }

        *queued = executable_changes;

        if queued.is_empty() {
            info!("All queued changes skipped due to failed dependencies");
            self.emit_no_analysis_diagnostic(
                queued,
                in_flight,
                max_parallelism,
                "local_queue_empty_after_reconciliation",
            )
            .await;
            if in_flight.is_empty() {
                return Ok((true, iteration)); // Should break
            } else {
                // Wait for in-flight to complete
                return Ok((false, iteration)); // Continue, don't break
            }
        }

        // Run dependency analysis
        info!(
            "Re-analysis triggered: iteration={}, queued={}, in_flight={}, trigger={}",
            iteration,
            queued.len(),
            in_flight.len(),
            effective_reason
        );
        send_event(
            &self.event_tx,
            ParallelEvent::AnalysisStarted {
                remaining_changes: queued.len(),
            },
        )
        .await;

        // Convert in_flight HashSet to Vec<String> for analyzer
        let in_flight_ids: Vec<String> = in_flight.iter().cloned().collect();
        let analysis_result = analyzer(queued, &in_flight_ids, iteration).await;

        if analysis_result.order.is_empty() {
            warn!("No order returned from analysis");
            if in_flight.is_empty() {
                return Ok((true, iteration)); // Should break
            } else {
                return Ok((false, iteration)); // Continue
            }
        }

        // Update dependencies
        self.failed_tracker
            .set_dependencies(analysis_result.dependencies.clone());
        self.change_dependencies = analysis_result.dependencies.clone();

        // Recalculate available slots (may have changed during analysis if tasks completed)
        let available_slots = self.calculate_available_slots(max_parallelism, in_flight);
        info!(
            "Available slots after analysis: {} (max: {}, in_flight: {}, queued: {})",
            available_slots,
            max_parallelism,
            in_flight.len(),
            queued.len()
        );

        // Select changes to dispatch based on order and available slots
        let selected_changes = self
            .select_changes_for_dispatch(&analysis_result, available_slots)
            .await;

        // Dispatch selected changes
        let new_iteration = if !selected_changes.is_empty() {
            let base_revision = self
                .workspace_manager
                .get_current_revision()
                .await
                .map_err(OrchestratorError::from)?;

            info!(
                "Dispatching {} changes (iteration {}): {:?}",
                selected_changes.len(),
                iteration,
                selected_changes
            );

            for change_id in &selected_changes {
                if let Err(e) = self
                    .dispatch_change_to_workspace(
                        change_id.clone(),
                        base_revision.clone(),
                        semaphore.clone(),
                        join_set,
                        in_flight,
                        cleanup_guard,
                    )
                    .await
                {
                    let message = format!("Failed to dispatch change '{}': {}", change_id, e);
                    self.failed_tracker.mark_failed(change_id);
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ProcessingError {
                            id: change_id.clone(),
                            error: message.clone(),
                        },
                    )
                    .await;
                    send_event(
                        &self.event_tx,
                        ParallelEvent::Log(LogEntry::error(message.clone())),
                    )
                    .await;
                    error!("{}", message);
                }
            }

            // Remove dispatched changes from queued
            let dispatched_set: std::collections::HashSet<_> = selected_changes.iter().collect();
            queued.retain(|c| !dispatched_set.contains(&c.id));

            iteration + 1
        } else {
            iteration
        };

        Ok((false, new_iteration))
    }
}
