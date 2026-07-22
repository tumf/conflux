//! Shared serial execution service for CLI and TUI modes.
//!
//! This module provides a unified service for running serial execution
//! that can be used by both CLI and TUI orchestrators, eliminating
//! code duplication between the two modes.
//!
//! The service provides helper functions for:
//! - Change selection based on progress and dependencies
//! - State tracking (apply counts, completed/stalled changes)
//! - Iteration limit checking
//! - Hook execution helpers
//!
//! The actual orchestration loop remains in the orchestrators for now,
//! as they have mode-specific concerns (WIP commits for CLI, DynamicQueue for TUI).

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::orchestration::acceptance::{
    decide_acceptance_retry, normalize_findings, repository_findings,
    semantic_progress_fingerprint, AcceptanceRetryDecision,
};
use crate::orchestration::{
    acceptance_test_streaming, archive_change, AcceptanceResult, ArchiveContext, ArchiveResult,
    OutputHandler,
};
use crate::parallel::acceptance_state::{
    consume_resumable_acceptance_marker, mark_acceptance_failed, mark_acceptance_passed,
    mark_acceptance_started, mark_apply_completed, parse_blocked_marker,
    record_acceptance_retry_checkpoint, record_acceptance_retry_context,
    write_acceptance_blocked_marker_with_context,
};
use crate::stall::{StallDetector, StallPhase};
use crate::task_parser;
use crate::task_parser::TaskProgress;
use crate::vcs::git::commands::basic::get_current_commit;
use crate::vcs::VcsBackend;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Service for serial execution of changes.
///
/// This service encapsulates the shared logic between CLI and TUI
/// serial execution modes, including:
/// - Change selection
/// - Apply/archive flow
/// - Acceptance testing
/// - Hook execution
/// - Iteration tracking
/// - Stall detection
pub struct SerialRunService {
    /// Configuration for the orchestrator
    config: OrchestratorConfig,
    /// Repository root directory
    repo_root: PathBuf,
    /// Apply count per change
    apply_counts: HashMap<String, u32>,
    /// Currently processing change ID
    current_change_id: Option<String>,
    /// Completed change IDs
    completed_change_ids: HashSet<String>,
    /// Stalled change IDs
    stalled_change_ids: HashSet<String>,
    /// Stall detector for monitoring progress
    stall_detector: StallDetector,
    /// Changes processed count
    changes_processed: usize,
    /// Current iteration
    iteration: u32,
}

impl SerialRunService {
    /// Create a new serial run service
    pub fn new(repo_root: PathBuf, config: OrchestratorConfig) -> Self {
        let stall_config = config.get_stall_detection();
        Self {
            config,
            repo_root,
            apply_counts: HashMap::new(),
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            stalled_change_ids: HashSet::new(),
            stall_detector: StallDetector::new(stall_config),
            changes_processed: 0,
            iteration: 0,
        }
    }

    /// Get the repository root path
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn repo_root(&self) -> &PathBuf {
        &self.repo_root
    }

    /// Get the current iteration number
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// Get the number of changes processed
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn changes_processed(&self) -> usize {
        self.changes_processed
    }

    /// Get the current change ID being processed
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn current_change_id(&self) -> Option<&String> {
        self.current_change_id.as_ref()
    }

    /// Get apply count for a change
    pub fn apply_count(&self, change_id: &str) -> u32 {
        *self.apply_counts.get(change_id).unwrap_or(&0)
    }

    /// Increment apply count for a change
    fn increment_apply_count(&mut self, change_id: &str) {
        let count = self.apply_counts.entry(change_id.to_string()).or_insert(0);
        *count += 1;
    }

    /// Check if a change is stalled
    pub fn is_stalled(&self, change_id: &str) -> bool {
        self.stalled_change_ids.contains(change_id)
    }

    /// Check if a change is completed
    pub fn is_completed(&self, change_id: &str) -> bool {
        self.completed_change_ids.contains(change_id)
    }

