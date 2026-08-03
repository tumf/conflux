//! Queue state management and dispatch coordination for [`super::ParallelExecutor`].
//!
//! This module handles queue monitoring, debounce logic, dependency checking,
//! and the reanalysis/dispatch cycle that drives the parallel execution scheduler.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::dependency_targets::DependencyTargetClass;
use crate::error::{OrchestratorError, Result};
use crate::events::{ExecutionEvent, LogEntry};
use crate::orchestration::state::WaitState;
use crate::orchestration::{
    execute_rejection_flow, handle_blocked_from_rejecting, handle_resume_apply_from_rejecting,
    run_rejection_review, RejectionReviewVerdict,
};
use crate::parallel::analysis_signature::{
    build_analysis_input_signature, proposal_digest_from_path, AnalysisInputMaterials,
    AnalysisInputSignature, BoundedAnalysisRetry, BoundedRetryCause, CompletedAnalysisInput,
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
    AcceptanceStalled,
}

#[derive(Debug, Clone, Default)]
pub(super) struct BlockedOnlyQueueClassification {
    pub dispatchable: Vec<crate::openspec::Change>,
    pub manual_merge_wait: Vec<String>,
    pub scheduler_lane_wait: Vec<String>,
    pub terminal_error_retry_required: Vec<String>,
    pub dependency_blocked: Vec<String>,
    pub candidate_unavailable: Vec<String>,
    /// Changes held by a reconciled runtime acceptance stall. They are waiting
    /// work, never dispatchable, so ordinary reconciliation cannot re-submit
    /// them and produce a `Blocked -> Blocked` loop.
    pub acceptance_stalled: Vec<String>,
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
            || !self.acceptance_stalled.is_empty()
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
        } else if self.acceptance_stalled.iter().any(|id| id == change_id) {
            Some(QueuedWorkClass::AcceptanceStalled)
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

use super::cleanup::WorkspaceCleanupGuard;
use super::dependency::{DependencyContext, EffectiveDependencyBaseEvidence};
use super::dispatch::archived_dirty_repair_candidate_from_workspace;
use super::dynamic_queue::ReanalysisReason;
use super::events::send_event;
use super::merge::base_dirty_reason;
use super::{
    AlreadyReportedFailureKind, MergeResult, MergeResultDisposition, MergeResultOrigin,
    MergeTaskOutcome, ParallelEvent, ParallelExecutor, WorkspaceResult,
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
    Completed(AnalysisOutcome),
    EmptyOrder(ReanalysisFlowDecision),
}

/// One pass through the pre-analysis pipeline.
///
/// `captured_signature` is the analysis-input snapshot taken immediately before the analyzer
/// was invoked. Recording that pre-analysis value (rather than recomputing it afterwards) is
/// what keeps a queue or repository change that happened *during* analysis visible to the
/// next probe instead of being hidden by the newer state.
struct DependencyAnalysisPass {
    attempt: DependencyAnalysisAttempt,
    captured_signature: Option<AnalysisInputSignature>,
}

impl DependencyAnalysisPass {
    fn terminal(decision: ReanalysisFlowDecision) -> Self {
        Self {
            attempt: DependencyAnalysisAttempt::EmptyOrder(decision),
            captured_signature: None,
        }
    }
}

/// Whether `reason` is an explicit scheduler edge that must analyze once per event.
///
/// These are exactly the reasons that already bypass the queue-coalescing debounce, so they
/// keep a single authoritative meaning: a real state-transition event gets one immediate
/// evaluation, matching signature or not.
///
/// `Completion` is deliberately absent. It is not consumed as a one-shot edge and does not
/// bypass debounce today, so treating it as an unconditional bypass would let a retained
/// reason replay analysis on every wake. It needs no bypass either: a task completion always
/// changes in-flight membership or capacity, which changes the signature on its own.
fn bypasses_unchanged_input_gate(reason: ReanalysisReason) -> bool {
    matches!(
        reason,
        ReanalysisReason::QueueNotification
            | ReanalysisReason::SlotRecovery
            | ReanalysisReason::ResolveCompletion
            | ReanalysisReason::RepairCandidate
    )
}

/// Whether the ordinary-timer suppression gate allows this pass to invoke the analyzer.
enum UnchangedInputGate {
    /// Invoke the analyzer. `captured` is `None` when no signature could be built, in which
    /// case the pass runs fail-open and records nothing.
    Analyze {
        captured: Option<AnalysisInputSignature>,
    },
    /// The current input equals an already completed input; skip the expensive analyzer.
    Suppress,
}

/// Result of one dispatch-selection pass.
///
/// `selected` is needed by the suppression decision: a pass that selects nothing while
/// capacity is positive and nothing is in flight must stay eligible for the next debounced
/// timer evaluation.
struct DispatchSelectionOutcome {
    iteration: u32,
    selected: usize,
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
            let message = if matches!(class, DependencyTargetClass::Resolving) {
                format!(
                    "Change '{}' blocked by resolving dependency '{}' and will remain queued",
                    change_id, dep_id
                )
            } else if matches!(class, DependencyTargetClass::Archived) {
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
                | DependencyTargetClass::Resolving
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

    /// Repository-visible evidence describing the effective dependency base.
    ///
    /// Signature probing and dependency merge checks share this one resolution path, so the
    /// analysis-input signature can never be invalidated by a different ref than the one
    /// dispatch eligibility is decided from.
    pub(super) async fn effective_dependency_base_evidence(
        &self,
    ) -> Result<EffectiveDependencyBaseEvidence> {
        let mut dependency_context =
            DependencyContext::from_executor(self, std::iter::empty::<&str>(), &HashSet::new());
        dependency_context
            .effective_dependency_base_evidence(self.workspace_manager.as_ref())
            .await
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
            let proposal_path = self
                .repo_root
                .join("openspec/changes")
                .join(change_id)
                .join("proposal.md");
            let metadata_dependencies = if self.repo_root.join("openspec/changes").exists() {
                match crate::openspec::parse_proposal_dependencies_strict_from_file(&proposal_path)
                {
                    Ok(dependencies) => dependencies,
                    Err(error) => {
                        let message = format!(
                            "Change '{}' blocked because dependency metadata could not be read from '{}': {}",
                            change_id,
                            proposal_path.display(),
                            error
                        );
                        warn!("{}", message);
                        if self.should_emit_dependency_blocked_transition(
                            change_id,
                            &[(change_id.clone(), DependencyTargetClass::Error)],
                        ) {
                            send_event(&self.event_tx, ParallelEvent::Error { message }).await;
                            send_event(
                                &self.event_tx,
                                ParallelEvent::DependencyBlocked {
                                    change_id: change_id.clone(),
                                    dependency_ids: vec![change_id.clone()],
                                },
                            )
                            .await;
                        }
                        continue;
                    }
                }
            } else {
                Vec::new()
            };
            let mut dependencies = analysis_result
                .dependencies
                .get(change_id)
                .cloned()
                .unwrap_or_default();
            for dep_id in metadata_dependencies {
                if dep_id != *change_id && !dependencies.contains(&dep_id) {
                    dependencies.push(dep_id);
                }
            }

            if !dependencies.is_empty() {
                let mut unresolved_deps = Vec::new();
                let mut blockers = Vec::new();
                for dep_id in &dependencies {
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
                        DependencyTargetClass::Resolving => {
                            unresolved_deps.push(dep_id.clone());
                            blockers.push((dep_id.clone(), class));
                            continue;
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
            if let Err(e) = self
                .workspace_manager
                .cleanup_workspace(&workspace_result.workspace_name)
                .await
            {
                error!(
                    "Failed to cleanup rejected workspace '{}' for change '{}': {}",
                    workspace_result.workspace_name, workspace_result.change_id, e
                );
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
                        outcome: MergeTaskOutcome::deferred(
                            "duplicate post-archive merge task suppressed because the same change is already active",
                            true,
                        ),
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
                .await;

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
    pub(super) async fn handle_merge_result(
        &mut self,
        merge_result: MergeResult,
    ) -> MergeResultDisposition {
        let (merge_result_tx, _merge_result_rx) = mpsc::channel(1);
        self.handle_merge_result_with_tx(merge_result, &merge_result_tx)
            .await
    }

    /// Apply one typed background base-lane result and return its scheduler disposition.
    ///
    /// Ownership release comes first and is independent of severity: the pending
    /// counter and, for a spawned retry, the base-mutating lane are returned to
    /// the scheduler before the disposition is decided, so a failure can never
    /// strand the lane it borrowed.
    ///
    /// This is also the single global-Error owner for the background base-lane
    /// boundary. Only [`MergeTaskOutcome::RunFatal`] emits one, exactly once,
    /// and it always comes with `AbortRun`; every change-local outcome already
    /// has an authoritative change-scoped owner upstream and must not be
    /// promoted here.
    pub(super) async fn handle_merge_result_with_tx(
        &mut self,
        merge_result: MergeResult,
        merge_result_tx: &mpsc::Sender<MergeResult>,
    ) -> MergeResultDisposition {
        self.pending_merge_count.fetch_sub(1, Ordering::Relaxed);

        let disposition = merge_result.outcome.disposition();

        // Lane release for spawned retries happens for every non-merged outcome,
        // ahead of the disposition-specific handling below. The merged path
        // instead promotes the next waiter, which is its own release path.
        if !disposition.is_merged() {
            let releases_retry_lane = match &merge_result.outcome {
                MergeTaskOutcome::Deferred { auto_resumable, .. } => *auto_resumable,
                _ => true,
            };
            if releases_retry_lane {
                self.release_retry_lane_after_non_terminal_outcome(
                    &merge_result.change_id,
                    merge_result.origin,
                )
                .await;
            }
        }

        match &merge_result.outcome {
            MergeTaskOutcome::Merged => {
                info!(
                    origin = ?merge_result.origin,
                    "Background base-lane task completed successfully for '{}'",
                    merge_result.change_id
                );
                self.dispatch_deferred_base_lane_waiter(merge_result_tx.clone())
                    .await;
            }

            MergeTaskOutcome::Deferred {
                reason,
                auto_resumable,
            } => {
                info!(
                    "Background merge task deferred for '{}' (workspace '{}', auto_resumable={}): {}",
                    merge_result.change_id, merge_result.workspace_name, auto_resumable, reason
                );
            }

            MergeTaskOutcome::ResolveExhausted {
                change_id,
                attempts,
                classification,
                detail,
            } => {
                // `ResolveFailed` already carried this to the reducer with the
                // change ID attached, and the worktree is preserved for explicit
                // retry. Wrapping it in a global Error would erase both facts.
                warn!(
                    change_id = %change_id,
                    workspace = %merge_result.workspace_name,
                    origin = ?merge_result.origin,
                    attempts = attempts,
                    classification = classification.token(),
                    "Bounded post-archive resolve exhausted; change remains in merge wait for explicit retry: {}",
                    detail
                );
            }

            MergeTaskOutcome::RecoverableAlreadyReported {
                change_id,
                kind,
                detail,
            } => {
                warn!(
                    change_id = %change_id,
                    workspace = %merge_result.workspace_name,
                    origin = ?merge_result.origin,
                    owner = kind.token(),
                    "Background base-lane failure already reported by its typed owner: {}",
                    detail
                );
            }

            MergeTaskOutcome::RunFatal { detail } => {
                error!(
                    change_id = %merge_result.change_id,
                    workspace = %merge_result.workspace_name,
                    origin = ?merge_result.origin,
                    "Background base-lane task failed fatally: {}",
                    detail
                );
                let message = format!(
                    "Background merge failed for '{}' (workspace '{}'): {}",
                    merge_result.change_id, merge_result.workspace_name, detail
                );
                // One global Error per aborting run: a later fatal result that
                // arrives during the bounded drain must not add a second.
                if self.run_fatal_abort.is_none() {
                    self.run_fatal_abort = Some(message.clone());
                    send_event(&self.event_tx, ParallelEvent::Error { message }).await;
                }
            }
        }

        if disposition == MergeResultDisposition::ContinueWithErrors {
            let failed_change = merge_result
                .outcome
                .scoped_change_id()
                .unwrap_or(merge_result.change_id.as_str())
                .to_string();
            self.change_failures_this_run.insert(failed_change);
        }

        disposition
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
    ) -> MergeTaskOutcome {
        let mut outcome = MergeTaskOutcome::Merged;
        for change_id in deferred.into_iter().take(1) {
            // A change with durable publication-required evidence is already in
            // cumulative base but is *not* done: retry must resume publication
            // rather than be discarded as a stale already-merged retry.
            let owes_publication = self.has_pending_publication_for(&change_id).await;
            if !owes_publication && self.is_change_already_merged_to_base(&change_id).await {
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
                    // Stale bookkeeping, not a failure: there is no workspace
                    // left to retry, so the intent is cleared and the run
                    // continues. A global Error here would claim the run is
                    // invalid when nothing is.
                    let message = format!(
                        "No workspace found for ResolveWait retry '{}', clearing stale retry intent",
                        change_id
                    );
                    warn!("{}", message);
                    send_event(
                        &self.event_tx,
                        ParallelEvent::Log(LogEntry::warn(&message).with_change_id(&change_id)),
                    )
                    .await;
                    // Remove from deferred set; the workspace is gone, nothing to retry.
                    self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                    self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                        .await;
                    outcome = MergeTaskOutcome::Merged;
                    continue;
                }
                Err(e) => {
                    // A repository query failed before any change-scoped
                    // transition could be established, so scope is unknown and
                    // the outcome fails closed to the single global owner.
                    let message = format!(
                        "Failed to find workspace for ResolveWait retry '{}': {}",
                        change_id, e
                    );
                    warn!("{}", message);
                    outcome = MergeTaskOutcome::run_fatal(message);
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
                outcome = MergeTaskOutcome::Merged;
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

                    // With upstream publication enabled the base-lane sequence
                    // already ran `on_merged` and emitted the change-scoped
                    // publication events; local merge is not terminal there.
                    let publication_owned_completion = self.upstream_enabled();
                    let retry_hooks = if publication_owned_completion {
                        None
                    } else {
                        self.hooks.as_ref()
                    };

                    // Run on_merged hook before merged status transition (MergeCompleted event).
                    if let Some(hooks) = retry_hooks {
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
                            // `HookFailed` owns this transition; the queue must
                            // not promote it into a run-fatal outcome.
                            outcome = MergeTaskOutcome::already_reported(
                                &change_id,
                                AlreadyReportedFailureKind::Hook,
                                message,
                            );
                            continue;
                        }
                    }

                    self.clear_resolve_wait_intent_for_outcome(&change_id).await;
                    if !publication_owned_completion {
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
                    }

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
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace_info.workspace_name.clone(),
                            },
                        )
                        .await;
                    }
                    outcome = MergeTaskOutcome::Merged;
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
                    outcome = MergeTaskOutcome::deferred(reason, auto_resumable);
                }
                Err(failure) => {
                    error!(
                        change_id = %change_id,
                        outcome = ?failure,
                        "Deferred merge retry failed"
                    );
                    // Keep in deferred set; another merge/resolve completion will
                    // trigger again. The typed failure already knows its scope.
                    outcome = failure.into_outcome(&change_id);
                }
            }
        }
        outcome
    }

    pub(super) async fn retry_deferred_rejection_review_for(
        &mut self,
        change_id: String,
    ) -> MergeTaskOutcome {
        let mut outcome = MergeTaskOutcome::Merged;
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
                // Stale bookkeeping, not a failure: nothing is left to review.
                let message = format!(
                    "No workspace found for RejectWait retry '{}', clearing reject wait",
                    change_id
                );
                warn!("{}", message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(&message).with_change_id(&change_id)),
                )
                .await;
                self.clear_reject_wait_intent_for_success(&change_id).await;
                self.abandon_base_mutating_lane_occupant_for_give_up(&change_id)
                    .await;
                return MergeTaskOutcome::Merged;
            }
            Err(e) => {
                // Repository query failure before any change-scoped transition:
                // scope is unknown, so it fails closed.
                let message = format!(
                    "Failed to find workspace for RejectWait retry '{}': {}",
                    change_id, e
                );
                warn!("{}", message);
                return MergeTaskOutcome::run_fatal(message);
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
                        outcome = MergeTaskOutcome::already_reported(
                            &change_id,
                            AlreadyReportedFailureKind::RejectionReview,
                            error.to_string(),
                        );
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
                        outcome = MergeTaskOutcome::already_reported(
                            &change_id,
                            AlreadyReportedFailureKind::RejectionReview,
                            error.to_string(),
                        );
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
                outcome = MergeTaskOutcome::already_reported(
                    &change_id,
                    AlreadyReportedFailureKind::RejectionReview,
                    error.to_string(),
                );
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
            if last_change.is_none() {
                *last_change = Some(std::time::Instant::now());
                debug!(
                    queued_added = outcome.queued_added,
                    "Queue reconciliation initialized reducer-visible queue debounce timestamp"
                );
            } else {
                debug!(
                    queued_added = outcome.queued_added,
                    "Queue reconciliation preserved existing queue debounce timestamp"
                );
            }
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
        let mut dependency_context = DependencyContext::from_executor(
            self,
            queued.iter().map(|change| change.id.as_str()),
            in_flight,
        );

        // Acceptance holds and validated external prerequisite waits are both
        // reducer-owned in-memory state, so they are read from the same snapshot
        // as the other wait lanes. Nothing is loaded from disk: after a restart
        // the reducer holds neither, the change is dispatched again, and
        // workspace evidence routes a complete unarchived apply revision back to
        // acceptance.
        let (
            reducer_queued,
            merge_wait_ids,
            resolve_wait_ids,
            reject_wait_ids,
            acceptance_stalled,
            externally_blocked,
        ) = self
            .shared_orchestrator_state
            .as_ref()
            .and_then(|state| state.try_read().ok())
            .map(|state| {
                (
                    state.queued_change_ids(),
                    state.merge_wait_change_ids(),
                    state.resolve_wait_change_ids(),
                    state.reject_wait_change_ids(),
                    state.acceptance_stalled_change_ids(),
                    state.externally_blocked_change_ids(),
                )
            })
            .unwrap_or_default();
        // An apply-origin external blocker suppresses dispatch exactly like an
        // acceptance-origin one; only the explanation differs.
        let held: HashSet<String> = acceptance_stalled
            .into_iter()
            .chain(externally_blocked)
            .collect();
        let acceptance_stalled = held;

        let merge_wait_set: HashSet<String> = merge_wait_ids.into_iter().collect();
        let resolve_wait_set: HashSet<String> = resolve_wait_ids.into_iter().collect();
        let reject_wait_set: HashSet<String> = reject_wait_ids.into_iter().collect();

        for change in queued {
            seen_ids.insert(change.id.clone());
            if acceptance_stalled.contains(&change.id) {
                classification.acceptance_stalled.push(change.id.clone());
                continue;
            }
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

            let proposal_path = self
                .repo_root
                .join("openspec/changes")
                .join(&change.id)
                .join("proposal.md");
            let metadata_dependencies = if self.repo_root.join("openspec/changes").exists() {
                match crate::openspec::parse_proposal_dependencies_strict_from_file(&proposal_path)
                {
                    Ok(dependencies) => dependencies,
                    Err(error) => {
                        warn!(
                            change_id = %change.id,
                            proposal = %proposal_path.display(),
                            error = %error,
                            "Blocking queue classification because dependency metadata could not be read"
                        );
                        classification.dependency_blocked.push(change.id.clone());
                        continue;
                    }
                }
            } else {
                Vec::new()
            };
            let mut dependencies = change.dependencies.clone();
            for dep_id in metadata_dependencies {
                if dep_id != change.id && !dependencies.contains(&dep_id) {
                    dependencies.push(dep_id);
                }
            }
            let blocked = dependency_context
                .is_blocked(&dependencies, self.workspace_manager.as_ref())
                .await
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
        if !self.resolve_wait_changes.is_empty() || !self.reject_wait_changes.is_empty() {
            return false;
        }

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
        queued: &[crate::openspec::Change],
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
                acceptance_stalled = classification.acceptance_stalled.len(),
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

        // Keep temporarily dependency-blocked candidates in `queued`: a later reanalysis,
        // such as ResolveCompletion, must re-evaluate them after repository-visible evidence
        // changes. Dispatch selection applies the per-pass gate without dropping them.
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

    /// Probe the repository-visible material a signature needs.
    ///
    /// Returns `Err` with an operator-visible reason on any read or revision failure so the
    /// caller can fail open rather than suppress analysis on partial input.
    pub(super) async fn probe_analysis_signature_materials(
        &self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
    ) -> std::result::Result<(String, std::collections::HashMap<String, String>), String> {
        let referenced_ids: HashSet<&str> = queued
            .iter()
            .map(|change| change.id.as_str())
            .chain(in_flight.iter().map(String::as_str))
            .collect();

        match &self.analysis_input_probe {
            Some(probe) => {
                let base_revision = probe.base_revision().await?;
                let mut digests = std::collections::HashMap::new();
                for change_id in referenced_ids {
                    digests.insert(change_id.to_string(), probe.proposal_digest(change_id)?);
                }
                Ok((base_revision, digests))
            }
            None => {
                // The named effective dependency base, not the checkout commit: dependency
                // merge evidence is read from that ref, and it can advance while `HEAD` does
                // not.
                let base_revision = self
                    .effective_dependency_base_evidence()
                    .await
                    .map_err(|error| error.to_string())?
                    .signature_material();
                let mut digests = std::collections::HashMap::new();
                for change_id in referenced_ids {
                    let path = self
                        .repo_root
                        .join("openspec/changes")
                        .join(change_id)
                        .join("proposal.md");
                    digests.insert(change_id.to_string(), proposal_digest_from_path(&path)?);
                }
                Ok((base_revision, digests))
            }
        }
    }

    /// Build the current analysis-input signature, or `None` when it cannot be built.
    ///
    /// Signature construction is fail-open by design: a proposal read or revision resolution
    /// error emits a deduplicated warning and yields no signature, which permits analysis and
    /// records nothing. It must never terminate the scheduler loop.
    async fn current_analysis_input_signature(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        available_slots: usize,
    ) -> Option<AnalysisInputSignature> {
        match self
            .probe_analysis_signature_materials(queued, in_flight)
            .await
        {
            Ok((base_revision, proposal_digests)) => {
                let in_flight_ids: Vec<String> = in_flight.iter().cloned().collect();
                Some(build_analysis_input_signature(&AnalysisInputMaterials {
                    queued,
                    in_flight_ids: &in_flight_ids,
                    available_slots,
                    max_parallelism,
                    base_revision: &base_revision,
                    proposal_digests: &proposal_digests,
                }))
            }
            Err(error) => {
                warn!(
                    error = %error,
                    queued = queued.len(),
                    in_flight = in_flight.len(),
                    "Could not build dependency-analysis input signature; permitting analysis without suppression"
                );
                let key = DiagnosticDeduplicationKey::AnalysisSignatureUnavailable {
                    queued_len: queued.len(),
                    in_flight_len: in_flight.len(),
                    error: error.clone(),
                };
                let log_entry = LogEntry::warn(format!(
                    "Dependency-analysis input signature unavailable; analysis proceeds without unchanged-input suppression: error={}, queued={}, in_flight={}",
                    error,
                    queued.len(),
                    in_flight.len()
                ));
                self.emit_log_diagnostic_once(
                    key,
                    log_entry,
                    "Suppressing repeated analysis-signature-unavailable diagnostic",
                )
                .await;
                None
            }
        }
    }

    /// Decide whether an ordinary timer evaluation may invoke the expensive analyzer.
    ///
    /// This runs *after* queue classification, reconciliation, and debounce eligibility, so
    /// cheap repository-visible work is never cached — only duplicate analyzer invocation is
    /// removed. Explicit scheduler edges never reach this gate.
    async fn check_unchanged_analysis_input(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        effective_reason: ReanalysisReason,
    ) -> UnchangedInputGate {
        let available_slots = self.calculate_available_slots(max_parallelism, in_flight);

        if bypasses_unchanged_input_gate(effective_reason) {
            // An explicit edge gets one immediate evaluation per event even when its input
            // signature matches or a bounded retry deadline is still pending. Its signature is
            // still captured so that once the edge is consumed, later timer-only wakes fall
            // back to unchanged-input suppression or to bounded fail-open retry.
            let captured = self
                .current_analysis_input_signature(
                    queued,
                    in_flight,
                    max_parallelism,
                    available_slots,
                )
                .await;
            self.note_signature_probe_outcome(captured.is_some());
            return UnchangedInputGate::Analyze { captured };
        }

        let now = tokio::time::Instant::now();

        // A previous attempt established no completed input. Fail-open must not mean fail-hot:
        // before the bounded deadline, an ordinary wake performs no probe and no analysis.
        if let Some(throttle) = self.analysis_retry_throttle {
            if throttle.blocks(now) {
                self.emit_bounded_retry_diagnostic(
                    queued,
                    in_flight,
                    max_parallelism,
                    throttle.cause(),
                )
                .await;
                return UnchangedInputGate::Suppress;
            }
            // The deadline passed: this evaluation retries both signature construction and
            // analysis.
            self.analysis_retry_throttle = None;
        }

        // While an input is suppressed, only re-probe at the bounded cadence. A 500 ms wake
        // before the deadline must not read proposal files or spawn a VCS subprocess.
        if let Some(probe_deadline) = self.next_analysis_signature_probe_at {
            if now < probe_deadline {
                debug!("Skipping dependency-analysis signature probe before its bounded deadline");
                return UnchangedInputGate::Suppress;
            }
        }

        let available_slots = self.calculate_available_slots(max_parallelism, in_flight);
        let Some(signature) = self
            .current_analysis_input_signature(queued, in_flight, max_parallelism, available_slots)
            .await
        else {
            self.note_signature_probe_outcome(false);
            return UnchangedInputGate::Analyze { captured: None };
        };

        let suppressing_record = self
            .last_completed_analysis_input
            .as_ref()
            .filter(|record| record.suppresses(&signature, now));

        match suppressing_record {
            Some(record) => {
                let degraded = record.is_degraded();
                self.next_analysis_signature_probe_at = Some(record.next_probe_deadline(now));
                self.emit_unchanged_analysis_input_diagnostic(
                    queued,
                    in_flight,
                    max_parallelism,
                    &signature,
                    degraded,
                )
                .await;
                UnchangedInputGate::Suppress
            }
            None => {
                self.note_signature_probe_outcome(true);
                UnchangedInputGate::Analyze {
                    captured: Some(signature),
                }
            }
        }
    }

    /// Apply the retry policy implied by one signature-probe attempt.
    ///
    /// A usable signature clears any pending fail-open throttle, because the next pass can rely
    /// on ordinary completed-input suppression instead. An unavailable signature arms the
    /// bounded deadline so persistent proposal-read or revision-resolution failure retries at
    /// the ten-second cadence rather than on every 500 ms wake.
    fn note_signature_probe_outcome(&mut self, signature_available: bool) {
        self.next_analysis_signature_probe_at = None;
        self.analysis_retry_throttle = if signature_available {
            None
        } else {
            Some(BoundedAnalysisRetry::after(
                BoundedRetryCause::SignatureUnavailable,
                tokio::time::Instant::now(),
            ))
        };
    }

    /// Arm the bounded retry deadline after an attempt that produced no usable result.
    fn arm_bounded_analysis_retry(&mut self, cause: BoundedRetryCause) {
        self.next_analysis_signature_probe_at = None;
        self.analysis_retry_throttle = Some(BoundedAnalysisRetry::after(
            cause,
            tokio::time::Instant::now(),
        ));
    }

    /// Operator-visible reason for a wake that is waiting on a bounded fail-open retry.
    async fn emit_bounded_retry_diagnostic(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        cause: BoundedRetryCause,
    ) {
        debug!(
            queued = queued.len(),
            in_flight = in_flight.len(),
            cause = cause.as_str(),
            "Waiting for the bounded dependency-analysis retry deadline"
        );
        let key = DiagnosticDeduplicationKey::BoundedAnalysisRetryPending {
            cause: cause.as_str(),
            queued_len: queued.len(),
            in_flight_len: in_flight.len(),
        };
        let log_entry = LogEntry::info(format!(
            "No analysis started: reason={}, queued={}, in_flight={}, max_parallelism={}",
            cause.as_str(),
            queued.len(),
            in_flight.len(),
            max_parallelism
        ));
        self.emit_log_diagnostic_once(
            key,
            log_entry,
            "Suppressing repeated bounded-analysis-retry diagnostic",
        )
        .await;
        self.emit_no_analysis_diagnostic(queued, in_flight, max_parallelism, cause.as_str())
            .await;
    }

    async fn emit_unchanged_analysis_input_diagnostic(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        signature: &AnalysisInputSignature,
        degraded: bool,
    ) {
        info!(
            queued = queued.len(),
            in_flight = in_flight.len(),
            degraded,
            "Skipping dependency analysis because the analysis input is unchanged since the last completed analysis"
        );
        let key = DiagnosticDeduplicationKey::UnchangedAnalysisInput {
            signature: signature.as_str().to_string(),
            queued_len: queued.len(),
            in_flight_len: in_flight.len(),
            degraded,
        };
        let log_entry = LogEntry::info(format!(
            "No analysis started: reason=unchanged_analysis_input, degraded={}, queued={}, in_flight={}, max_parallelism={}",
            degraded,
            queued.len(),
            in_flight.len(),
            max_parallelism
        ));
        self.emit_log_diagnostic_once(
            key,
            log_entry,
            "Suppressing repeated unchanged-analysis-input diagnostic",
        )
        .await;
        self.emit_no_analysis_diagnostic(
            queued,
            in_flight,
            max_parallelism,
            "unchanged_analysis_input",
        )
        .await;
    }

    /// Record a completed analysis input so unchanged ordinary timer wakes stay quiescent.
    ///
    /// `captured` is the pre-analysis snapshot. A recoverable-failure fallback is recorded as
    /// degraded, so a broken analyzer command is not relaunched on every wake but still gets
    /// one retry after the fixed degraded interval.
    fn record_completed_analysis_input(
        &mut self,
        captured: Option<AnalysisInputSignature>,
        provenance: crate::analyzer::AnalysisProvenance,
    ) {
        let Some(signature) = captured else {
            return;
        };

        let now = tokio::time::Instant::now();
        let record = if provenance.is_degraded() {
            CompletedAnalysisInput::degraded(signature, now)
        } else {
            CompletedAnalysisInput::healthy(signature)
        };
        // Arm the probe deadline at record time so the immediate 500 ms wake after a completed
        // analysis performs no proposal read and spawns no VCS subprocess just to confirm the
        // input it was already derived from.
        self.next_analysis_signature_probe_at = Some(record.next_probe_deadline(now));
        self.last_completed_analysis_input = Some(record);
        // A usable result supersedes any bounded fail-open retry.
        self.analysis_retry_throttle = None;
        debug!(
            degraded = provenance.is_degraded(),
            "Recorded completed dependency-analysis input signature"
        );
    }

    /// Drop any suppression for the current input.
    ///
    /// Used when an analysis pass produced no usable dispatch decision while the scheduler was
    /// otherwise idle: an erroneous non-deterministic dependency result must not be able to
    /// freeze an idle scheduler permanently.
    fn forget_completed_analysis_input(&mut self) {
        self.last_completed_analysis_input = None;
        self.next_analysis_signature_probe_at = None;
        self.analysis_retry_throttle = None;
    }

    async fn should_run_analysis_now(
        &mut self,
        queued: &[crate::openspec::Change],
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
        effective_reason: ReanalysisReason,
    ) -> ReanalysisFlowDecision {
        let bypass_debounce = bypasses_unchanged_input_gate(effective_reason);

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
                -> std::pin::Pin<Box<dyn std::future::Future<Output = AnalysisOutcome> + Send + 'a>>
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

        // The analyzer prompt must not depend on `HashSet` iteration order: the signature treats
        // in-flight membership as a set, so the consumer has to see one deterministic ordering
        // of that same set.
        let mut in_flight_ids: Vec<String> = in_flight.iter().cloned().collect();
        in_flight_ids.sort_unstable();
        let analysis_outcome = analyzer(queued, &in_flight_ids, iteration).await;

        if analysis_outcome.result.order.is_empty() {
            // An unusable analysis decision must not establish a completed input, or it would
            // suppress every later attempt. It is also not evidence that queued work finished:
            // ending the scheduler here would strand reducer-visible queued intent. Keep the
            // loop alive, let the canonical drain checks own termination, and rate-limit the
            // retry so a persistently unusable analyzer is not relaunched every 500 ms.
            warn!("No order returned from analysis");
            let queued_work_remains = !queued.is_empty();
            if queued_work_remains {
                self.arm_bounded_analysis_retry(BoundedRetryCause::UnusableAnalysisResult);
            }
            return DependencyAnalysisAttempt::EmptyOrder(ReanalysisFlowDecision::done(
                !queued_work_remains && in_flight.is_empty(),
                iteration,
            ));
        }

        DependencyAnalysisAttempt::Completed(analysis_outcome)
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
    ) -> Result<DispatchSelectionOutcome> {
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
            return Ok(DispatchSelectionOutcome {
                iteration,
                selected: 0,
            });
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

        Ok(DispatchSelectionOutcome {
            iteration: iteration + 1,
            selected: selected_changes.len(),
        })
    }

    async fn before_dependency_analysis<F>(
        &mut self,
        queued: &mut Vec<crate::openspec::Change>,
        in_flight: &HashSet<String>,
        max_parallelism: usize,
        iteration: u32,
        reanalysis_reason: ReanalysisReason,
        analyzer: &F,
    ) -> Result<DependencyAnalysisPass>
    where
        for<'a> F: Fn(
                &'a [crate::openspec::Change],
                &'a [String],
                u32,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = AnalysisOutcome> + Send + 'a>>
            + Send
            + Sync,
    {
        if let Some((should_break, iteration)) = self
            .prepare_dispatch_candidates(queued, in_flight, max_parallelism, iteration)
            .await
            .into_result()
        {
            return Ok(DependencyAnalysisPass::terminal(
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
            return Ok(DependencyAnalysisPass::terminal(
                ReanalysisFlowDecision::done(should_break, iteration),
            ));
        }

        if let Some((should_break, iteration)) = self
            .filter_executable_candidates(queued, in_flight, max_parallelism, iteration)
            .await
            .into_result()
        {
            return Ok(DependencyAnalysisPass::terminal(
                ReanalysisFlowDecision::done(should_break, iteration),
            ));
        }

        // Queue classification, reconciliation, dependency blocker checks, and debounce
        // eligibility have all run against fresh repository state by now. Only the expensive
        // analyzer invocation is subject to unchanged-input suppression.
        let captured_signature = match self
            .check_unchanged_analysis_input(
                queued,
                in_flight,
                max_parallelism,
                analysis_decision.effective_reason,
            )
            .await
        {
            UnchangedInputGate::Analyze { captured } => captured,
            UnchangedInputGate::Suppress => {
                return Ok(DependencyAnalysisPass::terminal(
                    ReanalysisFlowDecision::done(false, iteration),
                ));
            }
        };

        let attempt = self
            .run_dependency_analysis_attempt(
                queued,
                in_flight,
                iteration,
                analysis_decision.effective_reason,
                analyzer,
            )
            .await;

        Ok(DependencyAnalysisPass {
            attempt,
            captured_signature,
        })
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
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
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

        let pass = self
            .before_dependency_analysis(
                queued,
                in_flight,
                max_parallelism,
                iteration,
                reanalysis_reason,
                analyzer,
            )
            .await?;
        let captured_signature = pass.captured_signature;
        let analysis_outcome = match pass.attempt {
            DependencyAnalysisAttempt::Completed(outcome) => outcome,
            DependencyAnalysisAttempt::EmptyOrder(decision) => {
                if let Some(result) = decision.into_result() {
                    return Ok(result);
                }
                unreachable!("empty analysis order always produces a terminal flow decision")
            }
        };
        let provenance = analysis_outcome.provenance;
        let analysis_result = analysis_outcome.result;

        let was_idle_before_dispatch = in_flight.is_empty();
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
            Ok(None) => {
                // Zero capacity with a usable analysis result is the state that produced the
                // live no-progress loop: the input was genuinely analyzed, so record it and
                // let capacity recovery change the signature later.
                self.record_completed_analysis_input(captured_signature, provenance);
                return Ok((false, iteration));
            }
            Err(error) => return Err(error),
        };

        let dispatch = self
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

        if dispatch.selected == 0 && was_idle_before_dispatch {
            // Positive capacity, nothing in flight, and nothing selected: an erroneous
            // non-deterministic dependency result must not be able to freeze an otherwise idle
            // scheduler, so leave this input eligible for the next debounced evaluation.
            info!(
                queued = queued.len(),
                available_slots,
                "Analysis selected no dispatch while idle with positive capacity; leaving the input eligible for re-analysis"
            );
            self.forget_completed_analysis_input();
        } else {
            self.record_completed_analysis_input(captured_signature, provenance);
        }

        Ok((false, dispatch.iteration))
    }
}
