//! Workspace execution logic for apply and archive operations.

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::parallel::output_bridge::ParallelApplyEventHandler;

use super::acceptance_state::delete_acceptance_state;
use super::archive_state::delete_archive_state;
use super::events::ParallelEvent;
use crate::orchestration::build_acceptance_tail_findings;
use crate::stall::StallDetector;
use crate::vcs::git::commands as git_commands;
use crate::vcs::git::commands::has_uncommitted_changes;
use crate::vcs::git::GitWorkspaceManager;
use crate::vcs::VcsBackend;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Parallel execution context for hooks
#[derive(Debug, Clone, Default)]
pub struct ParallelHookContext {
    /// Workspace path (set as OPENSPEC_WORKSPACE_PATH env var)
    pub workspace_path: String,
    /// Group index (set as OPENSPEC_GROUP_INDEX env var)
    pub group_index: Option<u32>,
    /// Total changes being processed in this group
    #[allow(dead_code)] // Available for future use in hook context
    pub total_changes_in_group: usize,
    /// Total changes in the run
    pub total_changes: usize,
    /// Changes processed so far
    pub changes_processed: usize,
}

/// Build a HookContext for parallel mode with workspace-specific environment variables.
fn build_parallel_hook_context(
    change_id: &str,
    completed_tasks: u32,
    total_tasks: u32,
    apply_count: u32,
    parallel_ctx: Option<&ParallelHookContext>,
) -> HookContext {
    let (changes_processed, total_changes, remaining_changes) = match parallel_ctx {
        Some(ctx) => (
            ctx.changes_processed,
            ctx.total_changes,
            ctx.total_changes.saturating_sub(ctx.changes_processed),
        ),
        None => (0, 0, 0),
    };

    let mut ctx = HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(change_id, completed_tasks, total_tasks)
        .with_apply_count(apply_count);

    // Add parallel-specific environment variables
    if let Some(parallel_ctx) = parallel_ctx {
        // These will be added to env_vars through a custom method
        ctx = ctx.with_parallel_context(&parallel_ctx.workspace_path, parallel_ctx.group_index);
    }

    ctx
}

async fn run_post_apply_cleanup_review(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
) -> Result<()> {
    let user_template = config.get_acceptance_command()?;
    let prompt = crate::agent::build_cleanup_review_prompt_with_skill(
        config.get_cleanup_review_skill(),
        change_id,
    );
    let command = OrchestratorConfig::expand_prompt(
        &OrchestratorConfig::expand_change_id(user_template, change_id),
        &prompt,
    );

    info!(
        change_id = %change_id,
        workspace = %workspace_path.display(),
        "Starting post-apply cleanup review for dirty managed worktree"
    );

    let (mut child, mut output_rx) = ai_runner
        .execute_streaming_with_retry(
            &command,
            Some(workspace_path),
            Some("cleanup-review"),
            Some(change_id),
        )
        .await?;

    let mut stdout = String::new();
    while let Some(line) = output_rx.recv().await {
        match line {
            crate::ai_command_runner::OutputLine::Stdout(s) => {
                stdout.push_str(&s);
                stdout.push('\n');
            }
            crate::ai_command_runner::OutputLine::Stderr(_) => {}
        }
    }

    let status = child.wait().await.map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to wait for cleanup-review command for change '{}' in workspace '{}': {}",
            change_id,
            workspace_path.display(),
            e
        ))
    })?;

    if !status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cleanup-review command failed with exit code {:?} for change '{}'",
            status.code(),
            change_id
        )));
    }

    if !crate::agent::parse_cleanup_review_output(&stdout) {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cleanup-review output missing required single marker CLEANUP_REVIEW: CLEAN for change '{}'",
            change_id
        )));
    }

    let (still_dirty, dirty_status) =
        has_uncommitted_changes(workspace_path).await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to verify post-cleanup dirty state for change '{}': {}",
                change_id, e
            ))
        })?;

    if still_dirty {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cleanup-review reported clean marker but worktree remains dirty for change '{}': {}",
            change_id, dirty_status
        )));
    }

    info!(
        change_id = %change_id,
        workspace = %workspace_path.display(),
        "Post-apply cleanup review succeeded and worktree is clean"
    );

    Ok(())
}

