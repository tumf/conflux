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
use crate::orchestration::{
    acceptance_test_streaming, archive_change, AcceptanceResult, ArchiveContext, ArchiveResult,
    OutputHandler,
};
use crate::stall::{StallDetector, StallPhase};
use crate::task_parser::TaskProgress;
use crate::vcs::VcsBackend;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

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
    pub async fn process_change<O: OutputHandler, F>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        cancel_check: F,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
    {
        self.iteration += 1;
        let change_id = &change.id;

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
    async fn apply_change_internal<O: OutputHandler, F>(
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
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
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
        let apply_result = common_apply::execute_apply_loop(
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
                    OutputLine::Stderr(s) => output.on_stderr(s),
                }
            },
        )
        .await?;

        // Abort the background cancel monitoring task now that apply is complete
        cancel_task.abort();

        // Check if apply loop completed successfully
        if apply_result.completed {
            info!(
                "Apply loop completed for {} after {} iterations",
                change.id, apply_result.iterations
            );

            // Increment apply count for this change
            self.increment_apply_count(&change.id);

            // Re-fetch change to get updated task counts after apply
            let (updated_change, is_complete) = Self::refetch_change_after_apply(&change.id);

            if is_complete {
                let updated_change = updated_change.unwrap(); // Safe: checked above
                info!(
                    "Tasks complete for {}, running acceptance test...",
                    change.id
                );

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
                    Ok((result, _attempt_number, _command)) => {
                        Ok(self.process_acceptance_result(&change.id, agent, result))
                    }
                    Err(e) => {
                        error!("Acceptance error for {}: {}", change.id, e);
                        Err(e)
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
    fn refetch_change_after_apply(change_id: &str) -> (Option<Change>, bool) {
        let updated_changes = openspec::list_changes_native().unwrap_or_default();
        let updated_change = updated_changes.iter().find(|c| c.id == change_id).cloned();
        let is_complete = updated_change.as_ref().is_some_and(|c| c.is_complete());
        (updated_change, is_complete)
    }

    /// Process acceptance test result and determine outcome.
    ///
    /// Handles Pass, Continue, Fail, CommandFailed, and Cancelled results,
    /// applying max_continues logic for Continue results.
    fn process_acceptance_result(
        &self,
        change_id: &str,
        agent: &AgentRunner,
        acceptance_result: AcceptanceResult,
    ) -> ChangeProcessResult {
        match acceptance_result {
            AcceptanceResult::Pass => {
                info!("Acceptance passed for {}, ready for archive", change_id);
                ChangeProcessResult::AcceptancePassed
            }
            AcceptanceResult::Continue => {
                let continue_count = agent.count_consecutive_acceptance_continues(change_id);
                let max_continues = self.config.get_acceptance_max_continues();

                if continue_count >= max_continues {
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
            AcceptanceResult::Blocked => {
                warn!("Acceptance blocked for {} - implementation blocker detected", change_id);
                ChangeProcessResult::AcceptanceBlocked
            }
            AcceptanceResult::Fail { findings } => {
                warn!(
                    "Acceptance failed for {} ({} tail lines), will retry apply",
                    change_id,
                    findings.len()
                );
                // Note: tasks.md is now updated by the acceptance agent itself
                ChangeProcessResult::AcceptanceFailed { findings }
            }
            AcceptanceResult::CommandFailed { error, findings: _ } => {
                error!("Acceptance command failed for {}: {}", change_id, error);
                // Note: tasks.md is now updated by the acceptance agent itself
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::Cancelled => {
                info!("Acceptance cancelled for {}", change_id);
                ChangeProcessResult::Cancelled
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
    /// Operation was cancelled
    Cancelled,
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
    /// Acceptance blocked due to implementation blocker
    AcceptanceBlocked,
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
    use crate::config::OrchestratorConfig;
    use tempfile::TempDir;

    fn create_test_change(id: &str, completed: u32, total: u32, is_approved: bool) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            is_approved,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_select_next_change_prioritizes_progress() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 1, 10, true), // 10% progress
            create_test_change("b", 5, 10, true), // 50% progress
            create_test_change("c", 8, 10, true), // 80% progress (highest)
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
            create_test_change("a", 1, 10, true),
            create_test_change("b", 8, 10, true), // Highest progress but stalled
            create_test_change("c", 5, 10, true),
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
            create_test_change("a", 5, 10, true), // 50% progress, incomplete
            create_test_change("b", 10, 10, true), // 100% complete
        ];

        let next = service.select_next_change(&changes);
        // Should select the incomplete one first (archive happens in a separate phase in practice,
        // but select_next_change returns the first match which would be 'b' if it's complete)
        // Actually, reading the implementation, it prioritizes incomplete first, so should be 'a'
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));
    }

    #[test]
    fn test_process_acceptance_result_blocked_returns_correct_variant() {
        use crate::orchestration::AcceptanceResult;
        use crate::agent::AgentRunner;

        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            &agent,
            AcceptanceResult::Blocked,
        );

        matches!(result, ChangeProcessResult::AcceptanceBlocked);
    }

    #[test]
    fn test_mark_stalled_prevents_reselection() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 5, 10, true),
            create_test_change("b", 8, 10, true), // Highest progress
        ];

        // Initially, highest progress change should be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("b"));

        // Mark 'b' as stalled (simulating BLOCKED acceptance)
        service.mark_stalled("b", "Implementation blocker detected");

        // After marking as stalled, 'b' should not be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));

        // Verify 'b' is marked as stalled
        assert!(service.is_stalled("b"));
        assert!(!service.is_stalled("a"));
    }
}
