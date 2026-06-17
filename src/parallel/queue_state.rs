//! Queue state management and dispatch coordination for [`super::ParallelExecutor`].
//!
//! This module handles queue monitoring, debounce logic, dependency checking,
//! and the reanalysis/dispatch cycle that drives the parallel execution scheduler.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::analyzer::AnalysisResult;
use crate::dependency_targets::DependencyTargetClass;
use crate::error::{OrchestratorError, Result};
use crate::events::{ExecutionEvent, LogEntry};
use crate::orchestration::state::WaitState;
use crate::orchestration::{
    execute_rejection_flow, handle_blocked_from_rejecting, handle_resume_apply_from_rejecting,
    run_rejection_review, RejectionReviewVerdict,
};
use crate::parallel::dedup::DiagnosticDeduplicationKey;
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct QueueReconciliationOutcome {
    pub queued_added: usize,
    pub repair_added: usize,
}

fn analysis_attempt_id(
    iteration: u32,
    trigger: impl std::fmt::Display,
    queued: &[crate::openspec::Change],
) -> String {
    let mut queued_ids: Vec<&str> = queued.iter().map(|change| change.id.as_str()).collect();
    queued_ids.sort_unstable();
    format!(
        "iteration={};trigger={};queued={}",
        iteration,
        trigger,
        queued_ids.join(",")
    )
}

impl QueueReconciliationOutcome {
    #[cfg(test)]
    pub fn total_added(self) -> usize {
        self.queued_added + self.repair_added
    }

    pub fn has_queued_additions(self) -> bool {
        self.queued_added > 0
    }

    pub fn has_repair_additions(self) -> bool {
        self.repair_added > 0
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum QueuedWorkClass {
    DispatchableApply,
    ManualMergeWait,
    SchedulerLaneWait,
    TerminalErrorRetryRequired,
    DependencyBlocked,
    CandidateUnavailable,
}

#[derive(Debug, Clone, Default)]
pub(super) struct BlockedOnlyQueueClassification {
    pub dispatchable: Vec<crate::openspec::Change>,
    pub manual_merge_wait: Vec<String>,
    pub scheduler_lane_wait: Vec<String>,
    pub terminal_error_retry_required: Vec<String>,
    pub dependency_blocked: Vec<String>,
    pub candidate_unavailable: Vec<String>,
}

impl BlockedOnlyQueueClassification {
    pub fn has_dispatchable_apply(&self) -> bool {
        !self.dispatchable.is_empty()
    }

    pub fn has_blocked_or_waiting_work(&self) -> bool {
        !self.manual_merge_wait.is_empty()
            || !self.scheduler_lane_wait.is_empty()
            || !self.terminal_error_retry_required.is_empty()
            || !self.dependency_blocked.is_empty()
            || !self.candidate_unavailable.is_empty()
    }

    pub fn is_blocked_only(&self) -> bool {
        !self.has_dispatchable_apply() && self.has_blocked_or_waiting_work()
    }

    #[cfg(test)]
    pub fn class_for(&self, change_id: &str) -> Option<QueuedWorkClass> {
        if self
            .dispatchable
            .iter()
            .any(|change| change.id == change_id)
        {
            Some(QueuedWorkClass::DispatchableApply)
        } else if self.manual_merge_wait.iter().any(|id| id == change_id) {
            Some(QueuedWorkClass::ManualMergeWait)
        } else if self.scheduler_lane_wait.iter().any(|id| id == change_id) {
            Some(QueuedWorkClass::SchedulerLaneWait)
        } else if self
            .terminal_error_retry_required
            .iter()
            .any(|id| id == change_id)
        {
            Some(QueuedWorkClass::TerminalErrorRetryRequired)
        } else if self.dependency_blocked.iter().any(|id| id == change_id) {
            Some(QueuedWorkClass::DependencyBlocked)
        } else if self.candidate_unavailable.iter().any(|id| id == change_id) {
            Some(QueuedWorkClass::CandidateUnavailable)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueueReconciliationDiagnosticLevel {
    Info,
    Warn,
}

use super::acceptance_state::delete_acceptance_state;
use super::cleanup::WorkspaceCleanupGuard;
use super::dependency::DependencyContext;
use super::dispatch::archived_dirty_repair_candidate_from_workspace;
use super::dynamic_queue::ReanalysisReason;
use super::events::send_event;
use super::merge::base_dirty_reason;
use super::{
    MergeResult, MergeResultOrigin, MergeTaskOutcome, ParallelEvent, ParallelExecutor,
    WorkspaceResult,
};

pub(crate) struct ReanalysisDispatchContext<'a, F> {
    pub queued: &'a mut Vec<crate::openspec::Change>,
    pub in_flight: &'a mut HashSet<String>,
    pub max_parallelism: usize,
    pub iteration: u32,
    pub reanalysis_reason: ReanalysisReason,
    pub analyzer: &'a F,
    pub semaphore: Arc<Semaphore>,
    pub join_set: &'a mut JoinSet<WorkspaceResult>,
    pub cleanup_guard: &'a mut WorkspaceCleanupGuard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReanalysisFlowDecision {
    Continue,
    Done { should_break: bool, iteration: u32 },
}

impl ReanalysisFlowDecision {
    fn done(should_break: bool, iteration: u32) -> Self {
        Self::Done {
            should_break,
            iteration,
        }
    }

    fn into_result(self) -> Option<(bool, u32)> {
        match self {
            Self::Continue => None,
            Self::Done {
                should_break,
                iteration,
            } => Some((should_break, iteration)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReanalysisExecutionDecision {
    effective_reason: ReanalysisReason,
}

enum DependencyAnalysisAttempt {
    Completed(AnalysisResult),
    EmptyOrder(ReanalysisFlowDecision),
}

struct DispatchSelectedCandidatesContext<'a> {
    queued: &'a mut Vec<crate::openspec::Change>,
    in_flight: &'a mut HashSet<String>,
    iteration: u32,
    analysis_result: &'a AnalysisResult,
    available_slots: usize,
    semaphore: Arc<Semaphore>,
    join_set: &'a mut JoinSet<WorkspaceResult>,
    cleanup_guard: &'a mut WorkspaceCleanupGuard,
}

fn on_merged_failure_message(change_id: &str, error: &OrchestratorError) -> String {
    format!(
        "on_merged hook failed for '{}'; merged transition blocked: {}",
        change_id, error
    )
}

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
                self.reject_wait_changes = guard.reject_wait_change_ids().into_iter().collect();
            }
        }
    }

    pub(super) async fn clear_resolve_wait_intent_for_outcome(&mut self, change_id: &str) {
        self.resolve_wait_changes.remove(change_id);
        self.last_dispatched_resolve_wait_changes.remove(change_id);
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.clear_resolve_wait_intent(change_id);
        }
    }

    pub(super) async fn clear_reject_wait_intent_for_success(&mut self, change_id: &str) {
        self.reject_wait_changes.remove(change_id);
        self.last_dispatched_reject_wait_changes.remove(change_id);
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.clear_reject_wait_intent(change_id);
        }
    }

    async fn abandon_base_mutating_lane_occupant_for_give_up(&mut self, change_id: &str) {
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            let released = guard.abandon_base_mutating_lane_occupant(change_id);
            if released {
                info!(
                    change_id = %change_id,
                    "Released base-mutating lane after spawned retry give-up"
                );
            }
        }
    }