/// Execute apply command in a single workspace, repeating until tasks are 100% complete.
///
/// Returns (revision, final_iteration_count) on success.
#[allow(clippy::too_many_arguments)]
pub async fn execute_apply_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    _apply_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    parallel_ctx: Option<&ParallelHookContext>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    repo_root: &Path,
    _apply_history: &Arc<Mutex<crate::history::ApplyHistory>>,
    _acceptance_history: &Arc<Mutex<crate::history::AcceptanceHistory>>,
    _acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
    _initial_iteration: u32,
) -> Result<(
    String,
    u32,
    Option<crate::execution::apply::ApplyBlockedHandoff>,
    Option<crate::execution::apply::ApplyRejectedHandoff>,
)> {
    // Validate that workspace_path is a worktree, not the base repository
    match git_commands::is_worktree(repo_root, workspace_path).await {
        Ok(true) => {
            info!(
                "Workspace path validation passed: {} is a valid worktree",
                workspace_path.display()
            );
        }
        Ok(false) => {
            let error_msg = format!(
                "Parallel apply execution guard: workspace_path is NOT a worktree (executing in base repository is forbidden)
\
                 change_id: {}
\
                 workspace_path: {}
\
                 repo_root: {}",
                change_id,
                workspace_path.display(),
                repo_root.display()
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to validate worktree status for parallel apply
\
                 change_id: {}
\
                 workspace_path: {}
\
                 repo_root: {}
\
                 validation_error: {}",
                change_id,
                workspace_path.display(),
                repo_root.display(),
                e
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
    }

    // Create AgentRunner for execute_apply_loop
    let mut agent = AgentRunner::new(config.clone());

    // Clone event_tx for permission error handling before moving it to event_handler
    let event_tx_for_permission = event_tx.clone();

    // Create event handler for apply loop
    let event_handler = ParallelApplyEventHandler::new(change_id.to_string(), event_tx);

    // Create hook context for apply loop
    let hook_ctx = if let Some(ctx) = parallel_ctx {
        let remaining_changes = ctx.total_changes.saturating_sub(ctx.changes_processed);
        common_apply::ApplyLoopHookContext::parallel(
            ctx.changes_processed,
            ctx.total_changes,
            remaining_changes,
            workspace_path.to_string_lossy().to_string(),
            ctx.group_index.unwrap_or(0) as usize,
        )
    } else {
        common_apply::ApplyLoopHookContext::serial(0, 0, 0)
    };

    // Create workspace manager for WIP commit/stall detection in parallel mode
    // The workspace (Git worktree) is already created, so we just need a manager
    // for commit operations
    let workspace_manager = GitWorkspaceManager::new(
        workspace_path.parent().unwrap_or(repo_root).to_path_buf(),
        repo_root.to_path_buf(),
        1, // max_concurrent (not used for existing worktree)
        config.clone(),
    );

    // Execute apply loop using common implementation
    let apply_result = match common_apply::execute_apply_loop(
        change_id,
        workspace_path,
        config,
        &mut agent,
        vcs_backend,
        Some(&workspace_manager), // Pass workspace_manager for WIP commits and stall detection
        hooks,
        &hook_ctx,
        &event_handler,
        cancel_token,
        ai_runner,
        |_line| async move {
            // Output is handled by event_handler
        },
    )
    .await
    {
        Ok(result) => result,
        Err(crate::error::OrchestratorError::PermissionBlocked {
            denied_path,
            guidance,
        }) => {
            // Permission blocked - emit event and return error with guidance
            use tracing::warn;
            warn!(
                "Permission auto-rejected for {} in workspace {}: {}",
                change_id,
                workspace_path.display(),
                denied_path
            );

            // Send event if event_tx is available
            if let Some(ref tx) = event_tx_for_permission {
                use crate::parallel::ParallelEvent;
                let _ = tx
                    .send(ParallelEvent::ApplyFailed {
                        change_id: change_id.to_string(),
                        error: format!("Permission auto-rejected: {}\n{}", denied_path, guidance),
                    })
                    .await;
            }

            return Err(crate::error::OrchestratorError::PermissionBlocked {
                denied_path,
                guidance,
            });
        }
        Err(e) => return Err(e),
    };

    if apply_result.blocked_handoff.is_none() && apply_result.rejected_handoff.is_none() {
        let (is_dirty, dirty_status) =
            has_uncommitted_changes(workspace_path).await.map_err(|e| {
                OrchestratorError::AgentCommand(format!(
                    "Failed to inspect worktree dirty state after apply completion for '{}': {}",
                    change_id, e
                ))
            })?;

        if is_dirty {
            warn!(
                change_id = %change_id,
                workspace = %workspace_path.display(),
                dirty_status = %dirty_status,
                "Managed worktree is dirty after apply completion; running post-apply cleanup review before acceptance handoff"
            );
            run_post_apply_cleanup_review(change_id, workspace_path, config, ai_runner).await?;
        }
    }

    info!(
        "Apply completed for {} (revision={})",
        change_id, apply_result.revision
    );

    // Return revision, iteration count, and apply-blocked handoff metadata
    Ok((
        apply_result.revision,
        apply_result.iterations,
        apply_result.blocked_handoff,
        apply_result.rejected_handoff,
    ))
}

/// Execute archive command in a workspace with streaming output
#[allow(clippy::too_many_arguments)]
pub async fn execute_archive_finalization_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    ai_runner: &AiCommandRunner,
    shared_stagger_state: &Arc<Mutex<Option<std::time::Instant>>>,
) -> Result<String> {
    use crate::execution::archive::{ensure_archive_commit, verify_archive_completion};

    let verification = verify_archive_completion(change_id, Some(workspace_path));
    if !verification.is_success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cannot resume archive finalization for '{}': archive move has regressed; active change directory is present or archive files are missing in '{}'",
            change_id,
            workspace_path.display()
        )));
    }

    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::ArchiveResumed {
                change_id: change_id.to_string(),
                reason: Some("archive_commit_incomplete".to_string()),
                summary: Some(
                    "Archive move is already complete; resuming commit finalization only"
                        .to_string(),
                ),
            })
            .await;
    }

    let resolve_agent =
        AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());
    let change_id_owned = change_id.to_string();
    let event_tx_clone = event_tx.clone();
    ensure_archive_commit(
        change_id,
        workspace_path,
        &resolve_agent,
        ai_runner,
        vcs_backend,
        move |line| {
            let event_tx = event_tx_clone.clone();
            let change_id = change_id_owned.clone();
            async move {
                let text = match line {
                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                };
                if let Some(ref tx) = event_tx {
                    if text.contains("Archive commit finalization retry scheduled") {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                crate::events::LogEntry::warn(text.clone())
                                    .with_change_id(&change_id)
                                    .with_operation("archive-finalization"),
                            ))
                            .await;
                    }
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id,
                            output: text,
                            iteration: 1,
                        })
                        .await;
                }
            }
        },
    )
    .await?;

    if let Err(err) = delete_acceptance_state(workspace_path) {
        warn!(
            "Failed to delete acceptance state for {} after archive finalization resume: {}",
            change_id, err
        );
    }
    if let Err(err) = delete_archive_state(workspace_path) {
        warn!(
            "Failed to delete archive state for {} after archive finalization resume: {}",
            change_id, err
        );
    }

    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            let revision = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!(
                        "Failed to get revision after archive finalization resume: {}",
                        e
                    ))
                })?;
            if revision.status.success() {
                Ok(String::from_utf8_lossy(&revision.stdout).trim().to_string())
            } else {
                Err(OrchestratorError::GitCommand(format!(
                    "Failed to get revision after archive finalization resume: {}",
                    String::from_utf8_lossy(&revision.stderr).trim()
                )))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_archive_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    archive_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    parallel_ctx: Option<&ParallelHookContext>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    archive_history: &Arc<Mutex<crate::history::ArchiveHistory>>,
    apply_history: &Arc<Mutex<crate::history::ApplyHistory>>,
    shared_stagger_state: &Arc<Mutex<Option<std::time::Instant>>>,
) -> Result<String> {
    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cancelled archive for '{}' in workspace '{}'",
            change_id,
            workspace_path.display()
        )));
    }

    // Verify task completion before archiving using common function
    use crate::execution::archive::get_task_progress;

    let progress = match get_task_progress(change_id, Some(workspace_path)) {
        Ok(Some(progress)) => {
            if progress.total == 0 {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive '{}' in workspace '{}': tasks.md exists but contains no tasks (0 tasks found)",
                    change_id,
                    workspace_path.display()
                )));
            }
            if progress.completed < progress.total {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive '{}' in workspace '{}': tasks not complete ({}/{} tasks completed)",
                    change_id,
                    workspace_path.display(),
                    progress.completed,
                    progress.total
                )));
            }
            info!(
                "Task verification passed for {}: {}/{} tasks completed",
                change_id, progress.completed, progress.total
            );
            progress
        }
        Ok(None) => {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': tasks.md not found at {} or in archive directory",
                change_id,
                workspace_path.display(),
                workspace_path
                    .join("openspec/changes")
                    .join(change_id)
                    .join("tasks.md")
                    .display()
            )));
        }
        Err(e) => {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': failed to parse tasks.md: {}",
                change_id,
                workspace_path.display(),
                e
            )));
        }
    };

    crate::vcs::git::commands::get_current_commit(workspace_path)
        .await
        .map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': failed to resolve current revision: {}",
                change_id,
                workspace_path.display(),
                e
            ))
        })?;

    let stall_detector = StallDetector::new(config.get_stall_detection());

    // Run pre_archive hook
    if let Some(hook_runner) = hooks {
        let hook_ctx = build_parallel_hook_context(
            change_id,
            progress.completed,
            progress.total,
            0, // apply_count not relevant for archive
            parallel_ctx,
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::HookStarted {
                    change_id: change_id.to_string(),
                    hook_type: "pre_archive".to_string(),
                })
                .await;
        }

        match hook_runner.run_hook(HookType::PreArchive, &hook_ctx).await {
            Ok(()) => {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookCompleted {
                            change_id: change_id.to_string(),
                            hook_type: "pre_archive".to_string(),
                        })
                        .await;
                }
            }
            Err(e) => {
                error!("pre_archive hook failed for {}: {}", change_id, e);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookFailed {
                            change_id: change_id.to_string(),
                            hook_type: "pre_archive".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                return Err(e);
            }
        }
    }

    // Build prompt with history context
    let user_prompt = config.get_archive_prompt();
    let history_context = {
        let history = archive_history.lock().await;
        history.format_context(change_id)
    };
    let full_prompt = crate::agent::build_archive_prompt_with_skill(
        config.get_archive_skill(),
        change_id,
        user_prompt,
        &history_context,
    );

    // Expand change_id and prompt in archive command
    let command = OrchestratorConfig::expand_change_id(archive_cmd_template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

    debug!("Archive command in workspace: {}", command);

    // Send ArchiveStarted event with expanded command
    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::ArchiveStarted {
                change_id: change_id.to_string(),
                command: command.clone(),
            })
            .await;
    }

    use crate::execution::archive::{
        build_archive_error_message, ensure_archive_commit, extract_archive_runtime_blocker,
        verify_archive_completion, ARCHIVE_COMMAND_MAX_RETRIES,
    };
    use crate::history::ArchivePrimaryReason;

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;
    let is_git_repo = if matches!(vcs_backend, VcsBackend::Git | VcsBackend::Auto) {
        match git_commands::check_git_repo(workspace_path).await {
            Ok(is_repo) => is_repo,
            Err(e) => {
                warn!(
                    "Failed to check Git repository status for {}: {}",
                    change_id, e
                );
                false
            }
        }
    } else {
        false
    };
    let mut empty_commit_streak = 0u32;

    loop {
        attempt += 1;
        let start = std::time::Instant::now();

        // Execute command via AiCommandRunner (with stagger and retry)
        // Execute in workspace directory (cwd parameter)
        debug!(
            module = module_path!(),
            "Executing shell command via AiCommandRunner with retry: {} (cwd: {:?})",
            command,
            workspace_path
        );
        let (mut child, mut output_rx) = ai_runner
            .execute_streaming_with_retry(
                &command,
                Some(workspace_path),
                Some("archive"),
                Some(change_id),
            )
            .await?;

        // Create output collector for history
        let mut output_collector = crate::history::OutputCollector::new();

        // Forward output to event channel
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let change_id_clone = change_id.to_string();
        let event_tx_clone = event_tx.clone();
        while let Some(line) = output_rx.recv().await {
            // Collect output for history
            match &line {
                AiOutputLine::Stdout(s) => output_collector.add_stdout(s),
                AiOutputLine::Stderr(s) => output_collector.add_stderr(s),
            }

            if let Some(ref tx) = event_tx_clone {
                let output_text = match line {
                    AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s,
                };
                let _ = tx
                    .send(ParallelEvent::ArchiveOutput {
                        change_id: change_id_clone.clone(),
                        output: output_text,
                        iteration: attempt,
                    })
                    .await;
            }
        }

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to wait for archive command for '{}' in workspace '{}' (attempt {}): {}",
                change_id,
                workspace_path.display(),
                attempt,
                e
            ))
        })?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive command failed for change '{}' in workspace '{}' (attempt {}) with exit code: {:?}",
                change_id,
                workspace_path.display(),
                attempt,
                status.code()
            )));
        }

        if is_git_repo {
            if let Err(e) =
                git_commands::create_archive_wip_commit(workspace_path, change_id, attempt).await
            {
                warn!(
                    "Failed to create WIP(archive) commit for {} (attempt {}): {}",
                    change_id, attempt, e
                );
            } else if stall_detector.config().enabled {
                match git_commands::is_head_empty_commit(workspace_path).await {
                    Ok(is_empty) => {
                        if is_empty {
                            empty_commit_streak = empty_commit_streak.saturating_add(1);
                        } else {
                            empty_commit_streak = 0;
                        }
                        if empty_commit_streak >= stall_detector.config().threshold {
                            let message = format!(
                                "Stall detected for {} after {} empty WIP commits (archive)",
                                change_id, empty_commit_streak
                            );
                            warn!(
                                "{} (threshold {})",
                                message,
                                stall_detector.config().threshold
                            );
                            return Err(OrchestratorError::AgentCommand(message));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check WIP(archive) commit for {} (attempt {}): {}",
                            change_id, attempt, e
                        );
                    }
                }
            }
        }

        let verification = verify_archive_completion(change_id, Some(workspace_path));

        // Record archive attempt in history
        {
            let mut history = archive_history.lock().await;
            let verification_result = if verification.is_success() {
                None
            } else {
                Some(format!(
                    "Change still exists at openspec/changes/{}",
                    change_id
                ))
            };
            let attempt_record = crate::history::ArchiveAttempt {
                attempt: history.count(change_id) + 1,
                success: status.success() && verification.is_success(),
                duration: start.elapsed(),
                error: if status.success() && verification.is_success() {
                    None
                } else if !status.success() {
                    Some(format!("Exit code: {:?}", status.code()))
                } else {
                    Some("Archive command succeeded but verification failed".to_string())
                },
                primary_reason: if status.success() && verification.is_success() {
                    None
                } else if !status.success() {
                    Some(ArchivePrimaryReason::CommandFailed)
                } else {
                    Some(ArchivePrimaryReason::VerificationFailed)
                },
                verification_result,
                exit_code: status.code(),
                stdout_tail: output_collector.stdout_tail(),
                stderr_tail: output_collector.stderr_tail(),
            };
            history.record(change_id, attempt_record);
        }

        if verification.is_success() {
            break;
        }

        if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
            let retry_summary =
                "archive verification failed; change directory still exists".to_string();
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ArchiveRetryScheduled {
                        change_id: change_id.to_string(),
                        attempt,
                        max_attempts,
                        reason: Some(
                            ArchivePrimaryReason::VerificationFailed
                                .as_str()
                                .to_string(),
                        ),
                        summary: Some(retry_summary),
                    })
                    .await;
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Archive verification failed for {} (attempt {}/{}); retrying archive command",
                            change_id, attempt, max_attempts
                        ))
                        .with_change_id(change_id)
                        .with_operation("archive")
                        .with_iteration(attempt),
                    ))
                    .await;
            }
            warn!(
                change_id = %change_id,
                attempt = attempt,
                max_attempts = max_attempts,
                "Archive verification failed; retrying archive command"
            );
            continue;
        }

        let runtime_blocker = extract_archive_runtime_blocker(
            output_collector.stdout_tail().as_deref(),
            output_collector.stderr_tail().as_deref(),
        );
        let final_error = build_archive_error_message(
            change_id,
            Some(workspace_path),
            runtime_blocker.as_deref(),
        );
        return Err(OrchestratorError::AgentCommand(final_error));
    }

    info!(
        "Archive verification passed for {}: change moved to archive",
        change_id
    );

    if is_git_repo {
        if let Err(e) = git_commands::squash_archive_wip_commits(workspace_path, change_id).await {
            warn!(
                "Failed to squash WIP(archive) commits for {}: {}",
                change_id, e
            );
        }
    }

    let resolve_agent =
        AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());
    let change_id_owned = change_id.to_string();
    let event_tx_clone = event_tx.clone();
    let final_attempt = attempt;
    ensure_archive_commit(
        change_id,
        workspace_path,
        &resolve_agent,
        ai_runner,
        vcs_backend,
        move |line| {
            let event_tx = event_tx_clone.clone();
            let change_id = change_id_owned.clone();
            let iteration = final_attempt;
            async move {
                let text = match line {
                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                };
                if let Some(ref tx) = event_tx {
                    if text.contains("Archive commit finalization retry scheduled") {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                crate::events::LogEntry::warn(text.clone())
                                    .with_change_id(&change_id)
                                    .with_operation("archive-finalization")
                                    .with_iteration(iteration),
                            ))
                            .await;
                    }
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id,
                            output: text,
                            iteration,
                        })
                        .await;
                }
            }
        },
    )
    .await?;

    // Get the current revision after archive
    // Note: The worktree may have been deleted by the archive command (e.g., /conflux:archive),
    // so we need to handle the case where the Git repository is no longer accessible.
    let revision = match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            debug!(
                module = module_path!(),
                "Executing git command: git rev-parse HEAD (cwd: {:?})", workspace_path
            );

            // Check if the workspace path still exists and is a Git repository
            if !workspace_path.exists() {
                warn!(
                    "Workspace path {:?} no longer exists after archive (likely deleted by archive command), using placeholder revision",
                    workspace_path
                );
                "archived".to_string()
            } else {
                match Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(workspace_path)
                    .output()
                    .await
                {
                    Ok(revision_output) if revision_output.status.success() => {
                        String::from_utf8_lossy(&revision_output.stdout)
                            .trim()
                            .to_string()
                    }
                    Ok(revision_output) => {
                        let stderr = String::from_utf8_lossy(&revision_output.stderr);
                        warn!(
                            "Failed to get revision from workspace {:?} after archive: {} (likely deleted by archive command), using placeholder",
                            workspace_path, stderr
                        );
                        "archived".to_string()
                    }
                    Err(e) => {
                        warn!(
                            "Failed to execute git rev-parse in workspace {:?} after archive: {} (likely deleted by archive command), using placeholder",
                            workspace_path, e
                        );
                        "archived".to_string()
                    }
                }
            }
        }
    };

    // Run post_archive hook
    if let Some(hook_runner) = hooks {
        let hook_ctx = build_parallel_hook_context(
            change_id,
            progress.completed,
            progress.total,
            0, // apply_count not relevant for archive
            parallel_ctx,
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::HookStarted {
                    change_id: change_id.to_string(),
                    hook_type: "post_archive".to_string(),
                })
                .await;
        }

        match hook_runner.run_hook(HookType::PostArchive, &hook_ctx).await {
            Ok(()) => {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookCompleted {
                            change_id: change_id.to_string(),
                            hook_type: "post_archive".to_string(),
                        })
                        .await;
                }
            }
            Err(e) => {
                error!("post_archive hook failed for {}: {}", change_id, e);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookFailed {
                            change_id: change_id.to_string(),
                            hook_type: "post_archive".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                return Err(e);
            }
        }
    }

    if let Err(err) = delete_acceptance_state(workspace_path) {
        warn!(
            "Failed to delete acceptance state for {} after archive completion: {}",
            change_id, err
        );
    }

    // Clear history after successful archive
    {
        let mut apply_hist = apply_history.lock().await;
        apply_hist.clear(change_id);
        let mut archive_hist = archive_history.lock().await;
        archive_hist.clear(change_id);
    }

    if let Err(err) = delete_archive_state(workspace_path) {
        warn!(
            "Failed to delete archive state for {} after archive completion: {}",
            change_id, err
        );
    }

    Ok(revision)
}