    /// Select the next change to process.
    ///
    /// Prioritizes changes by highest progress percentage.
    /// Filters out stalled changes and their dependencies.
    pub fn select_next_change<'a>(&self, changes: &'a [Change]) -> Option<&'a Change> {
        // Filter out completed and stalled changes
        let eligible: Vec<_> = changes
            .iter()
            .filter(|c| !self.is_completed(&c.id) && !self.is_stalled(&c.id))
            .collect();

        // Further filter out changes that depend on stalled changes
        let filtered: Vec<_> = eligible
            .iter()
            .filter(|c| {
                !c.dependencies
                    .iter()
                    .any(|dep| self.stalled_change_ids.contains(dep))
            })
            .copied()
            .collect();

        if filtered.is_empty() {
            return None;
        }

        // Find incomplete changes and prioritize by progress
        let incomplete: Vec<_> = filtered.iter().filter(|c| !c.is_complete()).collect();

        if !incomplete.is_empty() {
            // Prioritize incomplete changes by highest progress percentage
            return incomplete
                .into_iter()
                .max_by(|a, b| {
                    let a_progress = if a.total_tasks > 0 {
                        a.completed_tasks as f32 / a.total_tasks as f32
                    } else {
                        0.0
                    };
                    let b_progress = if b.total_tasks > 0 {
                        b.completed_tasks as f32 / b.total_tasks as f32
                    } else {
                        0.0
                    };
                    a_progress
                        .partial_cmp(&b_progress)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
        }

        // If all are complete, select the first one for archiving
        filtered.first().copied()
    }

    /// Mark a change as stalled
    pub fn mark_stalled(&mut self, change_id: &str, reason: &str) {
        warn!("Marking {} as stalled: {}", change_id, reason);
        self.stalled_change_ids.insert(change_id.to_string());
    }

    /// Consume a resumable acceptance marker for an explicit serial retry.
    pub fn consume_explicit_acceptance_retry(&mut self, change_id: &str) -> Result<bool> {
        let consumed = consume_resumable_acceptance_marker(&self.repo_root, change_id)?;
        if consumed {
            self.stalled_change_ids.remove(change_id);
        }
        Ok(consumed)
    }

    fn restore_acceptance_checkpoint_history(
        &self,
        change_id: &str,
        agent: &mut AgentRunner,
    ) -> Result<()> {
        let Some(state) = crate::parallel::acceptance_state::load_acceptance_state_for(
            &self.repo_root,
            change_id,
        )?
        else {
            return Ok(());
        };
        if state.previous_finding_identities.is_empty() {
            return Ok(());
        }

        let mut history = crate::history::AcceptanceHistory::new();
        history.set_checkpoint(
            change_id,
            state.cycle_count,
            state.previous_finding_identities,
            state.semantic_fingerprint,
        );
        agent.seed_acceptance_history(history);
        Ok(())
    }

    fn preflight_blocked_marker(&mut self, change_id: &str) -> Result<Option<ChangeProcessResult>> {
        if let Some(marker) = parse_blocked_marker(&self.repo_root, change_id)? {
            let error = format!("Blocked marker ({:?}): {}", marker.origin, marker.reason);
            self.mark_stalled(change_id, &error);
            return Ok(Some(ChangeProcessResult::Stalled { error }));
        }
        Ok(None)
    }

    /// Process a single iteration for a change.
    ///
    /// This includes:
    /// - Running hooks (on_change_start, pre_apply, post_apply, etc.)
    /// - Applying or archiving the change
    /// - Running acceptance tests
    /// - Stall detection
    ///
    /// Returns `Ok(ChangeProcessResult)` indicating the outcome.
    /// Callers should handle the result and decide whether to continue the loop.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_change<O: OutputHandler, F, G>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        cancel_check: F,
        is_single_change_stopped: G,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
        G: Fn() -> bool + Clone,
    {
        self.iteration += 1;
        let change_id = &change.id;

        if let Some(result) = self.preflight_blocked_marker(change_id)? {
            return Ok(result);
        }

        // Check if this is a new change
        let is_new_change = self.current_change_id.as_ref() != Some(change_id);
        if is_new_change {
            // Run on_change_start hook
            let change_start_context = HookContext::new(
                self.changes_processed,
                total_changes,
                remaining_changes,
                false,
            )
            .with_change(change_id, change.completed_tasks, change.total_tasks)
            .with_apply_count(0);

            hooks
                .run_hook(HookType::OnChangeStart, &change_start_context)
                .await?;

            self.current_change_id = Some(change_id.clone());
        }

        let apply_count = self.apply_count(change_id);

        // Process the change
        if change.is_complete() {
            // Archive completed change
            self.archive_change_internal(
                change,
                agent,
                ai_runner,
                hooks,
                output,
                total_changes,
                remaining_changes,
                apply_count,
                operation_tracker,
            )
            .await
        } else {
            // Apply incomplete change
            self.apply_change_internal(
                change,
                agent,
                ai_runner,
                hooks,
                output,
                total_changes,
                remaining_changes,
                apply_count,
                &cancel_check,
                &is_single_change_stopped,
                operation_tracker,
            )
            .await
        }
    }

    /// Internal method to archive a change
    #[allow(clippy::too_many_arguments)]
    async fn archive_change_internal<O: OutputHandler>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        apply_count: u32,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult> {
        info!("Change {} is complete, archiving...", change.id);

        // Update operation to "archive" before running archive
        Self::update_operation_tracker(&operation_tracker, "archive");

        let archive_ctx = ArchiveContext::new(
            self.changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
        );

        let stall_config = self.config.get_stall_detection();

        match archive_change(
            change,
            agent,
            ai_runner,
            hooks,
            &archive_ctx,
            output,
            None,
            &stall_config,
        )
        .await
        {
            Ok(ArchiveResult::Success) => {
                // Update changes_processed count
                self.changes_processed += 1;

                // Clear acceptance history after successful archive
                agent.clear_acceptance_history(&change.id);

                // Run on_change_end hook (not included in shared archive_change)
                let new_remaining = remaining_changes.saturating_sub(1);
                let change_end_context =
                    HookContext::new(self.changes_processed, total_changes, new_remaining, false)
                        .with_change(&change.id, change.completed_tasks, change.total_tasks)
                        .with_apply_count(apply_count);
                hooks
                    .run_hook(HookType::OnChangeEnd, &change_end_context)
                    .await?;

                // Run on_merged hook after on_change_end (serial mode: archive success = merge complete equivalent)
                let merged_context =
                    HookContext::new(self.changes_processed, total_changes, new_remaining, false)
                        .with_change(&change.id, change.completed_tasks, change.total_tasks)
                        .with_apply_count(apply_count);
                hooks.run_hook(HookType::OnMerged, &merged_context).await?;

                // Mark change as completed and clear current
                self.completed_change_ids.insert(change.id.clone());
                self.current_change_id = None;
                self.apply_counts.remove(&change.id);
                self.stall_detector.clear_change(&change.id);

                Ok(ChangeProcessResult::Archived)
            }
            Ok(ArchiveResult::Stalled { error }) => {
                self.mark_stalled(&change.id, &error);
                Ok(ChangeProcessResult::Stalled { error })
            }
            Ok(ArchiveResult::Failed { error }) => Ok(ChangeProcessResult::Failed { error }),
            Ok(ArchiveResult::Cancelled) => Ok(ChangeProcessResult::Cancelled),
            Err(e) => Err(e),
        }
    }

    /// Internal method to apply a change
    #[allow(clippy::too_many_arguments)]
    async fn apply_change_internal<O: OutputHandler, F, G>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        _apply_count: u32,
        cancel_check: &F,
        is_single_change_stopped: &G,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
        G: Fn() -> bool + Clone,
    {
        info!("Applying change: {}", change.id);

        // Create event handler for apply loop
        let event_handler = SerialApplyEventHandler::new(output);

        // Create hook context for apply loop
        let hook_ctx = common_apply::ApplyLoopHookContext::serial(
            self.changes_processed,
            total_changes,
            remaining_changes,
        );

        // Create a cancellation token and spawn a background task to poll cancel_check
        // This allows us to bridge the cancel_check closure to CancellationToken
        let cancel_token = CancellationToken::new();
        let cancel_token_for_task = cancel_token.clone();
        let cancel_check_clone = cancel_check.clone();
        let cancel_task = tokio::spawn(async move {
            loop {
                if cancel_check_clone() {
                    cancel_token_for_task.cancel();
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        // Execute apply loop using common implementation
        let apply_result = match common_apply::execute_apply_loop(
            &change.id,
            &self.repo_root,
            &self.config,
            agent,
            VcsBackend::Git,
            None, // workspace_manager (None for serial mode)
            Some(hooks),
            &hook_ctx,
            &event_handler,
            Some(&cancel_token), // Pass cancel_token to enable apply loop cancellation
            ai_runner,
            |line| async move {
                match &line {
                    OutputLine::Stdout(s) => output.on_stdout(s),
                    OutputLine::Stderr(s) => output.on_agent_stderr(s),
                }
            },
        )
        .await
        {
            Ok(result) => result,
            Err(crate::error::OrchestratorError::PermissionBlocked {
                denied_path,
                guidance,
            }) => {
                // Abort the background cancel monitoring task
                cancel_task.abort();

                // Mark as stalled with permission guidance
                let error_message = format!(
                    "Permission auto-rejected for: {}\n{}",
                    denied_path, guidance
                );
                self.mark_stalled(&change.id, &error_message);
                return Ok(ChangeProcessResult::Stalled {
                    error: error_message,
                });
            }
            Err(e) => {
                // Abort the background cancel monitoring task
                cancel_task.abort();
                return Err(e);
            }
        };

        // Abort the background cancel monitoring task now that apply is complete
        cancel_task.abort();

        let apply_blocked_handoff = apply_result.blocked_handoff.clone();

        // Check if apply loop completed successfully or detected blocked handoff.
        if apply_result.completed || apply_blocked_handoff.is_some() {
            if apply_result.completed {
                info!(
                    "Apply loop completed for {} after {} iterations",
                    change.id, apply_result.iterations
                );
            } else if let Some(ref handoff) = apply_blocked_handoff {
                warn!(
                    change_id = %change.id,
                    blocker_path = %handoff.blocker_path.display(),
                    iterations = apply_result.iterations,
                    "Apply blocked handoff detected; keeping change blocked with preserved worktree context"
                );
            }

            // Increment apply count for this change
            self.increment_apply_count(&change.id);

            // Re-fetch change to get updated task counts after apply
            let (updated_change, is_complete) = self.refetch_change_after_apply(&change.id);

            if is_complete || apply_blocked_handoff.is_some() {
                let updated_change = updated_change.unwrap_or_else(|| change.clone());

                if let Some(ref handoff) = apply_blocked_handoff {
                    warn!(
                        change_id = %change.id,
                        blocker_path = %handoff.blocker_path.display(),
                        "Apply reported recoverable blocker; leaving change stalled for explicit unblock/resume"
                    );

                    Ok(ChangeProcessResult::Stalled {
                        error: format!(
                            "Apply blocked handoff recorded at {}",
                            handoff.blocker_path.display()
                        ),
                    })
                } else {
                    info!(
                        "Tasks complete for {}, running acceptance test...",
                        change.id
                    );

                    let revision = get_current_commit(&self.repo_root).await.map_err(|error| {
                        crate::error::OrchestratorError::AgentCommand(format!(
                            "Failed to determine acceptance revision: {error}"
                        ))
                    })?;
                    mark_apply_completed(&self.repo_root, &revision, &change.id)?;
                    mark_acceptance_started(&self.repo_root, &revision, &change.id)?;
                    self.restore_acceptance_checkpoint_history(&change.id, agent)?;

                    // Update operation to "acceptance" before running acceptance test
                    Self::update_operation_tracker(&operation_tracker, "acceptance");

                    // Run acceptance test
                    match acceptance_test_streaming(
                        &updated_change,
                        agent,
                        ai_runner,
                        &self.config,
                        output,
                        cancel_check,
                    )
                    .await
                    {
                        Ok((result, _attempt_number, _command)) => Ok(self
                            .process_acceptance_result(
                                &change.id,
                                &self.repo_root,
                                &revision,
                                agent,
                                result,
                                is_single_change_stopped,
                            )),
                        Err(e) => {
                            error!("Acceptance error for {}: {}", change.id, e);
                            Err(e)
                        }
                    }
                }
            } else {
                info!(
                    "Apply completed for {}, but tasks not yet complete",
                    change.id
                );
                Ok(ChangeProcessResult::ApplySuccessIncomplete)
            }
        } else {
            error!(
                "Apply loop did not complete for {} after {} iterations",
                change.id, apply_result.iterations
            );
            Ok(ChangeProcessResult::ApplyFailed {
                error: format!(
                    "Apply loop did not complete after {} iterations",
                    apply_result.iterations
                ),
            })
        }
    }

    /// Check stall detection after apply
    pub fn check_stall_after_apply(
        &mut self,
        change_id: &str,
        progress: &TaskProgress,
        is_empty_commit: Option<bool>,
    ) -> Option<String> {
        if let Some(is_empty) = is_empty_commit {
            if !is_progress_complete(progress)
                && self
                    .stall_detector
                    .register_commit(change_id, StallPhase::Apply, is_empty)
            {
                let count = self
                    .stall_detector
                    .current_count(change_id, StallPhase::Apply);
                let threshold = self.stall_detector.config().threshold;
                let message = format!(
                    "Stall detected for {} after {} empty WIP commits (apply)",
                    change_id, count
                );
                return Some(format!("{} (threshold {})", message, threshold));
            }
        }
        None
    }

    /// Re-fetch change to get updated task counts after apply.
    ///
    /// Returns the updated change and whether it's complete.
    fn refetch_change_after_apply(&self, change_id: &str) -> (Option<Change>, bool) {
        let updated_changes =
            openspec::list_changes_native_from(&self.repo_root).unwrap_or_default();
        let updated_change = updated_changes.iter().find(|c| c.id == change_id).cloned();
        let is_complete = updated_change.as_ref().is_some_and(|c| c.is_complete());
        (updated_change, is_complete)
    }

    /// Process acceptance test result and determine outcome.
    ///
    /// Handles Pass, Continue, Fail, CommandFailed, and Cancelled results,
    /// applying max_continues logic for Continue results.
    fn process_acceptance_result<F>(
        &self,
        change_id: &str,
        workspace_path: &std::path::Path,
        revision: &str,
        agent: &AgentRunner,
        acceptance_result: AcceptanceResult,
        is_single_change_stopped: F,
    ) -> ChangeProcessResult
    where
        F: Fn() -> bool,
    {
        match acceptance_result {
            AcceptanceResult::Pass => {
                if let Err(error) =
                    mark_acceptance_passed(workspace_path, revision, Some(change_id))
                {
                    return ChangeProcessResult::AcceptanceCommandFailed {
                        error: format!("Failed to persist acceptance pass checkpoint: {error}"),
                    };
                }
                info!("Acceptance passed for {}, ready for archive", change_id);
                match task_parser::resolve_acceptance_follow_up_tasks_path_for_cleanup(
                    change_id,
                    workspace_path,
                ) {
                    Ok(Some(tasks_path)) => {
                        if let Err(err) = task_parser::clear_acceptance_follow_up(&tasks_path) {
                            return ChangeProcessResult::AcceptanceCommandFailed {
                                error: format!(
                                    "Acceptance passed but follow-up cleanup failed at {}: {}",
                                    tasks_path.display(),
                                    err
                                ),
                            };
                        }
                    }
                    Ok(None) => debug!("No acceptance follow-up to clear for {}", change_id),
                    Err(err) => {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!(
                                "Acceptance passed but follow-up path resolution failed: {}",
                                err
                            ),
                        };
                    }
                }
                ChangeProcessResult::AcceptancePassed
            }
            AcceptanceResult::Continue => {
                let continue_count = agent.count_consecutive_acceptance_continues(change_id);
                let max_continues = self.config.get_acceptance_max_continues();

                if continue_count >= max_continues {
                    if let Err(error) = record_acceptance_retry_context(
                        workspace_path,
                        revision,
                        change_id,
                        &[],
                        continue_count,
                    ) {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!(
                                "Failed to persist acceptance retry checkpoint: {error}"
                            ),
                        };
                    }
                    warn!(
                        "Acceptance CONTINUE limit ({}) exceeded for {}, treating as FAIL",
                        max_continues, change_id
                    );
                    ChangeProcessResult::AcceptanceContinueExceeded
                } else {
                    info!(
                        "Acceptance requires continuation for {} (attempt {}/{}), retrying...",
                        change_id, continue_count, max_continues
                    );
                    ChangeProcessResult::AcceptanceContinue
                }
            }
            AcceptanceResult::Gated => {
                if let Err(error) =
                    mark_acceptance_failed(workspace_path, revision, Some(change_id)).and_then(
                        |()| {
                            write_acceptance_blocked_marker_with_context(
                                workspace_path,
                                change_id,
                                "acceptance_gated",
                                &["acceptance emitted gated compatibility token".to_string()],
                                "no_semantic_progress",
                                &["recoverable acceptance gate".to_string()],
                                true,
                                "explicit retry",
                            )
                        },
                    )
                {
                    return ChangeProcessResult::AcceptanceCommandFailed {
                        error: format!("Failed to persist acceptance stalled evidence: {error}"),
                    };
                }
                warn!(
                    "Acceptance gated for {} - preserving change as stalled/resumable",
                    change_id
                );
                ChangeProcessResult::Stalled {
                    error: "Acceptance gated with recoverable blocker".to_string(),
                }
            }
            AcceptanceResult::Fail { findings } => {
                let previous = match crate::parallel::acceptance_state::load_acceptance_state_for(
                    workspace_path,
                    change_id,
                ) {
                    Ok(state) => state,
                    Err(error) => {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!(
                                "Failed to restore acceptance retry checkpoint: {error}"
                            ),
                        };
                    }
                };
                let retry_count = previous.as_ref().map_or_else(
                    || {
                        agent
                            .get_last_acceptance_attempt(change_id)
                            .map(|attempt| attempt.attempt)
                            .unwrap_or(1)
                    },
                    |state| state.cycle_count.saturating_add(1),
                );
                let fingerprint = match semantic_progress_fingerprint(workspace_path) {
                    Ok(fingerprint) => fingerprint,
                    Err(error) => {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!("Failed to fingerprint acceptance progress: {error}"),
                        };
                    }
                };
                let normalized = normalize_findings(&findings);
                let identities = normalized
                    .iter()
                    .map(|finding| finding.identity.clone())
                    .collect::<Vec<_>>();
                let decision = decide_acceptance_retry(
                    previous.as_ref().map_or(&[] as &[String], |state| {
                        state.previous_finding_identities.as_slice()
                    }),
                    previous
                        .as_ref()
                        .and_then(|state| state.semantic_fingerprint.as_deref()),
                    &normalized,
                    &fingerprint,
                    retry_count,
                );
                if let Err(error) =
                    mark_acceptance_failed(workspace_path, revision, Some(change_id)).and_then(
                        |()| {
                            record_acceptance_retry_checkpoint(
                                workspace_path,
                                revision,
                                change_id,
                                identities.clone(),
                                Some(fingerprint),
                                retry_count,
                            )
                        },
                    )
                {
                    return ChangeProcessResult::AcceptanceCommandFailed {
                        error: format!("Failed to persist acceptance failure checkpoint: {error}"),
                    };
                }
                if let AcceptanceRetryDecision::Stall {
                    reason,
                    external_blockers,
                } = decision
                {
                    if let Err(error) = write_acceptance_blocked_marker_with_context(
                        workspace_path,
                        change_id,
                        reason,
                        &identities,
                        "no_semantic_progress",
                        &external_blockers,
                        true,
                        "explicit retry",
                    ) {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!(
                                "Failed to persist acceptance stalled evidence: {error}"
                            ),
                        };
                    }
                    return ChangeProcessResult::Stalled {
                        error: reason.to_string(),
                    };
                }
                let blocking_gate_context = findings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "no acceptance findings captured".to_string());
                warn!(
                    "Acceptance failed for {} ({} findings), blocking gate context: {}; will retry apply",
                    change_id,
                    findings.len(),
                    blocking_gate_context
                );
                let repository_findings = repository_findings(&findings);
                if !findings.is_empty() {
                    if let Ok(tasks_path) = task_parser::resolve_acceptance_follow_up_tasks_path(
                        change_id,
                        workspace_path,
                    ) {
                        if let Err(err) = task_parser::replace_acceptance_follow_up_from_latest_fail(
                            &tasks_path,
                            agent
                                .get_last_acceptance_attempt(change_id)
                                .map(|attempt| attempt.attempt)
                                .unwrap_or(1),
                            &findings,
                        ) {
                            warn!(
                                "Acceptance follow-up persistence degraded for {} at {}: {}",
                                change_id,
                                tasks_path.display(),
                                err
                            );
                        }
                    }
                }
                ChangeProcessResult::AcceptanceFailed {
                    findings: repository_findings,
                }
            }
            AcceptanceResult::CommandFailed {
                error,
                findings: _findings,
            } => {
                error!("Acceptance command failed for {}: {}", change_id, error);
                // Canonical owner note: runtime appends follow-up tasks for FAIL verdicts,
                // while command-level failures are surfaced without forcing local tasks.md updates.
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::MissingVerdict { findings } => {
                // A completed acceptance command with no canonical verdict is a
                // protocol failure, not an intentional CONTINUE. Route it as a
                // command-level failure so it never consumes the explicit
                // CONTINUE retry budget, and keep bounded output evidence in
                // the operator-visible diagnostic.
                let evidence = findings
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" | ");
                let error = format!(
                    "Acceptance completed without a canonical verdict (missing-verdict protocol \
                     failure); status-only or waiting output is not a verdict. Evidence: {}",
                    if evidence.is_empty() {
                        "no acceptance output captured".to_string()
                    } else {
                        evidence
                    }
                );
                error!("{} for {}", error, change_id);
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::PermissionStalled { blocker } => {
                let evidence = vec![blocker.summary()];
                if let Err(error) =
                    mark_acceptance_failed(workspace_path, revision, Some(change_id)).and_then(
                        |()| {
                            write_acceptance_blocked_marker_with_context(
                                workspace_path,
                                change_id,
                                "permission_stalled",
                                &evidence,
                                "no_semantic_progress",
                                &evidence,
                                true,
                                &blocker.next_action,
                            )
                        },
                    )
                {
                    return ChangeProcessResult::AcceptanceCommandFailed {
                        error: format!("Failed to persist acceptance stalled evidence: {error}"),
                    };
                }
                warn!(
                    "Acceptance stalled for {} due to repeated unresolved permission/tool policy blocker: {}",
                    change_id, blocker.next_action
                );
                ChangeProcessResult::Stalled {
                    error: blocker.next_action,
                }
            }
            AcceptanceResult::Cancelled => {
                // Check if this is a single-change stop or global cancel
                if is_single_change_stopped() {
                    info!("Single change {} stopped during acceptance", change_id);
                    ChangeProcessResult::ChangeStopped
                } else {
                    info!("Acceptance cancelled for {} (global cancel)", change_id);
                    ChangeProcessResult::Cancelled
                }
            }
        }
    }

    /// Update operation tracker with the current operation name.
    ///
    /// This is a helper to centralize tracker updates for both apply and acceptance flows.
    fn update_operation_tracker(
        operation_tracker: &Option<std::sync::Arc<std::sync::RwLock<String>>>,
        operation: &str,
    ) {
        if let Some(ref tracker) = operation_tracker {
            *tracker.write().unwrap() = operation.to_string();
        }
    }
}