    async fn apply_rejection_review_event_in_shared_state(&mut self, event: &ExecutionEvent) {
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.apply_execution_event(event);
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

    async fn mark_on_merged_failure_in_shared_state(&mut self, change_id: &str, error: &str) {
        if let Some(shared) = &self.shared_orchestrator_state {
            let mut guard = shared.write().await;
            guard.apply_execution_event(&ExecutionEvent::HookFailed {
                change_id: change_id.to_string(),
                hook_type: crate::hooks::HookType::OnMerged.to_string(),
                error: error.to_string(),
            });
        }
    }

    pub(super) fn trigger_resolve_wait_retry_dispatch(&mut self) {
        self.resolve_wait_retry_triggered = true;
    }

    pub(super) fn should_dispatch_resolve_wait_retry(&self) -> bool {
        if self.resolve_wait_changes.is_empty() && self.reject_wait_changes.is_empty() {
            return false;
        }

        self.resolve_wait_retry_triggered
            || self.last_dispatched_resolve_wait_changes != self.resolve_wait_changes
            || self.last_dispatched_reject_wait_changes != self.reject_wait_changes
    }

    #[allow(dead_code)]
    pub(super) async fn maybe_dispatch_resolve_wait_retry(&mut self) {
        let base_dirty_changed_to_clean = self
            .resolve_wait_base_dirty_changed_to_clean()
            .await
            .unwrap_or(false);

        if !self.should_dispatch_resolve_wait_retry() && !base_dirty_changed_to_clean {
            return;
        }

        self.retry_deferred_base_lane_waiters().await;
        self.last_dispatched_resolve_wait_changes = self.resolve_wait_changes.clone();
        self.last_dispatched_reject_wait_changes = self.reject_wait_changes.clone();
        self.resolve_wait_retry_triggered = false;
    }

    pub(super) async fn maybe_dispatch_resolve_wait_retry_with_tx(
        &mut self,
        merge_result_tx: &mpsc::Sender<MergeResult>,
    ) {
        let base_dirty_changed_to_clean = self
            .resolve_wait_base_dirty_changed_to_clean()
            .await
            .unwrap_or(false);

        if !self.should_dispatch_resolve_wait_retry() && !base_dirty_changed_to_clean {
            return;
        }

        self.dispatch_deferred_base_lane_waiter(merge_result_tx.clone())
            .await;
    }

    async fn resolve_wait_base_dirty_changed_to_clean(&mut self) -> Result<bool> {
        if self.resolve_wait_changes.is_empty() && self.reject_wait_changes.is_empty() {
            self.last_resolve_wait_base_dirty = None;
            return Ok(false);
        }

        let base_dirty = base_dirty_reason(&self.repo_root).await?.is_some();
        let changed_to_clean =
            matches!(self.last_resolve_wait_base_dirty, Some(true)) && !base_dirty;
        if changed_to_clean {
            info!(
                repo_root = %self.repo_root.display(),
                resolve_wait_count = self.resolve_wait_changes.len(),
                reject_wait_count = self.reject_wait_changes.len(),
                "Base repository transitioned from dirty to clean while base-lane waiters exist; waking retry dispatch"
            );
        }
        self.last_resolve_wait_base_dirty = Some(base_dirty);
        Ok(changed_to_clean)
    }

    pub(crate) fn has_resolve_wait(&self) -> bool {
        if let Some(shared) = &self.shared_orchestrator_state {
            if let Ok(guard) = shared.try_read() {
                return !guard.resolve_wait_change_ids().is_empty()
                    || !guard.reject_wait_change_ids().is_empty();
            }
        }
        !self.resolve_wait_changes.is_empty() || !self.reject_wait_changes.is_empty()
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

    fn dependency_blocker_fingerprint(
        change_id: &str,
        blockers: &[(String, DependencyTargetClass)],
    ) -> super::DependencyBlockerFingerprint {
        DependencyContext::blocker_fingerprint(change_id, blockers)
    }

    fn should_emit_dependency_blocked_transition(
        &mut self,
        change_id: &str,
        blockers: &[(String, DependencyTargetClass)],
    ) -> bool {
        let fingerprint = Self::dependency_blocker_fingerprint(change_id, blockers);
        let changed = self
            .dependency_blocker_fingerprints
            .get(change_id)
            .is_none_or(|previous| previous != &fingerprint);

        if changed {
            self.dependency_blocker_fingerprints
                .insert(change_id.to_string(), fingerprint.clone());
            self.diagnostic_dedup
                .should_emit(DiagnosticDeduplicationKey::DependencyBlocker {
                    change_id: change_id.to_string(),
                    fingerprint,
                });
            return true;
        }

        false
    }

    async fn emit_dependency_blocker_diagnostic(
        &self,
        change_id: &str,
        blockers: &[(String, DependencyTargetClass)],
    ) {
        for (dep_id, class) in blockers {
            let message = if matches!(class, DependencyTargetClass::Archived) {
                match self.effective_dependency_base().await {
                    Ok(effective_base) => format!(
                        "Change '{}' blocked by archived-but-not-merged dependency '{}' on effective dependency base '{}' and will remain queued",
                        change_id, dep_id, effective_base
                    ),
                    Err(err) => format!(
                        "Change '{}' blocked by archived dependency '{}' while effective dependency base could not be determined ({}) and will remain queued",
                        change_id, dep_id, err
                    ),
                }
            } else {
                format!(
                    "Change '{}' blocked by {} dependency '{}' and will remain queued",
                    change_id,
                    class.as_str(),
                    dep_id
                )
            };
            match class {
                DependencyTargetClass::Missing
                | DependencyTargetClass::Rejected
                | DependencyTargetClass::Error => warn!("{}", message),
                DependencyTargetClass::Queued
                | DependencyTargetClass::InFlight
                | DependencyTargetClass::ActiveButNotQueued => info!("{}", message),
                DependencyTargetClass::Archived => info!("{}", message),
            }
            if matches!(
                class,
                DependencyTargetClass::Missing
                    | DependencyTargetClass::Rejected
                    | DependencyTargetClass::Error
            ) {
                send_event(&self.event_tx, ParallelEvent::Error { message }).await;
            }
        }
    }

    /// Select the repository-visible base used for archived dependency merge checks.
    ///
    /// Ordinary runs use the original branch captured at scheduler startup. Stacked
    /// orchestration can advance a separate integration branch after startup; when the
    /// repository is currently on such a branch, that current branch is the effective
    /// dependency base for dispatch decisions.
    async fn effective_dependency_base(&self) -> Result<String> {
        let mut dependency_context =
            DependencyContext::from_executor(self, std::iter::empty::<&str>(), &HashSet::new());
        dependency_context
            .effective_dependency_base(self.workspace_manager.as_ref())
            .await
            .map(str::to_string)
    }

    /// Check if a dependency is resolved (merged to the effective dependency base).
    ///
    /// A dependency is considered resolved if its archive commit is present in the
    /// effective base branch/tree and its active change directory is absent there.
    /// This preserves the archive-only safety guard while allowing stacked runs to
    /// unblock dependents once the dependency is merged into the integration branch.
    #[allow(dead_code)]
    pub(super) async fn is_dependency_resolved(&self, dep_id: &str) -> Result<bool> {
        let (is_resolved, _) = self.is_dependency_resolved_with_base(dep_id).await?;
        Ok(is_resolved)
    }

    async fn is_dependency_resolved_with_base(&self, dep_id: &str) -> Result<(bool, String)> {
        let mut dependency_context =
            DependencyContext::from_executor(self, std::iter::empty::<&str>(), &HashSet::new());
        dependency_context
            .is_dependency_resolved_with_base(dep_id, self.workspace_manager.as_ref())
            .await
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
        in_flight: &HashSet<String>,
    ) -> Vec<String> {
        let mut selected_changes: Vec<String> = Vec::new();
        let mut dependency_context = DependencyContext::from_executor(
            self,
            analysis_result.order.iter().map(String::as_str),
            in_flight,
        );

        for change_id in &analysis_result.order {
            if dependency_context.is_terminal_error_change(change_id) {
                info!(
                    change_id = %change_id,
                    "Skipping ordinary apply dispatch because terminal error requires explicit retry"
                );
                continue;
            }

            // Check if change has unresolved dependencies
            if let Some(deps) = analysis_result.dependencies.get(change_id) {
                let mut unresolved_deps = Vec::new();
                let mut blockers = Vec::new();
                for dep_id in deps {
                    let class = dependency_context.classify(dep_id);
                    match class {
                        DependencyTargetClass::Missing | DependencyTargetClass::Rejected => {
                            unresolved_deps.push(dep_id.clone());
                            blockers.push((dep_id.clone(), class));
                            continue;
                        }
                        DependencyTargetClass::Error => {
                            warn!(
                                change_id = %change_id,
                                dependency = %dep_id,
                                "Blocking dispatch because dependency is in terminal error and requires explicit retry"
                            );
                            unresolved_deps.push(dep_id.clone());
                            blockers.push((dep_id.clone(), class));
                            continue;
                        }
                        DependencyTargetClass::Archived => {
                            DependencyContext::log_archived_dependency_check(change_id, dep_id);
                        }
                        DependencyTargetClass::Queued
                        | DependencyTargetClass::InFlight
                        | DependencyTargetClass::ActiveButNotQueued => {}
                    }

                    match dependency_context
                        .is_dependency_resolved_with_base(dep_id, self.workspace_manager.as_ref())
                        .await
                    {
                        Ok((true, effective_base)) => {
                            DependencyContext::log_dependency_resolved(
                                change_id,
                                dep_id,
                                class,
                                &effective_base,
                            );
                        }
                        Ok((false, effective_base)) => {
                            DependencyContext::log_dependency_unresolved(
                                change_id,
                                dep_id,
                                class,
                                &effective_base,
                            );
                            unresolved_deps.push(dep_id.clone());
                            blockers.push((dep_id.clone(), class));
                        }
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
                            blockers.push((dep_id.clone(), class));
                        }
                    }
                }

                if !unresolved_deps.is_empty() {
                    if self.should_emit_dependency_blocked_transition(change_id, &blockers) {
                        info!(
                            "Change '{}' blocked: waiting for dependencies {:?}",
                            change_id, unresolved_deps
                        );
                        self.emit_dependency_blocker_diagnostic(change_id, &blockers)
                            .await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::DependencyBlocked {
                                change_id: change_id.clone(),
                                dependency_ids: unresolved_deps,
                            },
                        )
                        .await;
                    } else {
                        debug!(
                            change_id,
                            blockers = ?blockers,
                            "Suppressing repeated dependency blocked transition"
                        );
                    }
                    continue;
                }
            }

            // Check if this change was previously blocked and is now resolved.
            // Clearing the in-memory fingerprint makes the next blocked observation a new transition.
            if self
                .dependency_blocker_fingerprints
                .remove(change_id)
                .is_some()
            {
                info!(
                    "Change '{}' dependencies resolved, forcing fresh workspace recreation",
                    change_id
                );
                self.force_recreate_worktree.insert(change_id.clone());
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
            // waiting in ResolveWait due to that blocker, retry them without blocking the scheduler.
            self.dispatch_deferred_base_lane_waiter(merge_result_tx.clone())
                .await;
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
            self.dispatch_deferred_base_lane_waiter(merge_result_tx.clone())
                .await;
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
            let Some(_active_merge_guard) =
                super::merge::ActivePostArchiveMergeGuard::acquire(change_id.clone())
            else {
                info!(
                    change_id = %change_id,
                    workspace = %workspace_name,
                    "Skipping duplicate post-archive merge task because the same change is already active"
                );
                if let Err(send_error) = merge_result_tx
                    .send(MergeResult {
                        change_id,
                        workspace_name,
                        origin: MergeResultOrigin::PostArchiveMerge,
                        outcome: Ok(MergeTaskOutcome::deferred(
                            "duplicate post-archive merge task suppressed because the same change is already active",
                            true,
                        )),
                    })
                    .await
                {
                    warn!(
                        "Failed to send duplicate merge suppression result to scheduler loop: {}",
                        send_error
                    );
                }
                return;
            };
            let outcome = merge_executor
                .handle_merge_and_cleanup(workspace_result)
                .await
                .map_err(|error| error.to_string());

            if let Err(send_error) = merge_result_tx
                .send(MergeResult {
                    change_id,
                    workspace_name,
                    origin: MergeResultOrigin::PostArchiveMerge,
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

    async fn release_retry_lane_after_non_terminal_outcome(
        &mut self,
        change_id: &str,
        origin: MergeResultOrigin,
    ) {
        let wait_state = match origin {
            MergeResultOrigin::ResolveWaitRetry => WaitState::ResolveWait,
            MergeResultOrigin::RejectWaitRetry => WaitState::RejectWait,
            MergeResultOrigin::PostArchiveMerge => return,
        };

        if let Some(shared) = &self.shared_orchestrator_state {
            let released = {
                let mut guard = shared.write().await;
                guard.release_base_mutating_lane_after_retry(change_id, wait_state)
            };
            if released {
                info!(
                    change_id = %change_id,
                    origin = ?origin,
                    "Released base-mutating lane after non-terminal spawned retry outcome"
                );
            } else {
                debug!(
                    change_id = %change_id,
                    origin = ?origin,
                    "No base-mutating lane release needed after non-terminal spawned retry outcome"
                );
            }
        }
        self.sync_resolve_wait_from_shared_state_nonblocking();
    }

    #[allow(dead_code)]
    pub(super) async fn handle_merge_result(&mut self, merge_result: MergeResult) -> bool {
        let (merge_result_tx, _merge_result_rx) = mpsc::channel(1);
        self.handle_merge_result_with_tx(merge_result, &merge_result_tx)
            .await
    }

    pub(super) async fn handle_merge_result_with_tx(
        &mut self,
        merge_result: MergeResult,
        merge_result_tx: &mpsc::Sender<MergeResult>,
    ) -> bool {
        self.pending_merge_count.fetch_sub(1, Ordering::Relaxed);

        match merge_result.outcome {
            Ok(MergeTaskOutcome::Merged) => {
                info!(
                    origin = ?merge_result.origin,
                    "Background base-lane task completed successfully for '{}'",
                    merge_result.change_id
                );
                self.dispatch_deferred_base_lane_waiter(merge_result_tx.clone())
                    .await;
                true
            }

            Ok(MergeTaskOutcome::Deferred {
                reason,
                auto_resumable,
            }) => {
                info!(
                    "Background merge task deferred for '{}' (workspace '{}', auto_resumable={}): {}",
                    merge_result.change_id, merge_result.workspace_name, auto_resumable, reason
                );
                if auto_resumable {
                    self.release_retry_lane_after_non_terminal_outcome(
                        &merge_result.change_id,
                        merge_result.origin,
                    )
                    .await;
                }
                false
            }
            Err(error) => {
                error!(
                    "Background merge task failed for '{}' (workspace '{}', origin {:?}): {}",
                    merge_result.change_id, merge_result.workspace_name, merge_result.origin, error
                );
                self.release_retry_lane_after_non_terminal_outcome(
                    &merge_result.change_id,
                    merge_result.origin,
                )
                .await;
                if !matches!(
                    merge_result.origin,
                    MergeResultOrigin::ResolveWaitRetry | MergeResultOrigin::RejectWaitRetry
                ) {
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
                false
            }
        }
    }

    async fn dispatch_deferred_base_lane_waiter(
        &mut self,
        merge_result_tx: mpsc::Sender<MergeResult>,
    ) {
        let Some(shared) = &self.shared_orchestrator_state else {
            // Without reducer-owned lane state there is no safe single-flight signal for a
            // detached retry. Legacy callers can still use retry_deferred_base_lane_waiters
            // in tests that do not wire the shared scheduler state.
            self.retry_deferred_merges().await;
            self.last_dispatched_resolve_wait_changes = self.resolve_wait_changes.clone();
            self.last_dispatched_reject_wait_changes = self.reject_wait_changes.clone();
            self.resolve_wait_retry_triggered = false;
            return;
        };

        let promoted = {
            let mut guard = shared.write().await;
            guard.promote_next_base_mutating_lane_waiter()
        };

        let Some((change_id, wait_state)) = promoted else {
            self.sync_resolve_wait_from_shared_state_nonblocking();
            self.last_dispatched_resolve_wait_changes = self.resolve_wait_changes.clone();
            self.last_dispatched_reject_wait_changes = self.reject_wait_changes.clone();
            self.resolve_wait_retry_triggered = false;
            return;
        };

        match wait_state {
            crate::orchestration::state::WaitState::ResolveWait => {
                self.resolve_wait_changes.insert(change_id.clone());
                self.spawn_base_lane_retry_task(
                    change_id,
                    MergeResultOrigin::ResolveWaitRetry,
                    merge_result_tx,
                );
            }
            crate::orchestration::state::WaitState::RejectWait => {
                self.reject_wait_changes.insert(change_id.clone());
                self.spawn_base_lane_retry_task(
                    change_id,
                    MergeResultOrigin::RejectWaitRetry,
                    merge_result_tx,
                );
            }
            other => {
                warn!(
                    "Ignoring unsupported base-mutating lane promotion for '{}' with wait state {:?}",
                    change_id, other
                );
            }
        }

        self.last_dispatched_resolve_wait_changes = self.resolve_wait_changes.clone();
        self.last_dispatched_reject_wait_changes = self.reject_wait_changes.clone();
        self.resolve_wait_retry_triggered = false;
    }

    fn spawn_base_lane_retry_task(
        &self,
        change_id: String,
        origin: MergeResultOrigin,
        merge_result_tx: mpsc::Sender<MergeResult>,
    ) {
        let mut retry_executor = ParallelExecutor::new(
            self.repo_root.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        retry_executor.max_conflict_retries = self.max_conflict_retries;
        retry_executor.shared_stagger_state = self.shared_stagger_state.clone();
        retry_executor.auto_resolve_count = self.auto_resolve_count.clone();
        retry_executor.pending_merge_count = self.pending_merge_count.clone();
        retry_executor.cancel_token = self.cancel_token.clone();
        retry_executor.manual_resolve_count = self.manual_resolve_count.clone();
        retry_executor.hooks = self.hooks.clone();
        retry_executor.shared_orchestrator_state = self.shared_orchestrator_state.clone();

        self.pending_merge_count.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let mut workspace_name = change_id.clone();
            let outcome = async {
                match origin {
                    MergeResultOrigin::ResolveWaitRetry => {
                        if let Ok(Some(workspace_info)) = retry_executor
                            .workspace_manager
                            .find_existing_workspace(&change_id)
                            .await
                        {
                            workspace_name = workspace_info.workspace_name;
                        }
                        retry_executor
                            .retry_deferred_merges_for(vec![change_id.clone()])
                            .await
                    }
                    MergeResultOrigin::RejectWaitRetry => {
                        if let Ok(Some(workspace_info)) = retry_executor
                            .workspace_manager
                            .find_existing_workspace(&change_id)
                            .await
                        {
                            workspace_name = workspace_info.workspace_name;
                        }
                        retry_executor
                            .retry_deferred_rejection_review_for(change_id.clone())
                            .await
                    }
                    MergeResultOrigin::PostArchiveMerge => unreachable!(
                        "post-archive merge is dispatched by spawn_merge_task, not retry dispatcher"
                    ),
                }
            }
            .await;

            if let Err(send_error) = merge_result_tx
                .send(MergeResult {
                    change_id,
                    workspace_name,
                    origin,
                    outcome,
                })
                .await
            {
                warn!(
                    "Failed to send base-lane retry result to scheduler loop: {}",
                    send_error
                );
            }
        });
    }

    /// Retry one pending base-mutating lane operation according to reducer-owned ordering.
    #[allow(dead_code)]
    pub(super) async fn retry_deferred_base_lane_waiters(&mut self) {
        let Some(shared) = &self.shared_orchestrator_state else {
            self.retry_deferred_merges().await;
            return;
        };

        let promoted = {
            let mut guard = shared.write().await;
            guard.promote_next_base_mutating_lane_waiter()
        };

        match promoted {
            Some((change_id, crate::orchestration::state::WaitState::ResolveWait)) => {
                self.resolve_wait_changes.insert(change_id.clone());
                let _ = self.retry_deferred_merges_for(vec![change_id]).await;
            }
            Some((change_id, crate::orchestration::state::WaitState::RejectWait)) => {
                self.reject_wait_changes.insert(change_id.clone());
                let _ = self.retry_deferred_rejection_review_for(change_id).await;
            }
            Some((change_id, wait_state)) => {
                warn!(
                    "Ignoring unsupported base-mutating lane promotion for '{}' with wait state {:?}",
                    change_id, wait_state
                );
            }
            None => {}
        }

        if let Some(shared) = &self.shared_orchestrator_state {
            let guard = shared.read().await;
            self.resolve_wait_changes = guard.resolve_wait_change_ids().into_iter().collect();
            self.reject_wait_changes = guard.reject_wait_change_ids().into_iter().collect();
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
        let _ = self.retry_deferred_merges_for(deferred).await;
    }

    pub(super) fn stale_retry_reason(workspace_info: &crate::vcs::WorkspaceInfo) -> Option<String> {
        if workspace_info.path.exists() {
            return None;
        }

        Some(format!(
            "workspace path '{}' no longer exists",
            workspace_info.path.display()
        ))
    }

    pub(super) async fn retry_deferred_merges_for(
        &mut self,
        deferred: Vec<String>,
    ) -> std::result::Result<MergeTaskOutcome, String> {
        let mut outcome = Ok(MergeTaskOutcome::Merged);
        for change_id in deferred.into_iter().take(1) {
            if self.is_change_already_merged_to_base(&change_id).await {
                info!(
                    "Skipping stale deferred merge retry for '{}' because it is already merged to base",
                    change_id
                );
                self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                    .await;
                continue;
            }

            send_event(
                &self.event_tx,
                ParallelEvent::Log(LogEntry::info(format!(
                    "ResolveWait retry dispatch started for '{}'",
                    change_id
                ))),
            )
            .await;

            // Locate the preserved workspace for this change.
            let workspace_info = match self
                .workspace_manager
                .find_existing_workspace(&change_id)
                .await
            {
                Ok(Some(ws)) => ws,
                Ok(None) => {
                    let message = format!(
                        "No workspace found for ResolveWait retry '{}', clearing stale retry intent",
                        change_id
                    );
                    warn!("{}", message);
                    send_event(&self.event_tx, ParallelEvent::Error { message }).await;
                    // Remove from deferred set; the workspace is gone, nothing to retry.
                    self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                    self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                        .await;
                    outcome = Ok(MergeTaskOutcome::Merged);
                    continue;
                }
                Err(e) => {
                    let message = format!(
                        "Failed to find workspace for ResolveWait retry '{}': {}",
                        change_id, e
                    );
                    warn!("{}", message);
                    send_event(
                        &self.event_tx,
                        ParallelEvent::Error {
                            message: message.clone(),
                        },
                    )
                    .await;
                    outcome = Err(message);
                    continue;
                }
            };

            if let Some(stale_reason) = Self::stale_retry_reason(&workspace_info) {
                warn!(
                    change_id = %change_id,
                    workspace = %workspace_info.workspace_name,
                    workspace_path = %workspace_info.path.display(),
                    stale_reason = %stale_reason,
                    "Deferred merge retry workspace path is stale; clearing retry intent"
                );
                self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                    .await;
                outcome = Ok(MergeTaskOutcome::Merged);
                continue;
            }

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
                            let message = on_merged_failure_message(&change_id, &e);
                            error!("{}", message);
                            self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                            self.mark_on_merged_failure_in_shared_state(&change_id, &message)
                                .await;
                            send_event(
                                &self.event_tx,
                                ParallelEvent::HookFailed {
                                    change_id: change_id.clone(),
                                    hook_type: crate::hooks::HookType::OnMerged.to_string(),
                                    error: e.to_string(),
                                },
                            )
                            .await;
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: change_id.clone(),
                                    error: message.clone(),
                                },
                            )
                            .await;
                            outcome = Err(message);
                            continue;
                        }
                    }

                    self.clear_resolve_wait_intent_for_outcome(&change_id).await;
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
                    outcome = Ok(MergeTaskOutcome::Merged);
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
                    let reason = deferred.reason;
                    let auto_resumable = deferred.auto_resumable;
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: change_id.clone(),
                            reason: reason.clone(),
                            auto_resumable,
                        },
                    )
                    .await;
                    outcome = Ok(MergeTaskOutcome::deferred(reason, auto_resumable));
                }
                Err(e) => {
                    error!("Deferred merge retry error for '{}': {}", change_id, e);
                    // Keep in deferred set; another merge/resolve completion will trigger again.
                    outcome = Err(e.to_string());
                }
            }
        }
        outcome
    }

    pub(super) async fn retry_deferred_rejection_review_for(
        &mut self,
        change_id: String,
    ) -> std::result::Result<MergeTaskOutcome, String> {
        let mut outcome = Ok(MergeTaskOutcome::Merged);
        send_event(
            &self.event_tx,
            ParallelEvent::Log(LogEntry::info(format!(
                "RejectWait retry dispatch started for '{}'",
                change_id
            ))),
        )
        .await;

        let workspace_info = match self
            .workspace_manager
            .find_existing_workspace(&change_id)
            .await
        {
            Ok(Some(ws)) => ws,
            Ok(None) => {
                let message = format!(
                    "No workspace found for RejectWait retry '{}', clearing reject wait",
                    change_id
                );
                warn!("{}", message);
                send_event(&self.event_tx, ParallelEvent::Error { message }).await;
                self.clear_reject_wait_intent_for_success(&change_id).await;
                self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                    .await;
                return Ok(MergeTaskOutcome::Merged);
            }
            Err(e) => {
                let message = format!(
                    "Failed to find workspace for RejectWait retry '{}': {}",
                    change_id, e
                );
                warn!("{}", message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Error {
                        message: message.clone(),
                    },
                )
                .await;
                return Err(message);
            }
        };

        send_event(
            &self.event_tx,
            ParallelEvent::WorkspaceStatusUpdated {
                change_id: change_id.clone(),
                workspace_name: workspace_info.workspace_name.clone(),
                status: crate::vcs::WorkspaceStatus::Rejecting,
            },
        )
        .await;
        self.clear_reject_wait_intent_for_success(&change_id).await;

        match run_rejection_review(
            &change_id,
            &workspace_info.path,
            &self.config,
            &self.ai_runner,
        )
        .await
        {
            Ok(RejectionReviewVerdict::Confirm) => {
                let rejected_path = workspace_info
                    .path
                    .join("openspec")
                    .join("changes")
                    .join(&change_id)
                    .join("REJECTED.md");
                let reason = format!(
                    "Rejecting review confirmed rejection (proposal: {})",
                    rejected_path.display()
                );
                let base_branch = self
                    .workspace_manager
                    .ensure_original_branch_initialized()
                    .await
                    .unwrap_or_else(|error| {
                        warn!(
                            "Failed to resolve base branch while confirming deferred rejection review for '{}': {}",
                            change_id, error
                        );
                        "main".to_string()
                    });

                match execute_rejection_flow(
                    &change_id,
                    &reason,
                    &workspace_info.path,
                    &base_branch,
                    &self.repo_root,
                )
                .await
                {
                    Ok(()) => {
                        let completed_event = ParallelEvent::RejectionReviewCompleted {
                            change_id: change_id.clone(),
                            outcome: crate::events::RejectionOutcome::Confirm,
                        };
                        self.apply_rejection_review_event_in_shared_state(&completed_event)
                            .await;
                        send_event(&self.event_tx, completed_event).await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ChangeRejected {
                                change_id: change_id.clone(),
                                reason,
                            },
                        )
                        .await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ChangeDequeued {
                                change_id: change_id.clone(),
                            },
                        )
                        .await;
                    }
                    Err(error) => {
                        error!(
                            "Deferred rejection review confirm flow failed for '{}': {}",
                            change_id, error
                        );
                        let failed_event = ParallelEvent::RejectionReviewFailed {
                            change_id: change_id.clone(),
                            error: format!(
                                "Rejected flow failed after deferred rejecting CONFIRM verdict: {}",
                                error
                            ),
                        };
                        self.apply_rejection_review_event_in_shared_state(&failed_event)
                            .await;
                        send_event(&self.event_tx, failed_event).await;
                    }
                }
            }
            Ok(RejectionReviewVerdict::Resume) => {
                match handle_resume_apply_from_rejecting(&change_id, &workspace_info.path).await {
                    Ok(()) => {
                        let completed_event = ParallelEvent::RejectionReviewCompleted {
                            change_id: change_id.clone(),
                            outcome: crate::events::RejectionOutcome::Resume,
                        };
                        self.apply_rejection_review_event_in_shared_state(&completed_event)
                            .await;
                        send_event(&self.event_tx, completed_event).await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::Log(
                                LogEntry::warn(
                                    "Deferred rejecting review returned RESUME; workspace is ready for apply resume",
                                )
                                .with_change_id(&change_id)
                                .with_operation("rejecting"),
                            ),
                        )
                        .await;
                    }
                    Err(error) => {
                        let failed_event = ParallelEvent::RejectionReviewFailed {
                            change_id: change_id.clone(),
                            error: error.to_string(),
                        };
                        self.apply_rejection_review_event_in_shared_state(&failed_event)
                            .await;
                        outcome = Err(error.to_string());
                        send_event(&self.event_tx, failed_event).await;
                    }
                }
            }
            Ok(RejectionReviewVerdict::Block) => {
                match handle_blocked_from_rejecting(&change_id, &workspace_info.path).await {
                    Ok(()) => {
                        let completed_event = ParallelEvent::RejectionReviewCompleted {
                            change_id: change_id.clone(),
                            outcome: crate::events::RejectionOutcome::Block,
                        };
                        self.apply_rejection_review_event_in_shared_state(&completed_event)
                            .await;
                        send_event(&self.event_tx, completed_event).await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::WorkspaceStatusUpdated {
                                change_id: change_id.clone(),
                                workspace_name: workspace_info.workspace_name.clone(),
                                status: crate::vcs::WorkspaceStatus::Blocked,
                            },
                        )
                        .await;
                    }
                    Err(error) => {
                        let failed_event = ParallelEvent::RejectionReviewFailed {
                            change_id: change_id.clone(),
                            error: error.to_string(),
                        };
                        self.apply_rejection_review_event_in_shared_state(&failed_event)
                            .await;
                        outcome = Err(error.to_string());
                        send_event(&self.event_tx, failed_event).await;
                    }
                }
            }
            Err(error) => {
                error!(
                    "Deferred rejection review failed for '{}': {}",
                    change_id, error
                );
                let failed_event = ParallelEvent::RejectionReviewFailed {
                    change_id: change_id.clone(),
                    error: format!("Rejecting review failed after deferred handoff: {}", error),
                };
                self.apply_rejection_review_event_in_shared_state(&failed_event)
                    .await;
                outcome = Err(error.to_string());
                send_event(&self.event_tx, failed_event).await;
            }
        }
        outcome
    }

    /// Retry rejection review for all RejectWait changes once the base-mutating lane is free.
    #[allow(dead_code)]
    pub(super) async fn retry_deferred_rejection_reviews(&mut self) {
        let Some(shared) = &self.shared_orchestrator_state else {
            return;
        };

        let (lane_occupied, reject_wait_ids) = {
            let guard = shared.read().await;
            (
                guard.is_base_mutating_lane_occupied(),
                guard.reject_wait_change_ids(),
            )
        };
        self.reject_wait_changes = reject_wait_ids.into_iter().collect();

        if lane_occupied || self.reject_wait_changes.is_empty() {
            return;
        }

        let Some(change_id) = self.reject_wait_changes.iter().min().cloned() else {
            return;
        };

        let _ = self.retry_deferred_rejection_review_for(change_id).await;
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
                    if let Some(shared) = &self.shared_orchestrator_state {
                        if let Ok(guard) = shared.try_read() {
                            if guard.is_final_terminal_dispatch_stop(&dynamic_id) {
                                info!(
                                    change_id = %dynamic_id,
                                    "Skipping dynamic queue ingestion because change is in a final terminal state"
                                );
                                drop(guard);
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(LogEntry::info(format!(
                                        "Ignoring stale queue entry for final terminal change: {}",
                                        dynamic_id
                                    ))),
                                )
                                .await;
                                continue;
                            }
                        }
                    }

                    match crate::openspec::list_changes_native_from(&self.repo_root) {
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
        self.diagnostic_dedup
            .should_emit(DiagnosticDeduplicationKey::QueueReconciliation {
                change_id: change_id.to_string(),
                reason: reason.to_string(),
            })
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

        self.emit_queue_reconciliation_diagnostic_without_dedupe(level, change_id, reason)
            .await;
    }

    async fn emit_queue_reconciliation_diagnostic_without_dedupe(
        &self,
        level: QueueReconciliationDiagnosticLevel,
        change_id: &str,
        reason: &str,
    ) {
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
    ) -> QueueReconciliationOutcome {
        let Some(shared_state) = &self.shared_orchestrator_state else {
            return QueueReconciliationOutcome::default();
        };

        let (mut queued_intent_ids, active_ids_from_reducer, terminal_error_ids, merge_wait_ids) =
            match shared_state.try_read() {
                Ok(state) => {
                    let terminal_error_ids = state
                        .initial_change_ids()
                        .iter()
                        .filter(|id| state.is_terminal_error_change(id))
                        .cloned()
                        .collect::<std::collections::HashSet<_>>();
                    (
                        state.queued_change_ids(),
                        state.active_change_ids(),
                        terminal_error_ids,
                        state.merge_wait_change_ids(),
                    )
                }
                Err(_) => return QueueReconciliationOutcome::default(),
            };

        let reducer_active_set: std::collections::HashSet<String> =
            active_ids_from_reducer.into_iter().collect();
        let reducer_merge_wait_set: std::collections::HashSet<String> =
            merge_wait_ids.into_iter().collect();

        let base_branch_for_archived_dirty_scan = match self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
        {
            Ok(branch) => Some(branch),
            Err(error) => {
                warn!(
                    "Failed to determine base branch during archived dirty queue reconciliation: {}",
                    error
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(format!(
                        "Queue reconciliation skipped archived-dirty worktree scan: failed_to_determine_base_branch ({})",
                        error
                    ))),
                )
                .await;
                None
            }
        };

        match self.workspace_manager.list_worktree_change_ids().await {
            Ok(worktree_change_ids) => {
                for worktree_change_id in worktree_change_ids {
                    if terminal_error_ids.contains(&worktree_change_id) {
                        self.emit_queue_reconciliation_diagnostic(
                            QueueReconciliationDiagnosticLevel::Info,
                            &worktree_change_id,
                            "terminal_error_retry_required",
                        )
                        .await;
                        continue;
                    }
                    if reducer_merge_wait_set.contains(&worktree_change_id) {
                        self.emit_queue_reconciliation_diagnostic(
                            QueueReconciliationDiagnosticLevel::Info,
                            &worktree_change_id,
                            "manual_merge_wait",
                        )
                        .await;
                        continue;
                    }
                    if queued_intent_ids.iter().any(|id| id == &worktree_change_id)
                        || in_flight.contains(&worktree_change_id)
                        || reducer_active_set.contains(&worktree_change_id)
                        || Self::is_post_archive_merge_active_for(&worktree_change_id)
                    {
                        continue;
                    }

                    let archived_dirty = if let Some(base_branch) =
                        &base_branch_for_archived_dirty_scan
                    {
                        match self
                            .workspace_manager
                            .find_existing_workspace(&worktree_change_id)
                            .await
                        {
                            Ok(Some(workspace)) => archived_dirty_repair_candidate_from_workspace(
                                &worktree_change_id,
                                &workspace.path,
                                base_branch,
                            )
                            .await
                            .is_some(),
                            Ok(None) => false,
                            Err(error) => {
                                warn!(
                                    change_id = %worktree_change_id,
                                    "Failed to find workspace during archived dirty queue reconciliation: {}",
                                    error
                                );
                                false
                            }
                        }
                    } else {
                        false
                    };

                    if archived_dirty {
                        info!(
                            change_id = %worktree_change_id,
                            "Queue reconciliation discovered archived dirty workspace without reducer queued intent"
                        );
                        queued_intent_ids.push(worktree_change_id);
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to list worktree change ids during archived dirty queue reconciliation: {}",
                    e
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(format!(
                        "Queue reconciliation skipped archived-dirty worktree scan: failed_to_list_worktrees ({})",
                        e
                    ))),
                )
                .await;
            }
        }

        if queued_intent_ids.is_empty() {
            return QueueReconciliationOutcome::default();
        }

        let mut known_changes = match crate::openspec::list_changes_native_from(&self.repo_root) {
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
                return QueueReconciliationOutcome::default();
            }
        };

        let mut known_by_id: std::collections::HashMap<String, crate::openspec::Change> =
            known_changes
                .drain(..)
                .map(|change| (change.id.clone(), change))
                .collect();

        let mut outcome = QueueReconciliationOutcome::default();

        for queued_id in queued_intent_ids {
            if terminal_error_ids.contains(&queued_id) {
                self.emit_queue_reconciliation_diagnostic(
                    QueueReconciliationDiagnosticLevel::Info,
                    &queued_id,
                    "terminal_error_retry_required",
                )
                .await;
                continue;
            }
            if reducer_merge_wait_set.contains(&queued_id) {
                self.emit_queue_reconciliation_diagnostic(
                    QueueReconciliationDiagnosticLevel::Info,
                    &queued_id,
                    "manual_merge_wait",
                )
                .await;
                continue;
            }
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
            if Self::is_post_archive_merge_active_for(&queued_id) {
                self.emit_queue_reconciliation_diagnostic(
                    QueueReconciliationDiagnosticLevel::Info,
                    &queued_id,
                    "post_archive_merge_active",
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
                    outcome.queued_added += 1;
                }
                None => {
                    let archived_dirty_candidate = if let Some(base_branch) =
                        &base_branch_for_archived_dirty_scan
                    {
                        match self
                            .workspace_manager
                            .find_existing_workspace(&queued_id)
                            .await
                        {
                            Ok(Some(workspace)) => {
                                archived_dirty_repair_candidate_from_workspace(
                                    &queued_id,
                                    &workspace.path,
                                    base_branch,
                                )
                                .await
                            }
                            Ok(None) => None,
                            Err(error) => {
                                warn!(
                                    change_id = %queued_id,
                                    "Failed to find workspace for reducer-queued archived dirty repair candidate: {}",
                                    error
                                );
                                None
                            }
                        }
                    } else {
                        None
                    };

                    if let Some(change) = archived_dirty_candidate {
                        info!(
                            "Queue reconciliation adding archived dirty repair candidate: {}",
                            queued_id
                        );
                        self.emit_queue_reconciliation_diagnostic(
                            QueueReconciliationDiagnosticLevel::Info,
                            &queued_id,
                            "archived_dirty_repair_candidate",
                        )
                        .await;
                        queued.push(change);
                        outcome.repair_added += 1;
                    } else if self.should_emit_queue_reconciliation_diagnostic(
                        &queued_id,
                        "candidate_not_found",
                    ) {
                        warn!(
                            "Queue reconciliation could not load reducer-queued change '{}': candidate_not_found",
                            queued_id
                        );
                        self.emit_queue_reconciliation_diagnostic_without_dedupe(
                            QueueReconciliationDiagnosticLevel::Warn,
                            &queued_id,
                            "candidate_not_found",
                        )
                        .await;
                    } else {
                        debug!(
                            change_id = %queued_id,
                            reason = "candidate_not_found",
                            "Suppressing repeated queue reconciliation candidate_not_found warning"
                        );
                    }
                }
            }
        }

        if outcome.queued_added > 0 {
            let mut last_change = self.last_queue_change_at.lock().await;
            *last_change = Some(std::time::Instant::now());
        }

        outcome
    }

    pub(super) async fn classify_queued_work(
        &self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
    ) -> BlockedOnlyQueueClassification {
        let mut classification = BlockedOnlyQueueClassification::default();
        let mut seen_ids: HashSet<String> = HashSet::new();
        let dependency_context = DependencyContext::from_executor(
            self,
            queued.iter().map(|change| change.id.as_str()),
            in_flight,
        );

        let (reducer_queued, merge_wait_ids, resolve_wait_ids, reject_wait_ids) = self
            .shared_orchestrator_state
            .as_ref()
            .and_then(|state| state.try_read().ok())
            .map(|state| {
                (
                    state.queued_change_ids(),
                    state.merge_wait_change_ids(),
                    state.resolve_wait_change_ids(),
                    state.reject_wait_change_ids(),
                )
            })
            .unwrap_or_default();

        let merge_wait_set: HashSet<String> = merge_wait_ids.into_iter().collect();
        let resolve_wait_set: HashSet<String> = resolve_wait_ids.into_iter().collect();
        let reject_wait_set: HashSet<String> = reject_wait_ids.into_iter().collect();

        for change in queued {
            seen_ids.insert(change.id.clone());
            if merge_wait_set.contains(&change.id) || self.merge_wait_changes.contains(&change.id) {
                classification.manual_merge_wait.push(change.id.clone());
                continue;
            }
            if resolve_wait_set.contains(&change.id)
                || reject_wait_set.contains(&change.id)
                || self.resolve_wait_changes.contains(&change.id)
                || self.reject_wait_changes.contains(&change.id)
            {
                classification.scheduler_lane_wait.push(change.id.clone());
                continue;
            }
            if dependency_context.is_terminal_error_change(&change.id) {
                classification
                    .terminal_error_retry_required
                    .push(change.id.clone());
                continue;
            }
            if self.failed_tracker.should_skip(&change.id).is_some() {
                classification.dependency_blocked.push(change.id.clone());
                continue;
            }

            let blocked = dependency_context
                .is_blocked(&change.dependencies)
                .is_some();

            if blocked {
                classification.dependency_blocked.push(change.id.clone());
            } else {
                classification.dispatchable.push(change.clone());
            }
        }

        for queued_id in reducer_queued {
            if !seen_ids.insert(queued_id.clone()) {
                continue;
            }
            if merge_wait_set.contains(&queued_id) || self.merge_wait_changes.contains(&queued_id) {
                classification.manual_merge_wait.push(queued_id);
            } else if resolve_wait_set.contains(&queued_id)
                || reject_wait_set.contains(&queued_id)
                || self.resolve_wait_changes.contains(&queued_id)
                || self.reject_wait_changes.contains(&queued_id)
            {
                classification.scheduler_lane_wait.push(queued_id);
            } else if dependency_context.is_terminal_error_change(&queued_id) {
                classification.terminal_error_retry_required.push(queued_id);
            } else {
                classification.candidate_unavailable.push(queued_id);
            }
        }

        for change_id in merge_wait_set
            .into_iter()
            .chain(self.merge_wait_changes.iter().cloned())
        {
            if seen_ids.insert(change_id.clone()) {
                classification.manual_merge_wait.push(change_id);
            }
        }
        for change_id in resolve_wait_set
            .into_iter()
            .chain(reject_wait_set)
            .chain(self.resolve_wait_changes.iter().cloned())
            .chain(self.reject_wait_changes.iter().cloned())
        {
            if seen_ids.insert(change_id.clone()) {
                classification.scheduler_lane_wait.push(change_id);
            }
        }

        classification
    }

    pub(super) async fn is_blocked_only_scheduler_state(
        &self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
    ) -> bool {
        in_flight.is_empty()
            && self.manual_resolve_active() == 0
            && self.pending_merge_count.load(Ordering::Relaxed) == 0
            && self
                .classify_queued_work(queued, in_flight)
                .await
                .is_blocked_only()
    }

    async fn emit_log_diagnostic_once(
        &mut self,
        key: DiagnosticDeduplicationKey,
        log_entry: LogEntry,
        suppressed_message: &'static str,
    ) {
        let event_tx = self.event_tx.clone();
        self.diagnostic_dedup
            .emit_or_suppress(
                key,
                || async move {
                    send_event(&event_tx, ParallelEvent::Log(log_entry)).await;
                },
                || debug!("{}", suppressed_message),
            )
            .await;
    }

    #[cfg(test)]
    async fn emit_error_diagnostic_once(
        &mut self,
        key: DiagnosticDeduplicationKey,
        message: String,
        suppressed_message: &'static str,
    ) {
        let event_tx = self.event_tx.clone();
        self.diagnostic_dedup
            .emit_or_suppress(
                key,
                || async move {
                    warn!("{}", message);
                    send_event(&event_tx, ParallelEvent::Log(LogEntry::warn(&message))).await;
                    send_event(&event_tx, ParallelEvent::Error { message }).await;
                },
                || debug!("{}", suppressed_message),
            )
            .await;
    }

    #[cfg(test)]
    fn analysis_failure_diagnostic(
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        error: &str,
    ) -> (DiagnosticDeduplicationKey, String) {
        let mut queued_ids: Vec<String> = queued.iter().map(|change| change.id.clone()).collect();
        queued_ids.sort();
        let mut in_flight_ids: Vec<String> = in_flight.iter().cloned().collect();
        in_flight_ids.sort();
        let normalized_error = error.trim().to_string();
        let key = DiagnosticDeduplicationKey::AnalysisFailure {
            queued_ids: queued_ids.clone(),
            in_flight_ids: in_flight_ids.clone(),
            error: normalized_error.clone(),
        };
        let message = format!(
            "Dependency analysis failed: error={}, queued={:?}, in_flight={:?}",
            normalized_error, queued_ids, in_flight_ids
        );
        (key, message)
    }

    fn no_analysis_diagnostic(
        &self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        reason: &str,
    ) -> Option<(DiagnosticDeduplicationKey, LogEntry)> {
        let reducer_queued = self
            .shared_orchestrator_state
            .as_ref()
            .and_then(|state| state.try_read().ok())
            .map(|state| state.queued_change_ids())
            .unwrap_or_default();
        if reducer_queued.is_empty() {
            return None;
        }

        let queued_len = queued.len();
        let in_flight_len = in_flight.len();
        let key = DiagnosticDeduplicationKey::NoAnalysis {
            reducer_queued: reducer_queued.clone(),
            queued_len,
            in_flight_len,
            reason: reason.to_string(),
        };
        let log_entry = LogEntry::info(format!(
            "No analysis started despite reducer-visible queued work: reason={}, reducer_queued={:?}, local_queued={}, in_flight={}, max_parallelism={}",
            reason, reducer_queued, queued_len, in_flight_len, max_parallelism
        ));
        Some((key, log_entry))
    }

    fn capacity_zero_dispatch_diagnostic(
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        analysis_order: &[String],
    ) -> (DiagnosticDeduplicationKey, LogEntry) {
        const REASON: &str = "dispatch_capacity_zero_after_analysis";
        let order = if analysis_order.is_empty() {
            let mut queued_ids = queued
                .iter()
                .map(|change| change.id.clone())
                .collect::<Vec<_>>();
            queued_ids.sort();
            queued_ids
        } else {
            analysis_order.to_vec()
        };
        let queued_len = queued.len();
        let in_flight_len = in_flight.len();
        let key = DiagnosticDeduplicationKey::DispatchCapacityZero {
            order,
            queued_len,
            in_flight_len,
            max_parallelism,
            reason: REASON.to_string(),
        };
        let log_entry = LogEntry::info(format!(
            "Dispatch suppressed after dependency analysis: reason={}, local_queued={}, in_flight={}, max_parallelism={}",
            REASON, queued_len, in_flight_len, max_parallelism
        ));
        (key, log_entry)
    }

    #[cfg(test)]
    pub(super) async fn emit_analysis_failure_diagnostic_once(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        error: &str,
    ) {
        let (key, message) = Self::analysis_failure_diagnostic(queued, in_flight, error);
        self.emit_error_diagnostic_once(
            key,
            message,
            "Suppressing repeated analysis failure diagnostic",
        )
        .await;
    }

    pub(super) async fn emit_no_analysis_diagnostic(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        reason: &str,
    ) {
        if let Some((key, log_entry)) =
            self.no_analysis_diagnostic(queued, in_flight, max_parallelism, reason)
        {
            self.emit_log_diagnostic_once(
                key,
                log_entry,
                "Suppressing repeated no-analysis diagnostic",
            )
            .await;
        }
    }

    pub(super) async fn emit_capacity_zero_dispatch_diagnostic_once(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        analysis_order: &[String],
    ) {
        let (key, log_entry) = Self::capacity_zero_dispatch_diagnostic(
            queued,
            in_flight,
            max_parallelism,
            analysis_order,
        );
        self.emit_log_diagnostic_once(
            key,
            log_entry,
            "Suppressing repeated capacity-zero dispatch diagnostic",
        )
        .await;
    }

    async fn prepare_dispatch_candidates(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
    ) -> ReanalysisFlowDecision {
        let classification = self.classify_queued_work(queued, in_flight).await;
        if classification.is_blocked_only() {
            info!(
                dispatchable = classification.dispatchable.len(),
                manual_merge_wait = classification.manual_merge_wait.len(),
                scheduler_lane_wait = classification.scheduler_lane_wait.len(),
                terminal_error_retry_required = classification.terminal_error_retry_required.len(),
                dependency_blocked = classification.dependency_blocked.len(),
                candidate_unavailable = classification.candidate_unavailable.len(),
                "Skipping dependency analysis because queued work is blocked-only"
            );
            self.emit_no_analysis_diagnostic(
                queued,
                in_flight,
                max_parallelism,
                "blocked_only_no_dispatchable_candidates",
            )
            .await;
            return ReanalysisFlowDecision::done(false, iteration);
        }

        if !classification.has_dispatchable_apply() && !queued.is_empty() {
            info!(
                queued = queued.len(),
                "Skipping dependency analysis because no ordinary dispatchable candidates are available"
            );
            self.emit_no_analysis_diagnostic(
                queued,
                in_flight,
                max_parallelism,
                "no_dispatchable_apply_candidates",
            )
            .await;
            return ReanalysisFlowDecision::done(false, iteration);
        }

        if queued.len() != classification.dispatchable.len() {
            *queued = classification.dispatchable;
        }

        ReanalysisFlowDecision::Continue
    }

    fn compute_effective_reanalysis_reason(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        reanalysis_reason: ReanalysisReason,
    ) -> ReanalysisExecutionDecision {
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
            info!(
                max_parallelism,
                in_flight = in_flight.len(),
                queued = queued.len(),
                manual_resolve_active = self.manual_resolve_active(),
                auto_resolve_active = self.auto_resolve_count.load(Ordering::Relaxed),
                "Re-analysis will continue with zero dispatch capacity; ordinary apply dispatch remains suppressed"
            );
        }

        let effective_reason =
            if slot_recovered && matches!(reanalysis_reason, ReanalysisReason::QueueNotification) {
                ReanalysisReason::SlotRecovery
            } else {
                reanalysis_reason
            };

        ReanalysisExecutionDecision { effective_reason }
    }

    async fn should_run_analysis_now(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
        effective_reason: ReanalysisReason,
    ) -> ReanalysisFlowDecision {
        let bypass_debounce = matches!(
            effective_reason,
            ReanalysisReason::QueueNotification
                | ReanalysisReason::SlotRecovery
                | ReanalysisReason::ResolveCompletion
                | ReanalysisReason::RepairCandidate
        );

        let should_analyze = if iteration == 1 {
            info!("First iteration, skipping debounce check");
            true
        } else {
            self.should_reanalyze(bypass_debounce).await
        };

        if should_analyze {
            ReanalysisFlowDecision::Continue
        } else {
            info!("Debounce active, waiting for timer or queue notification");
            self.emit_no_analysis_diagnostic(queued, in_flight, max_parallelism, "debounce_active")
                .await;
            ReanalysisFlowDecision::done(false, iteration)
        }
    }

    async fn filter_executable_candidates(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
    ) -> ReanalysisFlowDecision {
        let (executable_changes, skipped_changes) = self.filter_executable_changes(queued);

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
            return ReanalysisFlowDecision::done(in_flight.is_empty(), iteration);
        }

        ReanalysisFlowDecision::Continue
    }

    async fn run_dependency_analysis_attempt<F>(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        iteration: u32,
        effective_reason: ReanalysisReason,
        analyzer: &F,
    ) -> DependencyAnalysisAttempt
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = AnalysisResult> + Send + 'a>>
            + Send
            + Sync,
    {
        info!(
            "Re-analysis triggered: iteration={}, queued={}, in_flight={}, trigger={}",
            iteration,
            queued.len(),
            in_flight.len(),
            effective_reason
        );
        let attempt_id = analysis_attempt_id(iteration, effective_reason, queued);
        send_event(
            &self.event_tx,
            ParallelEvent::AnalysisStarted {
                remaining_changes: queued.len(),
                attempt_id,
            },
        )
        .await;

        let in_flight_ids: Vec<String> = in_flight.iter().cloned().collect();
        let analysis_result = analyzer(queued, &in_flight_ids, iteration).await;

        if analysis_result.order.is_empty() {
            warn!("No order returned from analysis");
            return DependencyAnalysisAttempt::EmptyOrder(ReanalysisFlowDecision::done(
                in_flight.is_empty(),
                iteration,
            ));
        }

        DependencyAnalysisAttempt::Completed(analysis_result)
    }

    async fn handle_post_analysis_capacity(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        _iteration: u32,
        analysis_result: &AnalysisResult,
    ) -> Result<Option<usize>> {
        self.failed_tracker
            .set_dependencies(analysis_result.dependencies.clone());
        self.change_dependencies = analysis_result.dependencies.clone();

        let available_slots = self.calculate_available_slots(max_parallelism, in_flight);
        info!(
            "Available slots after analysis: {} (max: {}, in_flight: {}, queued: {})",
            available_slots,
            max_parallelism,
            in_flight.len(),
            queued.len()
        );

        if available_slots == 0 {
            info!(
                max_parallelism,
                in_flight = in_flight.len(),
                queued = queued.len(),
                order = ?analysis_result.order,
                "Dependency analysis completed, but dispatch is suppressed because no execution slots are available"
            );
            self.emit_capacity_zero_dispatch_diagnostic_once(
                queued,
                in_flight,
                max_parallelism,
                &analysis_result.order,
            )
            .await;
            self.emit_no_analysis_diagnostic(
                queued,
                in_flight,
                max_parallelism,
                "dispatch_capacity_zero_after_analysis",
            )
            .await;
            return Ok(None);
        }

        Ok(Some(available_slots))
    }

    async fn dispatch_selected_candidates(
        &mut self,
        ctx: DispatchSelectedCandidatesContext<'_>,
    ) -> Result<u32> {
        let DispatchSelectedCandidatesContext {
            queued,
            in_flight,
            iteration,
            analysis_result,
            available_slots,
            semaphore,
            join_set,
            cleanup_guard,
        } = ctx;
        let selected_changes = self
            .select_changes_for_dispatch(analysis_result, available_slots, in_flight)
            .await;

        if selected_changes.is_empty() {
            return Ok(iteration);
        }

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
            if self.is_cancelled() {
                info!("Stopping selected-change dispatch loop because parallel execution is cancelled");
                break;
            }
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

        let dispatched_set: HashSet<_> = selected_changes.iter().collect();
        queued.retain(|c| !dispatched_set.contains(&c.id));

        Ok(iteration + 1)
    }

    async fn before_dependency_analysis<F>(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
        reanalysis_reason: ReanalysisReason,
        analyzer: &F,
    ) -> Result<DependencyAnalysisAttempt>
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = AnalysisResult> + Send + 'a>>
            + Send
            + Sync,
    {
        if let Some((should_break, iteration)) = self
            .prepare_dispatch_candidates(queued, in_flight, max_parallelism, iteration)
            .await
            .into_result()
        {
            return Ok(DependencyAnalysisAttempt::EmptyOrder(
                ReanalysisFlowDecision::done(should_break, iteration),
            ));
        }

        let analysis_decision = self.compute_effective_reanalysis_reason(
            queued,
            in_flight,
            max_parallelism,
            reanalysis_reason,
        );
        if let Some((should_break, iteration)) = self
            .should_run_analysis_now(
                queued,
                in_flight,
                max_parallelism,
                iteration,
                analysis_decision.effective_reason,
            )
            .await
            .into_result()
        {
            return Ok(DependencyAnalysisAttempt::EmptyOrder(
                ReanalysisFlowDecision::done(should_break, iteration),
            ));
        }

        if let Some((should_break, iteration)) = self
            .filter_executable_candidates(queued, in_flight, max_parallelism, iteration)
            .await
            .into_result()
        {
            return Ok(DependencyAnalysisAttempt::EmptyOrder(
                ReanalysisFlowDecision::done(should_break, iteration),
            ));
        }

        Ok(self
            .run_dependency_analysis_attempt(
                queued,
                in_flight,
                iteration,
                analysis_decision.effective_reason,
                analyzer,
            )
            .await)
    }

    pub(super) async fn perform_reanalysis_and_dispatch<F>(
        &mut self,
        ctx: ReanalysisDispatchContext<'_, F>,
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
        let ReanalysisDispatchContext {
            queued,
            in_flight,
            max_parallelism,
            iteration,
            reanalysis_reason,
            analyzer,
            semaphore,
            join_set,
            cleanup_guard,
        } = ctx;

        if self.is_cancelled() {
            info!(
                "Skipping dependency analysis and dispatch because parallel execution is cancelled"
            );
            return Ok((true, iteration));
        }

        let analysis_result = match self
            .before_dependency_analysis(
                queued,
                in_flight,
                max_parallelism,
                iteration,
                reanalysis_reason,
                analyzer,
            )
            .await?
        {
            DependencyAnalysisAttempt::Completed(result) => result,
            DependencyAnalysisAttempt::EmptyOrder(decision) => {
                if let Some(result) = decision.into_result() {
                    return Ok(result);
                }
                unreachable!("empty analysis order always produces a terminal flow decision")
            }
        };

        let available_slots = match self
            .handle_post_analysis_capacity(
                queued,
                in_flight,
                max_parallelism,
                iteration,
                &analysis_result,
            )
            .await
        {
            Ok(Some(slots)) => slots,
            Ok(None) => return Ok((false, iteration)),
            Err(error) => return Err(error),
        };

        let new_iteration = self
            .dispatch_selected_candidates(DispatchSelectedCandidatesContext {
                queued,
                in_flight,
                iteration,
                analysis_result: &analysis_result,
                available_slots,
                semaphore,
                join_set,
                cleanup_guard,
            })
            .await?;

        Ok((false, new_iteration))
    }
}