/// Execute acceptance test in a workspace with streaming output
fn format_acceptance_failure_log_message(findings: &[String]) -> String {
    let finding_count = findings.len();
    let blocking_gate_context = findings
        .first()
        .cloned()
        .unwrap_or_else(|| "no acceptance findings captured".to_string());

    format!(
        "Acceptance failed ({} findings), blocking gate context: {}",
        finding_count, blocking_gate_context
    )
}

fn resolve_acceptance_state_revision(start_revision: &str, end_revision: Option<String>) -> String {
    end_revision.unwrap_or_else(|| start_revision.to_string())
}

fn revision_to_history_commit_hash(revision: &str) -> Option<String> {
    if revision == "unknown" {
        None
    } else {
        Some(revision.to_string())
    }
}

fn write_acceptance_report(
    workspace_path: &Path,
    change_id: &str,
    attempt_number: u32,
    end_revision: &str,
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> Result<()> {
    let report_path = workspace_path.join("ACCEPTANCE_REPORT.json");
    let report = serde_json::json!({
        "change_id": change_id,
        "result": "pass",
        "attempt": attempt_number,
        "revision": revision_to_history_commit_hash(end_revision),
        "stdout_tail": stdout_tail,
        "stderr_tail": stderr_tail,
    });
    let report_json = serde_json::to_string_pretty(&report).map_err(OrchestratorError::Json)?;
    std::fs::write(&report_path, report_json).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to write acceptance report for change '{}' in workspace '{}': {}",
            change_id,
            workspace_path.display(),
            e
        ))
    })
}