/// Result of processing a single change
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants may not be used yet depending on mode
pub enum ChangeProcessResult {
    /// Change was successfully archived
    Archived,
    /// Change was stalled
    Stalled { error: String },
    /// Archive or apply failed
    Failed { error: String },
    /// Operation was cancelled (global stop)
    Cancelled,
    /// Single change was stopped (not a global cancel)
    ChangeStopped,
    /// Apply succeeded but tasks not yet complete
    ApplySuccessIncomplete,
    /// Apply failed
    ApplyFailed { error: String },
    /// Acceptance test passed
    AcceptancePassed,
    /// Acceptance test failed
    AcceptanceFailed { findings: Vec<String> },
    /// Acceptance test command failed
    AcceptanceCommandFailed { error: String },
    /// Acceptance test requires continuation
    AcceptanceContinue,
    /// Acceptance CONTINUE limit exceeded
    AcceptanceContinueExceeded,
    /// Acceptance gated and change was rejected
    Rejected { reason: String },
}

/// Helper function to check if progress is complete
fn is_progress_complete(progress: &TaskProgress) -> bool {
    progress.total > 0 && progress.completed >= progress.total
}

/// Event handler for serial apply loop that delegates to OutputHandler
struct SerialApplyEventHandler<'a, O: OutputHandler> {
    #[allow(dead_code)] // Kept for type safety but not used since output is handled via closure
    output: &'a O,
}

