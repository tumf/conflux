//! Shared archive operations for CLI and TUI modes.
//!
//! Provides a unified archive implementation that both modes can use,
//! eliminating code duplication and ensuring consistent behavior.
//!
//! Note: These functions are infrastructure for future CLI/TUI integration.
//! They will be used as the refactoring continues.

#![allow(dead_code)]

use crate::agent::AgentRunner;
use crate::config::StallDetectionConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::archive::ArchiveVerificationResult;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::Change;
use crate::vcs::git::commands as git_commands;
use std::path::Path;
use tracing::info;

use super::output::OutputHandler;

/// Result of an archive operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveResult {
    /// Archive completed successfully.
    Success,
    /// Archive stalled due to repeated empty WIP commits.
    Stalled { error: String },
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
/// * `output` - Output handler for logging
/// * `base_path` - Optional base path for archive verification
/// * `stall_config` - Stall detection configuration for empty WIP commits
///
/// # Returns
/// Same as `archive_change_streaming`
pub async fn archive_change<O>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ArchiveContext,
    output: &O,
    base_path: Option<&Path>,
    stall_config: &StallDetectionConfig,
) -> Result<ArchiveResult>
where
    O: OutputHandler,
{
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

    use crate::execution::archive::{
        build_archive_error_message, verify_archive_completion, ARCHIVE_COMMAND_MAX_RETRIES,
    };

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;
    let repo_root = base_path.unwrap_or_else(|| Path::new("."));
    let is_git_repo = match git_commands::check_git_repo(repo_root).await {
        Ok(is_repo) => is_repo,
        Err(e) => {
            output.on_warn(&format!("Failed to check Git repository status: {}", e));
            false
        }
    };
    let mut empty_commit_streak = 0u32;

    loop {
        attempt += 1;

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

        if is_git_repo {
            if let Err(e) =
                git_commands::create_archive_wip_commit(repo_root, &change.id, attempt).await
            {
                output.on_warn(&format!(
                    "Failed to create WIP(archive) commit for {} (attempt {}): {}",
                    change.id, attempt, e
                ));
            } else if stall_config.enabled {
                match git_commands::is_head_empty_commit(repo_root).await {
                    Ok(is_empty) => {
                        if is_empty {
                            empty_commit_streak = empty_commit_streak.saturating_add(1);
                        } else {
                            empty_commit_streak = 0;
                        }
                        if empty_commit_streak >= stall_config.threshold {
                            let message = format!(
                                "Stall detected for {} after {} empty WIP commits (archive)",
                                change.id, empty_commit_streak
                            );
                            output.on_warn(&format!(
                                "{} (threshold {})",
                                message, stall_config.threshold
                            ));
                            return Ok(ArchiveResult::Stalled { error: message });
                        }
                    }
                    Err(e) => {
                        output.on_warn(&format!(
                            "Failed to check WIP(archive) commit for {} (attempt {}): {}",
                            change.id, attempt, e
                        ));
                    }
                }
            }
        }

        // Verify archive was successful
        if verify_archive_completion(&change.id, base_path).is_success() {
            break;
        }

        if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
            output.on_warn(&format!(
                "Archive verification failed for {} (attempt {}/{}); retrying archive command",
                change.id, attempt, max_attempts
            ));
            continue;
        }

        let error_msg = build_archive_error_message(&change.id);
        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    if is_git_repo {
        if let Err(e) = git_commands::squash_archive_wip_commits(repo_root, &change.id).await {
            output.on_warn(&format!(
                "Failed to squash WIP(archive) commits for {}: {}",
                change.id, e
            ));
        }
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
/// * `base_path` - Optional base path for archive verification
/// * `stall_config` - Stall detection configuration for empty WIP commits
///
/// # Returns
/// Same as `archive_change`
#[allow(clippy::too_many_arguments)]
pub async fn archive_change_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ArchiveContext,
    output: &O,
    cancel_check: F,
    base_path: Option<&Path>,
    stall_config: &StallDetectionConfig,
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

    use crate::execution::archive::{
        build_archive_error_message, verify_archive_completion, ARCHIVE_COMMAND_MAX_RETRIES,
    };

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;
    let repo_root = base_path.unwrap_or_else(|| Path::new("."));
    let is_git_repo = match git_commands::check_git_repo(repo_root).await {
        Ok(is_repo) => is_repo,
        Err(e) => {
            output.on_warn(&format!("Failed to check Git repository status: {}", e));
            false
        }
    };
    let mut empty_commit_streak = 0u32;

    loop {
        attempt += 1;

        // Execute archive command with streaming
        let (mut child, mut output_rx, start) =
            agent.run_archive_streaming(&change.id, None).await?;

        // Stream output
        loop {
            if cancel_check() {
                let _ = child.terminate();
                let _ = child.kill().await;
                output.on_warn("Process killed due to cancellation");
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

            // Record the failed attempt
            agent.record_archive_attempt(&change.id, &status, start, Some(error_msg.clone()));

            // Run on_error hook
            let error_ctx = hook_ctx.clone().with_error(&error_msg);
            let _ = hooks.run_hook(HookType::OnError, &error_ctx).await;

            output.on_error(&error_msg);
            return Ok(ArchiveResult::Failed { error: error_msg });
        }

        if is_git_repo {
            if let Err(e) =
                git_commands::create_archive_wip_commit(repo_root, &change.id, attempt).await
            {
                output.on_warn(&format!(
                    "Failed to create WIP(archive) commit for {} (attempt {}): {}",
                    change.id, attempt, e
                ));
            } else if stall_config.enabled {
                match git_commands::is_head_empty_commit(repo_root).await {
                    Ok(is_empty) => {
                        if is_empty {
                            empty_commit_streak = empty_commit_streak.saturating_add(1);
                        } else {
                            empty_commit_streak = 0;
                        }
                        if empty_commit_streak >= stall_config.threshold {
                            let message = format!(
                                "Stall detected for {} after {} empty WIP commits (archive)",
                                change.id, empty_commit_streak
                            );
                            output.on_warn(&format!(
                                "{} (threshold {})",
                                message, stall_config.threshold
                            ));
                            return Ok(ArchiveResult::Stalled { error: message });
                        }
                    }
                    Err(e) => {
                        output.on_warn(&format!(
                            "Failed to check WIP(archive) commit for {} (attempt {}): {}",
                            change.id, attempt, e
                        ));
                    }
                }
            }
        }

        // Verify archive was successful
        let verification_status = verify_archive_completion(&change.id, base_path);
        if verification_status.is_success() {
            // Record successful archive attempt
            agent.record_archive_attempt(&change.id, &status, start, None);
            break;
        }

        // Verification failed - record with reason
        let verification_reason = match verification_status {
            ArchiveVerificationResult::NotArchived { ref change_id } => {
                format!("Change still exists at openspec/changes/{}", change_id)
            }
            _ => "Archive verification failed".to_string(),
        };
        agent.record_archive_attempt(
            &change.id,
            &status,
            start,
            Some(verification_reason.clone()),
        );

        if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
            output.on_warn(&format!(
                "Archive verification failed for {} (attempt {}/{}): {}; retrying archive command",
                change.id, attempt, max_attempts, verification_reason
            ));
            continue;
        }

        let error_msg = build_archive_error_message(&change.id);
        output.on_error(&error_msg);
        return Ok(ArchiveResult::Failed { error: error_msg });
    }

    if is_git_repo {
        if let Err(e) = git_commands::squash_archive_wip_commits(repo_root, &change.id).await {
            output.on_warn(&format!(
                "Failed to squash WIP(archive) commits for {}: {}",
                change.id, e
            ));
        }
    }

    // Clear apply and archive history
    agent.clear_apply_history(&change.id);
    agent.clear_archive_history(&change.id);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;
    use crate::hooks::HookRunner;
    use crate::openspec::Change;
    use crate::orchestration::output::NullOutputHandler;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_archive_result_is_success() {
        assert!(ArchiveResult::Success.is_success());
        assert!(!ArchiveResult::Stalled {
            error: "stalled".to_string()
        }
        .is_success());
        assert!(!ArchiveResult::Failed {
            error: "oops".to_string()
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

    #[cfg(unix)]
    #[tokio::test]
    async fn test_archive_change_retries_until_verified() {
        let temp_dir = TempDir::new().unwrap();

        let change_id = "retry-change";
        let change_dir = temp_dir.path().join("openspec/changes").join(change_id);
        let archive_dir = temp_dir.path().join("openspec/changes/archive");
        fs::create_dir_all(&change_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let attempts_path = temp_dir.path().join("archive_attempts.txt");
        let script_path = temp_dir.path().join("archive.sh");
        let script = format!(
            r#"#!/bin/sh
attempts_file="{attempts}"
base_dir="{base_dir}"
count=0
if [ -f "$attempts_file" ]; then
  count=$(cat "$attempts_file")
fi
count=$((count+1))
echo "$count" > "$attempts_file"
if [ "$count" -lt 2 ]; then
  exit 0
fi
mkdir -p "$base_dir/openspec/changes/archive"
mv "$base_dir/openspec/changes/$1" "$base_dir/openspec/changes/archive/$1"
"#,
            attempts = attempts_path.display(),
            base_dir = temp_dir.path().display()
        );
        fs::write(&script_path, script).unwrap();

        let config = OrchestratorConfig {
            archive_command: Some(format!("sh \"{}\" {{change_id}}", script_path.display())),
            ..Default::default()
        };

        let mut agent = AgentRunner::new(config);
        let hooks = HookRunner::empty();
        let output = NullOutputHandler::new();
        let context = ArchiveContext::new(0, 1, 1, 0);
        let change = Change {
            id: change_id.to_string(),
            completed_tasks: 1,
            total_tasks: 1,
            last_modified: "".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let stall_config = OrchestratorConfig::default().get_stall_detection();
        let result = archive_change(
            &change,
            &mut agent,
            &hooks,
            &context,
            &output,
            Some(temp_dir.path()),
            &stall_config,
        )
        .await
        .unwrap();

        assert_eq!(result, ArchiveResult::Success);

        let attempts = fs::read_to_string(&attempts_path).unwrap();
        let attempt_count: u32 = attempts.trim().parse().unwrap();
        assert_eq!(attempt_count, 2);

        let archived_dir = temp_dir
            .path()
            .join("openspec/changes/archive")
            .join(change_id);
        assert!(!change_dir.exists());
        assert!(archived_dir.exists());
    }
}