const ACCEPTANCE_VERDICT_GRACE_DEFAULT_SECS: u64 = 30;

tokio::task_local! {
    /// Test-only task-local override for the verdict grace period. Scoped to
    /// the calling task via [`scoped_verdict_grace_secs_for_test`] so it does
    /// not leak across concurrent tests the way an env var or static atomic
    /// would.
    pub(crate) static VERDICT_GRACE_OVERRIDE_SECS: u64;
}

/// Returns the grace period applied after detecting a canonical standalone
/// acceptance verdict. Defaults to 30 seconds. Tests may override the value
/// for the current task via [`scoped_verdict_grace_secs_for_test`].
pub(crate) fn acceptance_verdict_grace_period() -> std::time::Duration {
    let secs = VERDICT_GRACE_OVERRIDE_SECS
        .try_with(|secs| *secs)
        .ok()
        .filter(|secs| *secs > 0)
        .unwrap_or(ACCEPTANCE_VERDICT_GRACE_DEFAULT_SECS);
    std::time::Duration::from_secs(secs)
}

/// Run `fut` with the verdict grace period overridden to `secs`. The override
/// is scoped to the current task only; concurrent tasks (including parallel
/// tests) keep seeing the default. Test-only helper.
#[cfg(test)]
pub(crate) async fn scoped_verdict_grace_secs_for_test<F, R>(secs: u64, fut: F) -> R
where
    F: std::future::Future<Output = R>,
{
    VERDICT_GRACE_OVERRIDE_SECS.scope(secs, fut).await
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_acceptance_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    agent: &mut AgentRunner,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    config: &OrchestratorConfig,
    acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
    acceptance_history: &Arc<Mutex<crate::history::AcceptanceHistory>>,
    base_branch: Option<&str>,
) -> Result<(crate::orchestration::AcceptanceResult, u32)> {
    use crate::acceptance::{parse_acceptance_output, AcceptanceResult as ParseResult};

    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
    }

    info!("Running acceptance test for {} in workspace", change_id);

    // Capture current commit hash for diff tracking
    let commit_hash = crate::vcs::git::commands::get_current_commit(workspace_path)
        .await
        .ok(); // Allow to fail silently (non-git repos)

    // Get the acceptance iteration number (attempt number that will be used)
    let acceptance_iteration = agent.next_acceptance_attempt_number(change_id);

    // Build prompt with system instructions and history context
    let user_prompt = config.get_acceptance_prompt();
    let history_context = agent.format_acceptance_history(change_id);

    // Build diff context for all acceptance attempts
    let diff_context = {
        // Get current commit hash
        let current_commit = crate::vcs::git::commands::get_current_commit(workspace_path)
            .await
            .ok();

        // Determine base commit for diff
        let base_commit = {
            let acc_history = acceptance_history.lock().await;
            if acc_history.count(change_id) == 0 {
                // First acceptance: use base branch
                base_branch.map(|b| b.to_string())
            } else {
                // 2nd+ acceptance: use last acceptance commit
                acc_history.last_commit_hash(change_id)
            }
        };

        // Get changed files if we have both base and current commits
        if let (Some(base), Some(current)) = (base_commit.as_ref(), current_commit.as_ref()) {
            match crate::vcs::git::commands::get_changed_files(workspace_path, Some(base), current)
                .await
            {
                Ok(files) => {
                    // Get previous findings for 2nd+ attempts
                    let previous_findings = {
                        let acc_history = acceptance_history.lock().await;
                        if acc_history.count(change_id) > 0 {
                            acc_history.last_findings(change_id)
                        } else {
                            None
                        }
                    };

                    // Build diff context if we have files or findings
                    if !files.is_empty() || previous_findings.is_some() {
                        crate::agent::build_acceptance_diff_context(
                            &files,
                            previous_findings.as_deref(),
                        )
                    } else {
                        String::new()
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to get changed files for acceptance diff context: {}",
                        e
                    );
                    String::new()
                }
            }
        } else {
            String::new()
        }
    };

    // Build last acceptance output context for 2nd+ attempts
    let stdout_tail = agent.get_last_acceptance_stdout_tail(change_id);
    let stderr_tail = agent.get_last_acceptance_stderr_tail(change_id);
    let last_output_context = crate::agent::build_last_acceptance_output_context(
        stdout_tail.as_deref(),
        stderr_tail.as_deref(),
    );

    // Build prompt injected into `{prompt}`
    let full_prompt = match config.get_acceptance_prompt_mode() {
        crate::config::AcceptancePromptMode::Full => {
            crate::agent::build_acceptance_prompt_with_skill(
                config.get_accept_skill(),
                change_id,
                user_prompt,
                &history_context,
                &last_output_context,
                &diff_context,
            )
        }
        crate::config::AcceptancePromptMode::ContextOnly => {
            crate::agent::build_acceptance_prompt_context_only_with_skill(
                config.get_accept_skill(),
                change_id,
                user_prompt,
                &history_context,
                &last_output_context,
                &diff_context,
            )
        }
    };

    // Expand change_id and prompt in command
    let template = config.get_acceptance_command()?;
    let command = OrchestratorConfig::expand_change_id(template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

    debug!(
        module = module_path!(),
        "Executing acceptance command via AiCommandRunner: {} (cwd: {:?})", command, workspace_path
    );

    let start_revision = commit_hash.clone().unwrap_or_else(|| "unknown".to_string());

    // Send AcceptanceStarted event with command
    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::AcceptanceStarted {
                change_id: change_id.to_string(),
                command: command.clone(),
            })
            .await;
    }

    // Capture start time for history recording
    let start_time = std::time::Instant::now();

    // Execute command via AiCommandRunner (with stagger and retry)
    let (mut child, mut output_rx) = ai_runner
        .execute_streaming_with_retry(
            &command,
            Some(workspace_path),
            Some("acceptance"),
            Some(change_id),
        )
        .await?;

    // Create output collector for history
    let mut output_collector = crate::history::OutputCollector::new();
    let mut full_stdout = String::new();

    // Grace period after detecting a canonical standalone verdict before
    // terminating the acceptance child process. This handles the case where
    // the agent process (e.g. opencode + MCP child processes) does not exit
    // promptly after emitting the verdict but keeps stdout/stderr pipes open,
    // which previously left acceptance to wait for inactivity timeout retry.
    let verdict_grace_period = acceptance_verdict_grace_period();

    let mut verdict_detected = false;
    let mut verdict_deadline: Option<tokio::time::Instant> = None;
    let mut early_terminated = false;

    // Stream output until channel closes or verdict grace period expires.
    use crate::ai_command_runner::OutputLine as AiOutputLine;
    loop {
        let recv_future = output_rx.recv();
        let line = if let Some(deadline) = verdict_deadline {
            match tokio::time::timeout_at(deadline, recv_future).await {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(_) => {
                    info!(
                        "Acceptance verdict grace period ({}s) expired for {}, terminating child process",
                        verdict_grace_period.as_secs(),
                        change_id
                    );
                    let _ = child.terminate();
                    early_terminated = true;
                    break;
                }
            }
        } else {
            match recv_future.await {
                Some(line) => line,
                None => break,
            }
        };

        // Check for cancellation
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            warn!("Acceptance test cancelled for: {}", change_id);
            let _ = child.terminate();
            return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
        }

        match line {
            AiOutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                full_stdout.push_str(&s);
                full_stdout.push('\n');

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::info(&s)
                                .with_change_id(change_id)
                                .with_operation("acceptance")
                                .with_iteration(acceptance_iteration),
                        ))
                        .await;
                }

                // Detect canonical verdict and start grace period.
                // Primary: strict JSON verdict object (standalone or wrapped in
                // an opencode `--format json` event). Fallback: strict plain-text
                // standalone canonical marker. Trailing-text concatenation on
                // the plain-text marker does NOT count.
                if !verdict_detected && crate::acceptance::detect_verdict_in_line(&s).is_some() {
                    verdict_detected = true;
                    verdict_deadline = Some(tokio::time::Instant::now() + verdict_grace_period);
                    info!(
                        "Acceptance canonical verdict detected for {}, starting {}s grace period",
                        change_id,
                        verdict_grace_period.as_secs()
                    );
                }
            }
            AiOutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::info(&s)
                                .with_change_id(change_id)
                                .with_operation("acceptance")
                                .with_iteration(acceptance_iteration),
                        ))
                        .await;
                }
            }
        }
    }

    // Wait for child process to complete. After early verdict-driven
    // termination this returns the terminated exit status, which we treat as
    // success-equivalent for verdict-based result classification below.
    let status = child.wait().await.map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to wait for acceptance command for change '{}' in workspace '{}': {}",
            change_id,
            workspace_path.display(),
            e
        ))
    })?;

    // When the runtime terminated the child after a canonical verdict, the
    // terminated exit code is not a real failure — the acceptance result is
    // already determined by the verdict. Skip the command-failure branch in
    // that case so the verdict drives the final result.
    let verdict_finalized_run = early_terminated && verdict_detected;

    let end_revision = resolve_acceptance_state_revision(
        &start_revision,
        crate::vcs::git::commands::get_current_commit(workspace_path)
            .await
            .ok(),
    );
    if end_revision != start_revision {
        warn!(
            module = module_path!(),
            change_id = %change_id,
            start_revision = %start_revision,
            end_revision = %end_revision,
            workspace = %workspace_path.display(),
            "Acceptance updated HEAD during execution; durable acceptance state will use end revision"
        );
    }

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Parse acceptance output
    let parse_result = parse_acceptance_output(&full_stdout);
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // Check if command failed. A verdict-finalized run was terminated by us
    // after the canonical verdict, so the non-zero exit is expected and the
    // verdict drives the final result.
    if !status.success() && !verdict_finalized_run {
        let error_msg = format!(
            "Acceptance command failed with exit code: {:?}",
            status.code()
        );
        let attempt_number = agent.next_acceptance_attempt_number(change_id);
        let attempt = crate::history::AcceptanceAttempt {
            attempt: attempt_number,
            passed: false,
            duration: start_time.elapsed(),
            findings: Some(tail_findings.clone()),
            exit_code: status.code(),
            stdout_tail: stdout_tail.clone(),
            stderr_tail: stderr_tail.clone(),
            commit_hash: revision_to_history_commit_hash(&end_revision),
        };
        // Record to both agent history (local) and shared acceptance history
        agent.record_acceptance_attempt(change_id, attempt.clone());
        acceptance_history.lock().await.record(change_id, attempt);
        write_acceptance_report(
            workspace_path,
            change_id,
            attempt_number,
            &end_revision,
            stdout_tail.as_deref(),
            stderr_tail.as_deref(),
        )?;
        // Reset acceptance tail injection flag so next apply can receive new output

        acceptance_tail_injected.lock().await.remove(change_id);

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::Log(
                    crate::events::LogEntry::error(&error_msg)
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                ))
                .await;
            let _ = tx
                .send(ParallelEvent::AcceptanceCompleted {
                    change_id: change_id.to_string(),
                })
                .await;
        }

        return Ok((
            crate::orchestration::AcceptanceResult::CommandFailed {
                error: error_msg,
                findings: tail_findings,
            },
            attempt_number,
        ));
    }

    // Process parsed result
    match parse_result {
        ParseResult::Pass => {
            info!("Acceptance passed for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: true,
                duration: start_time.elapsed(),
                findings: None,
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            write_acceptance_report(
                workspace_path,
                change_id,
                attempt_number,
                &end_revision,
                stdout_tail.as_deref(),
                stderr_tail.as_deref(),
            )?;
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::info("Acceptance test passed")
                            .with_change_id(change_id)
                            .with_operation("acceptance")
                            .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((crate::orchestration::AcceptanceResult::Pass, attempt_number))
        }
        ParseResult::Continue => {
            info!("Acceptance requires continuation for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(vec!["Investigation incomplete - continue later".to_string()]),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::info("Acceptance test requires continuation")
                            .with_change_id(change_id)
                            .with_operation("acceptance")
                            .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Continue,
                attempt_number,
            ))
        }
        ParseResult::Gated => {
            info!("Acceptance gated for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(vec!["Implementation blocker detected".to_string()]),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(
                            "Acceptance gated - implementation blocker detected",
                        )
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Gated,
                attempt_number,
            ))
        }
        ParseResult::Fail { findings } => {
            let findings_for_tasks = if findings.is_empty() {
                tail_findings.clone()
            } else {
                findings
            };
            let blocking_gate_context = findings_for_tasks
                .first()
                .cloned()
                .unwrap_or_else(|| "no acceptance findings captured".to_string());
            info!(
                "Acceptance failed for: {} ({} findings), blocking gate context: {}",
                change_id,
                findings_for_tasks.len(),
                blocking_gate_context
            );
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(findings_for_tasks.clone()),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format_acceptance_failure_log_message(
                            &findings_for_tasks,
                        ))
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Fail {
                    findings: findings_for_tasks,
                },
                attempt_number,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_acceptance_failure_log_message, resolve_acceptance_state_revision,
        run_post_apply_cleanup_review,
    };
    use crate::ai_command_runner::AiCommandRunner;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::default_retry_patterns;
    use crate::config::OrchestratorConfig;
    use crate::task_parser::TaskProgress;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::process::Command;
    use tokio::sync::Mutex;

    #[test]
    fn test_progress_commit_message_format() {
        // Verify the commit message format matches the spec
        let change_id = "add-feature";
        let progress = TaskProgress {
            completed: 5,
            total: 10,
        };

        let expected = "WIP: add-feature (5/10 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_all_complete() {
        let change_id = "fix-bug";
        let progress = TaskProgress {
            completed: 7,
            total: 7,
        };

        let expected = "WIP: fix-bug (7/7 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_zero_progress() {
        let change_id = "new-change";
        let progress = TaskProgress {
            completed: 0,
            total: 5,
        };

        let expected = "WIP: new-change (0/5 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_special_characters() {
        // Test with change IDs that contain hyphens (common case)
        let change_id = "add-web-monitoring-feature";
        let progress = TaskProgress {
            completed: 50,
            total: 70,
        };

        let expected = "WIP: add-web-monitoring-feature (50/70 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_acceptance_failure_log_message_includes_blocking_gate_context() {
        let message = format_acceptance_failure_log_message(&[
            "archive-readiness gate failed: cargo clippy -- -D warnings (src/lib.rs:42)"
                .to_string(),
            "second finding".to_string(),
        ]);

        assert!(
            message.contains("Acceptance failed (2 findings), blocking gate context:"),
            "message should include finding count and blocking gate context prefix"
        );
        assert!(
            message.contains("cargo clippy -- -D warnings"),
            "message should preserve gate-specific failure context"
        );
    }

    #[test]
    fn test_format_acceptance_failure_log_message_handles_empty_findings() {
        let message = format_acceptance_failure_log_message(&[]);

        assert_eq!(
            message,
            "Acceptance failed (0 findings), blocking gate context: no acceptance findings captured"
        );
    }

    #[test]
    fn test_resolve_acceptance_state_revision_prefers_end_revision() {
        let resolved = resolve_acceptance_state_revision("start-rev", Some("end-rev".to_string()));
        assert_eq!(resolved, "end-rev");
    }

    #[test]
    fn test_resolve_acceptance_state_revision_falls_back_to_start_revision() {
        let resolved = resolve_acceptance_state_revision("start-rev", None);
        assert_eq!(resolved, "start-rev");
    }

    #[test]
    fn test_progress_check_condition() {
        // Test the condition for creating progress commits:
        // new_progress.completed > progress.completed
        let old_progress = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_progress_same = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_progress_increased = TaskProgress {
            completed: 5,
            total: 10,
        };
        let new_progress_decreased = TaskProgress {
            completed: 2,
            total: 10,
        };

        // Should NOT create commit when no progress
        assert!(new_progress_same.completed <= old_progress.completed);

        // Should create commit when progress increased
        assert!(new_progress_increased.completed > old_progress.completed);

        // Should NOT create commit when progress decreased (edge case)
        assert!(new_progress_decreased.completed <= old_progress.completed);
    }

    async fn init_test_git_repo(repo_root: &std::path::Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base\n").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
    }

    fn test_ai_runner() -> AiCommandRunner {
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: default_retry_patterns(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let shared_stagger_state = Arc::new(Mutex::new(None));
        AiCommandRunner::new(queue_config, shared_stagger_state)
    }

    #[tokio::test]
    async fn test_post_apply_cleanup_review_succeeds_with_single_clean_marker() {
        let temp_dir = TempDir::new().unwrap();
        init_test_git_repo(temp_dir.path()).await;
        std::fs::write(temp_dir.path().join("dirty.txt"), "dirty\n").unwrap();

        let config = OrchestratorConfig {
            acceptance_command: Some(
                "sh -c 'git add dirty.txt && git commit -m cleanup && echo CLEANUP_REVIEW: CLEAN'"
                    .to_string(),
            ),
            ..Default::default()
        };
        let ai_runner = test_ai_runner();

        run_post_apply_cleanup_review("change-a", temp_dir.path(), &config, &ai_runner)
            .await
            .expect("cleanup review should succeed");

        let (is_dirty, status) =
            crate::vcs::git::commands::has_uncommitted_changes(temp_dir.path())
                .await
                .unwrap();
        assert!(
            !is_dirty,
            "worktree must be clean after successful cleanup review: {status}"
        );
    }

    #[tokio::test]
    async fn test_post_apply_cleanup_review_fails_when_marker_missing() {
        let temp_dir = TempDir::new().unwrap();
        init_test_git_repo(temp_dir.path()).await;
        std::fs::write(temp_dir.path().join("dirty.txt"), "dirty\n").unwrap();

        let config = OrchestratorConfig {
            acceptance_command: Some(
                "sh -c 'git add dirty.txt && git commit -m cleanup && echo done'".to_string(),
            ),
            ..Default::default()
        };
        let ai_runner = test_ai_runner();

        let err = run_post_apply_cleanup_review("change-a", temp_dir.path(), &config, &ai_runner)
            .await
            .expect_err("cleanup review must fail without marker");
        assert!(
            err.to_string().contains("CLEANUP_REVIEW: CLEAN"),
            "error should mention missing marker: {err}"
        );
    }
}