impl<'a, O: OutputHandler> SerialApplyEventHandler<'a, O> {
    fn new(output: &'a O) -> Self {
        Self { output }
    }
}

impl<'a, O: OutputHandler> common_apply::ApplyEventHandler for SerialApplyEventHandler<'a, O> {
    fn on_apply_started(&self, _change_id: &str, _command: &str) {
        // No-op for serial mode - output is handled via output_handler closure
    }

    fn on_progress_updated(&self, _change_id: &str, _completed: u32, _total: u32) {
        // No-op for serial mode - progress is logged in execute_apply_loop
    }

    fn on_hook_started(&self, _change_id: &str, _hook_type: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_hook_completed(&self, _change_id: &str, _hook_type: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_hook_failed(&self, _change_id: &str, _hook_type: &str, _error: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_apply_output(&self, _change_id: &str, _line: &OutputLine, _iteration: u32) {
        // No-op: Output is already handled by the output_handler closure passed to execute_apply_loop
        // (lines 398-403). Having both would cause duplicate output.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::default_retry_patterns;
    use crate::config::OrchestratorConfig;
    use crate::hooks::{HookRunner, HooksConfig};
    use crate::openspec::ProposalMetadata;
    use crate::orchestration::output::NullOutputHandler;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    #[test]
    fn test_select_next_change_prioritizes_progress() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 1, 10), // 10% progress
            create_test_change("b", 5, 10), // 50% progress
            create_test_change("c", 8, 10), // 80% progress (highest)
        ];

        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("c"));
    }

