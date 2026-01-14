//! Shared archive operations for CLI and TUI modes.
//!
//! Provides a unified archive implementation that both modes can use,
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
use std::path::Path;
use std::time::Duration;
use tracing::{debug, info};

use super::output::OutputHandler;

/// Result of an archive operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveResult {
    /// Archive completed successfully.
    Success,
    /// Archive command failed.
    Failed { error: String },
    /// Archive was cancelled (e.g., by user or timeout).
    Cancelled,
}

impl ArchiveResult {
    /// Returns true if the archive succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, ArchiveResult::Success)
    }
}

/// Context for archive operations.
#[derive(Debug, Clone)]
pub struct ArchiveContext {
    /// Number of changes already processed.
    pub changes_processed: usize,
    /// Total number of changes in the run.
    pub total_changes: usize,
    /// Remaining changes to process.
    pub remaining_changes: usize,
    /// How many times this change was applied.
    pub apply_count: u32,
}

impl ArchiveContext {
    /// Create a new ArchiveContext.
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

/// Archive a completed change.
///
/// This function handles:
/// - Running pre-archive hooks (on_change_complete, pre_archive)
/// - Executing the archive command
/// - Verifying the archive was successful
/// - Running post-archive hooks (post_archive)
/// - Cleaning up apply history
///
/// # Arguments
/// * `change` - The change to archive
/// * `agent` - The agent runner for executing commands
/// * `hooks` - The hook runner for executing hooks
/// * `context` - Context information for hooks
/// * `output` - Output handler for streaming command output
///
/// # Returns
/// * `Ok(ArchiveResult::Success)` - Archive completed successfully
/// * `Ok(ArchiveResult::Failed { error })` - Archive command failed
/// * `Ok(ArchiveResult::Cancelled)` - Archive was cancelled
/// * `Err(e)` - An error occurred (e.g., hook failure with continue_on_failure=false)
pub async fn archive_change<O: OutputHandler>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ArchiveContext,
    output: &O,
) -> Result<ArchiveResult> {
    info!("Archiving change: {}", change.id);

    // Build hook context
    let hook_ctx = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    // Run on_change_complete hook
    if let Err(e) = hooks.run_hook(HookType::OnChangeComplete, &hook_ctx).await {
        output.on_warn(&format!("on_change_complete hook failed: {}", e));
        // Hook failure propagates if continue_on_failure=false
        return Err(e);
    }

    // Run pre_archive hook
    if let Err(e) = hooks.run_hook(HookType::PreArchive, &hook_ctx).await {
        output.on_warn(&format!("pre_archive hook failed: {}", e));
        return Err(e);
    }

    output.on_info(&format!("Archiving: {}", change.id));

    // Execute archive command
    let status = agent.run_archive(&change.id).await?;

    if !status.success() {
        let error_msg = format!("Archive command failed with exit code: {:?}", status.code());

        // Run on_error hook
        let error_ctx = hook_ctx.clone().with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_ctx).await;

        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    // Verify archive was successful
    if !verify_archive(&change.id) {
        let error_msg = format!(
            "Archive command succeeded but change '{}' was not actually archived. \
             The change directory still exists in openspec/changes/.",
            change.id
        );
        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    // Clear apply history for the archived change
    agent.clear_apply_history(&change.id);

    // Run post_archive hook with updated context
    let post_ctx = HookContext::new(
        context.changes_processed + 1,
        context.total_changes,
        context.remaining_changes.saturating_sub(1),
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    if let Err(e) = hooks.run_hook(HookType::PostArchive, &post_ctx).await {
        output.on_warn(&format!("post_archive hook failed: {}", e));
        return Err(e);
    }

    info!("Successfully archived: {}", change.id);
    output.on_success(&format!("Archived: {}", change.id));

    Ok(ArchiveResult::Success)
}

/// Archive a change with streaming output.
///
/// Similar to `archive_change` but uses streaming output for real-time feedback.
/// This is primarily used by TUI mode.
///
/// # Arguments
/// * `change` - The change to archive
/// * `agent` - The agent runner for executing commands
/// * `hooks` - The hook runner for executing hooks
/// * `context` - Context information for hooks
/// * `output` - Output handler for streaming command output
/// * `cancel_check` - Function to check if operation should be cancelled
///
/// # Returns
/// Same as `archive_change`
pub async fn archive_change_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ArchiveContext,
    output: &O,
    cancel_check: F,
) -> Result<ArchiveResult>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::agent::OutputLine;

    info!("Archiving change (streaming): {}", change.id);

    // Build hook context
    let hook_ctx = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    // Run on_change_complete hook
    if let Err(e) = hooks.run_hook(HookType::OnChangeComplete, &hook_ctx).await {
        output.on_warn(&format!("on_change_complete hook failed: {}", e));
        return Err(e);
    }

    // Run pre_archive hook
    if let Err(e) = hooks.run_hook(HookType::PreArchive, &hook_ctx).await {
        output.on_warn(&format!("pre_archive hook failed: {}", e));
        return Err(e);
    }

    output.on_info(&format!("Archiving: {}", change.id));

    // Execute archive command with streaming
    let (mut child, mut output_rx) = agent.run_archive_streaming(&change.id).await?;

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
            return Ok(ArchiveResult::Cancelled);
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
        OrchestratorError::AgentCommand(format!("Failed to wait for process: {}", e))
    })?;

    if !status.success() {
        let error_msg = format!("Archive command failed with exit code: {:?}", status.code());

        // Run on_error hook
        let error_ctx = hook_ctx.clone().with_error(&error_msg);
        let _ = hooks.run_hook(HookType::OnError, &error_ctx).await;

        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    // Verify archive was successful
    if !verify_archive(&change.id) {
        let error_msg = format!(
            "Archive command succeeded but change '{}' was not actually archived.",
            change.id
        );
        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    // Clear apply history
    agent.clear_apply_history(&change.id);

    // Run post_archive hook
    let post_ctx = HookContext::new(
        context.changes_processed + 1,
        context.total_changes,
        context.remaining_changes.saturating_sub(1),
        false,
    )
    .with_change(&change.id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);

    if let Err(e) = hooks.run_hook(HookType::PostArchive, &post_ctx).await {
        output.on_warn(&format!("post_archive hook failed: {}", e));
        return Err(e);
    }

    info!("Successfully archived: {}", change.id);
    output.on_success(&format!("Archived: {}", change.id));

    Ok(ArchiveResult::Success)
}

/// Verify that a change was actually archived.
///
/// Checks that:
/// - The change directory no longer exists in `openspec/changes/`
/// - OR the archive directory exists in `openspec/changes/archive/`
fn verify_archive(change_id: &str) -> bool {
    let change_path = Path::new("openspec/changes").join(change_id);
    let archive_path = Path::new("openspec/changes/archive").join(change_id);

    let change_exists = change_path.exists();
    let archive_exists = archive_path.exists();

    debug!(
        change_id = %change_id,
        change_path = %change_path.display(),
        archive_path = %archive_path.display(),
        change_exists = change_exists,
        archive_exists = archive_exists,
        "verify_archive: checking paths"
    );

    // Archive is successful if:
    // - Change directory doesn't exist (was moved/deleted)
    // - OR archive directory exists (was archived)
    !change_exists || archive_exists
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_result_is_success() {
        assert!(ArchiveResult::Success.is_success());
        assert!(!ArchiveResult::Failed {
            error: "test".to_string()
        }
        .is_success());
        assert!(!ArchiveResult::Cancelled.is_success());
    }

    #[test]
    fn test_archive_context_new() {
        let ctx = ArchiveContext::new(1, 5, 4, 2);
        assert_eq!(ctx.changes_processed, 1);
        assert_eq!(ctx.total_changes, 5);
        assert_eq!(ctx.remaining_changes, 4);
        assert_eq!(ctx.apply_count, 2);
    }

    #[test]
    fn test_verify_archive_path_structure() {
        // This test verifies the path structure is correct
        let change_id = "test-change";
        let change_path = Path::new("openspec/changes").join(change_id);
        let archive_path = Path::new("openspec/changes/archive").join(change_id);

        assert_eq!(
            change_path.to_str().unwrap(),
            "openspec/changes/test-change"
        );
        assert_eq!(
            archive_path.to_str().unwrap(),
            "openspec/changes/archive/test-change"
        );

        // Archive path should be under openspec/changes/archive, not openspec/archive
        assert!(archive_path.starts_with("openspec/changes/archive"));
    }
}
