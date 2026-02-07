//! Workspace execution logic for apply and archive operations.

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::parallel::output_bridge::ParallelApplyEventHandler;

use super::events::ParallelEvent;
use crate::orchestration::build_acceptance_tail_findings;
use crate::stall::StallDetector;
use crate::vcs::git::commands as git_commands;
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
) -> Result<(String, u32)> {
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
    let apply_result = common_apply::execute_apply_loop(
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
    .await?;

    // Return revision and iteration count
    Ok((apply_result.revision, apply_result.iterations))
}

/// Execute archive command in a workspace with streaming output
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
    shared_stagger_state: &crate::ai_command_runner::SharedStaggerState,
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
    let full_prompt = crate::agent::build_archive_prompt(user_prompt, &history_context);

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
        build_archive_error_message, ensure_archive_commit, verify_archive_completion,
        ARCHIVE_COMMAND_MAX_RETRIES,
    };

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
            .execute_streaming_with_retry(&command, Some(workspace_path))
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
            if let Some(ref tx) = event_tx {
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

        return Err(OrchestratorError::AgentCommand(
            build_archive_error_message(change_id, Some(workspace_path)),
        ));
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

    // Clear history after successful archive
    {
        let mut apply_hist = apply_history.lock().await;
        apply_hist.clear(change_id);
        let mut archive_hist = archive_history.lock().await;
        archive_hist.clear(change_id);
    }

    Ok(revision)
}

/// Execute acceptance test in a workspace with streaming output
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
        crate::config::AcceptancePromptMode::Full => crate::agent::build_acceptance_prompt(
            change_id,
            user_prompt,
            &history_context,
            &last_output_context,
            &diff_context,
        ),
        crate::config::AcceptancePromptMode::ContextOnly => {
            crate::agent::build_acceptance_prompt_context_only(
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
        .execute_streaming_with_retry(&command, Some(workspace_path))
        .await?;

    // Create output collector for history
    let mut output_collector = crate::history::OutputCollector::new();
    let mut full_stdout = String::new();

    // Stream output until channel closes
    use crate::ai_command_runner::OutputLine as AiOutputLine;
    while let Some(line) = output_rx.recv().await {
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

                // Forward to event channel
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
            AiOutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);

                // Forward to event channel
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::warn(&s)
                                .with_change_id(change_id)
                                .with_operation("acceptance")
                                .with_iteration(acceptance_iteration),
                        ))
                        .await;
                }
            }
        }
    }

    // Wait for child process to complete
    let status = child.wait().await.map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to wait for acceptance command for change '{}' in workspace '{}': {}",
            change_id,
            workspace_path.display(),
            e
        ))
    })?;

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Parse acceptance output
    let parse_result = parse_acceptance_output(&full_stdout);
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // Check if command failed
    if !status.success() {
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
            commit_hash: commit_hash.clone(),
        };
        // Record to both agent history (local) and shared acceptance history
        agent.record_acceptance_attempt(change_id, attempt.clone());
        acceptance_history.lock().await.record(change_id, attempt);
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
                commit_hash: commit_hash.clone(),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
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
                commit_hash: commit_hash.clone(),
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
        ParseResult::Blocked => {
            info!("Acceptance blocked for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(vec!["Implementation blocker detected".to_string()]),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: commit_hash.clone(),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn("Acceptance blocked - implementation blocker detected")
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
                crate::orchestration::AcceptanceResult::Blocked,
                attempt_number,
            ))
        }
        ParseResult::Fail { .. } => {
            let findings_for_tasks = tail_findings.clone();
            info!(
                "Acceptance failed for: {} ({} tail lines)",
                change_id,
                findings_for_tasks.len()
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
                commit_hash: commit_hash.clone(),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Acceptance test failed ({} tail lines)",
                            findings_for_tasks.len()
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
    use crate::task_parser::TaskProgress;

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
}