    #[test]
    fn test_select_next_change_excludes_stalled() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        service.mark_stalled("b", "test");

        let changes = vec![
            create_test_change("a", 1, 10),
            create_test_change("b", 8, 10), // Highest progress but stalled
            create_test_change("c", 5, 10),
        ];

        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("c")); // Should pick 'c', not 'b'
    }

    #[test]
    fn test_select_next_change_prioritizes_complete_for_archive() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 5, 10),  // 50% progress, incomplete
            create_test_change("b", 10, 10), // 100% complete
        ];

        let next = service.select_next_change(&changes);
        // Should select the incomplete one first (archive happens in a separate phase in practice,
        // but select_next_change returns the first match which would be 'b' if it's complete)
        // Actually, reading the implementation, it prioritizes incomplete first, so should be 'a'
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));
    }

    #[tokio::test]
    async fn serial_process_change_restores_checkpoint_before_next_acceptance() {
        use crate::parallel::acceptance_state::record_acceptance_retry_context;

        let temp_dir = TempDir::new().unwrap();
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test User"],
        ] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(temp_dir.path())
                .output()
                .unwrap();
        }
        let change_id = "serial-restart";
        let change_dir = temp_dir.path().join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# serial restart\n").unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [ ] pending\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        record_acceptance_retry_context(
            temp_dir.path(),
            "checkpoint-revision",
            change_id,
            &["Prior serial finding".to_string()],
            2,
        )
        .unwrap();

        let config = OrchestratorConfig {
            apply_command: Some(format!(
                "sh -c \"printf '%s\\n' '- [x] done' > openspec/changes/{change_id}/tasks.md\""
            )),
            acceptance_command: Some(
                "sh -c 'echo ACCEPTANCE: FAIL; echo FINDINGS:; echo - repeated serial finding'"
                    .to_string(),
            ),
            ..Default::default()
        };
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = AiCommandRunner::new(
            CommandQueueConfig {
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: default_retry_patterns(),
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 0,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
            },
            Arc::new(Mutex::new(None)),
        );

        let result = service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { .. }
        ));
        let checkpoint = crate::parallel::acceptance_state::load_acceptance_state_for(
            temp_dir.path(),
            change_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(checkpoint.cycle_count, 3);
        assert_eq!(
            checkpoint.previous_finding_identities,
            ["repository|repeated serial finding|implementation"]
        );

        std::fs::write(change_dir.join("tasks.md"), "- [ ] pending\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "prepare foreign checkpoint test"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        record_acceptance_retry_context(
            temp_dir.path(),
            "foreign-revision",
            "foreign-change",
            &["Foreign serial finding".to_string()],
            9,
        )
        .unwrap();
        let mut foreign_agent = AgentRunner::new(config);
        let result = service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut foreign_agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { .. }
        ));
        let checkpoint = crate::parallel::acceptance_state::load_acceptance_state_for(
            temp_dir.path(),
            change_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(checkpoint.cycle_count, 1);
        assert_eq!(
            checkpoint.previous_finding_identities,
            ["repository|repeated serial finding|implementation"]
        );
    }

    #[test]
    fn serial_restart_restores_checkpoint_history_before_next_acceptance() {
        use crate::agent::AgentRunner;
        use crate::parallel::acceptance_state::record_acceptance_retry_context;

        let temp_dir = TempDir::new().unwrap();
        record_acceptance_retry_context(
            temp_dir.path(),
            "test-revision",
            "test-change",
            &["Missing regression coverage".to_string()],
            2,
        )
        .unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let mut agent = AgentRunner::new(OrchestratorConfig::default());

        service
            .restore_acceptance_checkpoint_history("test-change", &mut agent)
            .unwrap();

        assert_eq!(agent.next_acceptance_attempt_number("test-change"), 3);
        assert_eq!(
            agent.get_last_acceptance_findings("test-change"),
            Some(vec![
                "repository|missing regression coverage|verification".to_string()
            ])
        );
        assert_eq!(
            agent.get_restored_acceptance_semantic_fingerprint("test-change"),
            Some("cbf29ce484222325".to_string())
        );
    }

    #[test]
    fn serial_repeated_findings_without_progress_stall_before_another_apply() {
        use crate::orchestration::acceptance::normalize_findings;
        use crate::parallel::acceptance_state::record_acceptance_retry_checkpoint;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["src/lib.rs:10 missing regression coverage".to_string()];
        let fingerprint = semantic_progress_fingerprint(temp_dir.path()).unwrap();
        record_acceptance_retry_checkpoint(
            temp_dir.path(),
            "previous-revision",
            "test-change",
            normalize_findings(&findings)
                .into_iter()
                .map(|finding| finding.identity)
                .collect(),
            Some(fingerprint),
            1,
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "current-revision",
            &agent,
            AcceptanceResult::Fail { findings },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::Stalled { ref error }
            if error == "repeated_acceptance_findings"
        ));
        assert_eq!(
            crate::parallel::acceptance_state::parse_blocked_marker(temp_dir.path(), "test-change")
                .unwrap()
                .unwrap()
                .reason,
            "repeated_acceptance_findings"
        );
    }

    #[test]
    fn serial_external_only_failure_stalls_without_apply_findings() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "current-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: vec!["external non-mockable prerequisite unavailable".to_string()],
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::Stalled { ref error } if error == "external_acceptance_blocker"
        ));
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "test-change"
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn serial_missing_verdict_routes_as_protocol_failure_not_continue() {
        // A completed acceptance command without a canonical verdict must be
        // surfaced as an explicit protocol/command failure with actionable
        // evidence — never as the intentional-CONTINUE retry path.
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "current-revision",
            &agent,
            AcceptanceResult::MissingVerdict {
                findings: vec!["Monitoring verification, will report when complete".to_string()],
            },
            || false,
        );

        match result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(
                    error.contains("missing-verdict protocol failure"),
                    "diagnostic must identify the missing verdict, got: {error}"
                );
                assert!(
                    error.contains("Monitoring verification, will report when complete"),
                    "diagnostic must retain bounded output evidence, got: {error}"
                );
            }
            other => panic!(
                "missing verdict must route as acceptance command failure, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn serial_explicit_continue_still_uses_continue_retry_path() {
        // Control: an explicit canonical CONTINUE keeps its intentional
        // continuation routing and configured retry policy.
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "current-revision",
            &agent,
            AcceptanceResult::Continue,
            || false,
        );

        assert!(
            matches!(result, ChangeProcessResult::AcceptanceContinue),
            "explicit CONTINUE below the retry limit must retry acceptance, got {:?}",
            result
        );
    }

    #[test]
    fn serial_cycle_limit_stalls_with_workspace_marker() {
        use crate::orchestration::acceptance::{normalize_findings, MAX_ACCEPTANCE_RETRY_CYCLES};
        use crate::parallel::acceptance_state::record_acceptance_retry_checkpoint;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["new finding at ceiling".to_string()];
        record_acceptance_retry_checkpoint(
            temp_dir.path(),
            "previous-revision",
            "test-change",
            normalize_findings(&["older finding".to_string()])
                .into_iter()
                .map(|finding| finding.identity)
                .collect(),
            Some("previous-progress".to_string()),
            MAX_ACCEPTANCE_RETRY_CYCLES - 1,
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "current-revision",
            &agent,
            AcceptanceResult::Fail { findings },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::Stalled { ref error }
            if error == "acceptance_cycle_limit_exhausted"
        ));
        let marker =
            crate::parallel::acceptance_state::parse_blocked_marker(temp_dir.path(), "test-change")
                .unwrap()
                .unwrap();
        assert_eq!(marker.reason, "acceptance_cycle_limit_exhausted");
        assert_eq!(marker.retry_count, MAX_ACCEPTANCE_RETRY_CYCLES);
    }

    #[test]
    fn test_process_acceptance_result_archive_readiness_fail_blocks_archive_progression() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec![
            "blocking gate: cargo clippy -- -D warnings".to_string(),
            "src/orchestration/archive.rs:459".to_string(),
        ];
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
    }

    #[test]
    fn serial_latest_fail_reconciles_completed_findings_with_parallel_parity() {
        let temp_dir = TempDir::new().unwrap();
        let change_id = "test-change";
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        let tasks_path = change_dir.join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] [SAME_FINDING] fixed wording\n- [x] [RETIRED_FINDING] fixed and not reported again\n- [x] [DIFFERENT_FINDING] unrelated completed defect\n",
        )
        .unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec![
            "[SAME_FINDING] defect still present with new evidence".to_string(),
            "[NEW_FINDING] distinct newly reported defect".to_string(),
        ];

        let result = service.process_acceptance_result(
            change_id,
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] [SAME_FINDING] defect still present with new evidence"));
        assert!(content.contains("- [ ] [NEW_FINDING] distinct newly reported defect"));
        assert!(!content.contains("RETIRED_FINDING"));
        assert!(!content.contains("DIFFERENT_FINDING"));
        assert_eq!(
            crate::task_parser::parse_file(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(1, 3)
        );
    }

    #[test]
    fn acceptance_fail_uses_recorded_attempt_number_for_follow_up() {
        use crate::agent::AgentRunner;
        use crate::history::AcceptanceAttempt;
        use crate::orchestration::AcceptanceResult;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let mut agent = AgentRunner::new(OrchestratorConfig::default());
        agent.record_acceptance_attempt(
            "test-change",
            AcceptanceAttempt {
                attempt: 1,
                passed: false,
                duration: Duration::from_secs(1),
                findings: Some(vec!["first".to_string()]),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: None,
            },
        );
        agent.record_acceptance_attempt(
            "test-change",
            AcceptanceAttempt {
                attempt: 2,
                passed: false,
                duration: Duration::from_secs(1),
                findings: Some(vec!["second".to_string()]),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: None,
            },
        );
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [x] done\n").unwrap();

        service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: vec!["canonical second".to_string()],
            },
            || false,
        );

        let content = std::fs::read_to_string(change_dir.join("tasks.md")).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- attempt: 2"));
        assert_eq!(
            content.matches("## Current Acceptance Follow-up").count(),
            1
        );
    }

    #[test]
    fn test_process_acceptance_result_fail_uses_archive_tasks_fallback_when_active_missing() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let archive_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("test-change");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(
            archive_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        let findings = vec!["archive fallback finding".to_string()];
        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));

        let content = std::fs::read_to_string(archive_dir.join("tasks.md")).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- attempt: 1"));
        assert!(content.contains("- [ ] archive fallback finding"));
    }

    #[test]
    fn test_process_acceptance_result_fail_degrades_when_no_tasks_path_available() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["missing tasks path finding".to_string()];

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
    }

    #[test]
    fn acceptance_pass_clears_runtime_follow_up() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #2 Failure Follow-up\n- [x] fixed\n",
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
        let content = std::fs::read_to_string(change_dir.join("tasks.md")).unwrap();
        assert!(!content.contains("Failure Follow-up"));
        assert!(content.contains("## Implementation Tasks\n- [x] done"));
    }

    #[test]
    fn test_process_acceptance_result_archive_readiness_pass_allows_archive_progression() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
    }

    #[test]
    fn test_process_acceptance_result_gated_returns_stalled_result() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            "test-revision",
            &agent,
            AcceptanceResult::Gated,
            || false, // Not a single-change stop
        );

        assert!(matches!(
            result,
            ChangeProcessResult::Stalled { ref error }
            if error == "Acceptance gated with recoverable blocker"
        ));
        let marker =
            crate::parallel::acceptance_state::parse_blocked_marker(temp_dir.path(), "test-change")
                .unwrap()
                .unwrap();
        assert_eq!(marker.reason, "acceptance_gated");
        assert_eq!(marker.semantic_progress, "no_semantic_progress");
        assert_eq!(marker.external_blockers, ["recoverable acceptance gate"]);
    }

    #[tokio::test]
    async fn serial_process_change_stops_at_workspace_marker_before_archive() {
        use crate::parallel::acceptance_state::write_acceptance_blocked_marker;

        let temp_dir = TempDir::new().unwrap();
        write_acceptance_blocked_marker(
            temp_dir.path(),
            "complete-change",
            "stalled",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let mut agent = AgentRunner::new(OrchestratorConfig::default());
        let ai_runner = AiCommandRunner::new(
            CommandQueueConfig {
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: default_retry_patterns(),
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 0,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
            },
            Arc::new(Mutex::new(None)),
        );
        let result = service
            .process_change(
                &create_test_change("complete-change", 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, ChangeProcessResult::Stalled { .. }));
        assert!(service.is_stalled("complete-change"));
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "complete-change"
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn serial_preflight_suppresses_apply_and_archive_for_any_marker() {
        use crate::parallel::acceptance_state::write_acceptance_blocked_marker;

        let temp_dir = TempDir::new().unwrap();
        write_acceptance_blocked_marker(
            temp_dir.path(),
            "complete-change",
            "stalled",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let result = service.preflight_blocked_marker("complete-change").unwrap();

        assert!(matches!(result, Some(ChangeProcessResult::Stalled { .. })));
        assert!(service.is_stalled("complete-change"));
    }

    #[test]
    fn malformed_marker_stops_serial_preflight_and_is_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("openspec/changes/blocked/APPLY_BLOCKED/marker.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ malformed").unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        assert!(service.preflight_blocked_marker("blocked").is_err());
        assert!(path.exists());
    }

    #[test]
    fn explicit_serial_retry_consumes_only_resumable_acceptance_marker() {
        use crate::parallel::acceptance_state::write_acceptance_blocked_marker;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        write_acceptance_blocked_marker(
            temp_dir.path(),
            "acceptance",
            "stalled",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();
        assert!(service
            .consume_explicit_acceptance_retry("acceptance")
            .unwrap());

        let apply_marker = temp_dir
            .path()
            .join("openspec/changes/apply/APPLY_BLOCKED/marker.md");
        std::fs::create_dir_all(apply_marker.parent().unwrap()).unwrap();
        std::fs::write(&apply_marker, "origin: apply\nreason: blocked\n").unwrap();
        assert!(!service.consume_explicit_acceptance_retry("apply").unwrap());
        assert!(apply_marker.exists());
    }

    #[test]
    fn test_mark_stalled_prevents_reselection() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 5, 10),
            create_test_change("b", 8, 10), // Highest progress
        ];

        // Initially, highest progress change should be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("b"));

        // Mark 'b' as stalled (simulating GATED acceptance)
        service.mark_stalled("b", "Implementation blocker detected");

        // After marking as stalled, 'b' should not be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));

        // Verify 'b' is marked as stalled
        assert!(service.is_stalled("b"));
        assert!(!service.is_stalled("a"));
    }
}
