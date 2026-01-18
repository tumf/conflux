//! Shared apply operations for CLI and TUI modes.
//!
//! Provides a unified apply implementation that both modes can use,
//! eliminating code duplication and ensuring consistent behavior.
//!
//! Note: These functions are infrastructure for future CLI/TUI integration.
//! They will be used as the refactoring continues.

#![allow(dead_code)]

use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::Change;
use crate::process_manager::TerminationOutcome;
use std::time::Duration;
use tracing::info;

use super::output::OutputHandler;

/// Result of an apply operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyResult {
    /// Apply completed successfully.
    Success,
    /// Apply command failed.
    Failed { error: String },
    /// Apply was cancelled (e.g., by user or timeout).
    Cancelled,
}

impl ApplyResult {
    /// Returns true if the apply succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, ApplyResult::Success)
    }
}

/// Context for apply operations.
#[derive(Debug, Clone)]
pub struct ApplyContext {
    /// Number of changes already processed.
    pub changes_processed: usize,
    /// Total number of changes in the run.
    pub total_changes: usize,
    /// Remaining changes to process.
    pub remaining_changes: usize,
    /// How many times this change has been applied (including this attempt).
    pub apply_count: u32,
}

impl ApplyContext {
    /// Create a new ApplyContext.
    pub fn new(
        changes_processed: usize,
        total_changes: usize,
        remaining_changes: usize,
        apply_count: u32,
    ) -> Self {
        Self {
            changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
        }
    }
}

/// Apply a change.
///
/// This function handles:
/// - Running pre_apply hook
/// - Executing the apply command
/// - Running post_apply hook (on success)
/// - Running on_error hook (on failure)
///
/// # Arguments
/// * `change` - The change to apply
/// * `agent` - The agent runner for executing commands
/// * `hooks` - The hook runner for executing hooks
/// * `context` - Context information for hooks
/// * `output` - Output handler for streaming command output
///
/// # Returns
/// * `Ok(ApplyResult::Success)` - Apply completed successfully
/// * `Ok(ApplyResult::Failed { error })` - Apply command failed
/// * `Ok(ApplyResult::Cancelled)` - Apply was cancelled
/// * `Err(e)` - An error occurred (e.g., hook failure with continue_on_failure=false)
pub async fn apply_change<O: OutputHandler>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ApplyContext,
    output: &O,
) -> Result<ApplyResult> {
    info!("Applying change: {}", change.id);

    // Build hook context
    let hook_ctx = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    // Run pre_apply hook
    if let Err(e) = hooks.run_hook(HookType::PreApply, &hook_ctx).await {
        output.on_warn(&format!("pre_apply hook failed: {}", e));
        return Err(e);
    }

    output.on_info(&format!("Applying: {}", change.id));

    // Execute apply command
    let status = agent.run_apply(&change.id).await?;

    if !status.success() {
        let error_msg = format!("Apply command failed with exit code: {:?}", status.code());

        // Run on_error hook
        let error_ctx = hook_ctx.clone().with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_ctx).await;

        output.on_error(&error_msg);
        return Ok(ApplyResult::Failed { error: error_msg });
    }

    // Run post_apply hook
    if let Err(e) = hooks.run_hook(HookType::PostApply, &hook_ctx).await {
        output.on_warn(&format!("post_apply hook failed: {}", e));
        return Err(e);
    }

    info!("Successfully applied: {}", change.id);
    output.on_success(&format!("Applied: {}", change.id));

    Ok(ApplyResult::Success)
}

/// Apply a change with streaming output.
///
/// Similar to `apply_change` but uses streaming output for real-time feedback.
/// This is primarily used by TUI mode.
///
/// # Arguments
/// * `change` - The change to apply
/// * `agent` - The agent runner for executing commands
/// * `hooks` - The hook runner for executing hooks
/// * `context` - Context information for hooks
/// * `output` - Output handler for streaming command output
/// * `cancel_check` - Function to check if operation should be cancelled
///
/// # Returns
/// Same as `apply_change`
pub async fn apply_change_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ApplyContext,
    output: &O,
    cancel_check: F,
) -> Result<ApplyResult>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::agent::OutputLine;

    info!("Applying change (streaming): {}", change.id);

    // Build hook context
    let hook_ctx = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    // Run pre_apply hook
    if let Err(e) = hooks.run_hook(HookType::PreApply, &hook_ctx).await {
        output.on_warn(&format!("pre_apply hook failed: {}", e));
        return Err(e);
    }

    output.on_info(&format!("Applying: {}", change.id));

    // Execute apply command with streaming
    let (mut child, mut output_rx, start_time) =
        agent.run_apply_streaming(&change.id, None).await?;

    // Stream output
    loop {
        if cancel_check() {
            let termination = child.terminate_with_timeout(Duration::from_secs(5)).await;
            let message = match termination {
                Ok(TerminationOutcome::Exited(_)) => {
                    "Process terminated due to cancellation".to_string()
                }
                Ok(TerminationOutcome::ForceKilled(_)) => {
                    "Process force killed after cancellation timeout".to_string()
                }
                Ok(TerminationOutcome::TimedOut) => {
                    "Process still running after force kill timeout".to_string()
                }
                Err(e) => format!("Failed to terminate process after cancellation: {}", e),
            };
            output.on_warn(&message);
            return Ok(ApplyResult::Cancelled);
        }

        match output_rx.try_recv() {
            Ok(OutputLine::Stdout(s)) => output.on_stdout(&s),
            Ok(OutputLine::Stderr(s)) => output.on_stderr(&s),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No data available, check if process is done
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }

    // Wait for child process to complete
    let status = child.wait().await.map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to wait for apply command for change '{}': {}",
            change.id, e
        ))
    })?;

    // Record the apply attempt for history context in subsequent retries
    agent.record_apply_attempt(&change.id, &status, start_time);

    if !status.success() {
        let error_msg = format!(
            "Apply command failed for change '{}' with exit code: {:?}",
            change.id,
            status.code()
        );

        // Run on_error hook
        let error_ctx = hook_ctx.clone().with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_ctx).await;

        output.on_error(&error_msg);
        return Ok(ApplyResult::Failed { error: error_msg });
    }

    // Run post_apply hook
    if let Err(e) = hooks.run_hook(HookType::PostApply, &hook_ctx).await {
        output.on_warn(&format!("post_apply hook failed: {}", e));
        return Err(e);
    }

    info!("Successfully applied: {}", change.id);
    output.on_success(&format!("Applied: {}", change.id));

    Ok(ApplyResult::Success)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_result_is_success() {
        assert!(ApplyResult::Success.is_success());
        assert!(!ApplyResult::Failed {
            error: "test".to_string()
        }
        .is_success());
        assert!(!ApplyResult::Cancelled.is_success());
    }

    #[test]
    fn test_apply_context_new() {
        let ctx = ApplyContext::new(1, 5, 4, 2);
        assert_eq!(ctx.changes_processed, 1);
        assert_eq!(ctx.total_changes, 5);
        assert_eq!(ctx.remaining_changes, 4);
        assert_eq!(ctx.apply_count, 2);
    }
}
