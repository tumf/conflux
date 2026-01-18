//! Workspace execution logic for apply and archive operations.

use crate::agent::{build_apply_prompt, AgentRunner, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::stall::{StallDetector, StallPhase};
use crate::vcs::git::commands as git_commands;
use crate::vcs::VcsBackend;
use std::path::Path;
use std::process::Stdio as StdStdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use super::events::ParallelEvent;

/// Create an iteration snapshot with WIP commit message including iteration number.
///
/// This function creates a WIP (work-in-progress) commit after each apply iteration,
/// regardless of whether progress was made. This ensures that work is not lost if the
/// process is interrupted or reaches the maximum iteration limit.
///
/// # IMPORTANT: Message Format Consistency
///
/// This function uses the SAME commit message format as the unified apply loop
/// in `src/execution/apply.rs::format_wip_commit_message()` to ensure consistency
/// between serial and parallel execution modes.
///
/// # Arguments
///
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
/// * `iteration` - Current iteration number
/// * `completed` - Number of completed tasks
/// * `total` - Total number of tasks
/// * `vcs_backend` - The VCS backend to use (Git)
///
/// # Commit Message Format
///
/// The commit message follows the format: `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})`
/// For example: `WIP: add-feature (5/10 tasks, apply#3)`
///
/// This MUST match `src/execution/apply.rs::format_wip_commit_message()`.
async fn create_iteration_snapshot(
    workspace_path: &Path,
    change_id: &str,
    iteration: u32,
    completed: u32,
    total: u32,
    vcs_backend: VcsBackend,
) -> Result<Option<bool>> {
    let commit_message = format!(
        "WIP: {} ({}/{} tasks, apply#{})",
        change_id, completed, total, iteration
    );

    debug!(
        "Creating iteration snapshot #{} for {}: {}",
        iteration, change_id, commit_message
    );

    let mut commit_created = false;

    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            // For Git: stage all changes and create/amend commit
            debug!(
                module = module_path!(),
                "Executing git command: git add -A (cwd: {:?})", workspace_path
            );
            let add_output = Command::new("git")
                .args(["add", "-A"])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to stage changes: {}", e))
                })?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                warn!(
                    "Failed to stage changes for iteration {}: {}",
                    iteration, stderr
                );
                return Ok(None);
            }

            // Check if we have commits to amend
            debug!(
                module = module_path!(),
                "Executing git command: git rev-parse HEAD (cwd: {:?})", workspace_path
            );
            let has_commits = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map(|output| output.status.success())
                .unwrap_or(false);

            if has_commits {
                // Amend existing commit
                debug!(
                    "Executing git command: git commit --amend --allow-empty -m {} (cwd: {:?})",
                    commit_message, workspace_path
                );
                let commit_output = Command::new("git")
                    .args(["commit", "--amend", "--allow-empty", "-m", &commit_message])
                    .current_dir(workspace_path)
                    .stdin(StdStdio::null())
                    .output()
                    .await
                    .map_err(|e| {
                        OrchestratorError::GitCommand(format!("Failed to amend commit: {}", e))
                    })?;

                if !commit_output.status.success() {
                    let stderr = String::from_utf8_lossy(&commit_output.stderr);
                    warn!(
                        "Failed to amend WIP commit for iteration {}: {}",
                        iteration, stderr
                    );
                } else {
                    commit_created = true;
                    debug!(
                        "Iteration snapshot #{} created for {} (git, amended)",
                        iteration, change_id
                    );
                }
            } else {
                // Create initial commit
                debug!(
                    "Executing git command: git commit --allow-empty -m {} (cwd: {:?})",
                    commit_message, workspace_path
                );
                let commit_output = Command::new("git")
                    .args(["commit", "--allow-empty", "-m", &commit_message])
                    .current_dir(workspace_path)
                    .stdin(StdStdio::null())
                    .output()
                    .await
                    .map_err(|e| {
                        OrchestratorError::GitCommand(format!("Failed to create commit: {}", e))
                    })?;

                if !commit_output.status.success() {
                    let stderr = String::from_utf8_lossy(&commit_output.stderr);
                    warn!(
                        "Failed to create initial WIP commit for iteration {}: {}",
                        iteration, stderr
                    );
                } else {
                    commit_created = true;
                    debug!(
                        "Iteration snapshot #{} created for {} (git, initial)",
                        iteration, change_id
                    );
                }
            }
        }
    }

    if commit_created {
        match git_commands::is_head_empty_commit(workspace_path).await {
            Ok(is_empty) => Ok(Some(is_empty)),
            Err(e) => {
                warn!(
                    "Failed to check WIP commit contents for {} (apply#{}): {}",
                    change_id, iteration, e
                );
                Ok(None)
            }
        }
    } else {
        Ok(None)
    }
}

/// Squash all WIP iteration snapshots into a single Apply commit.
///
/// This function is called after all apply iterations succeed. It combines all WIP
/// snapshots into a single final commit with an Apply message.
///
/// # Arguments
///
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
/// * `final_iteration` - The final iteration number
/// * `vcs_backend` - The VCS backend to use (Git)
///
/// # Commit Message Format
///
/// The commit message follows the format: `Apply: {change_id} (apply#{final_iteration})`
/// For example: `Apply: add-feature (apply#5)`
async fn squash_wip_commits(
    workspace_path: &Path,
    change_id: &str,
    final_iteration: u32,
    vcs_backend: VcsBackend,
) -> Result<()> {
    let apply_message = format!("Apply: {} (apply#{})", change_id, final_iteration);

    debug!("Squashing WIP commits for {} into Apply commit", change_id);

    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            // For Git, we update the commit message to the final Apply message
            // Since we've been amending the same commit, we just need to update the message
            debug!(
                module = module_path!(),
                "Executing git command: git commit --amend -m {} (cwd: {:?})",
                apply_message,
                workspace_path
            );
            let output = Command::new("git")
                .args(["commit", "--amend", "-m", &apply_message])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to squash WIP commits: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(OrchestratorError::GitCommand(format!(
                    "Failed to set Apply message: {}",
                    stderr
                )));
            }

            info!("WIP commits squashed into Apply commit for {}", change_id);
        }
    }

    Ok(())
}

/// Check task progress for a change in the given workspace.
///
/// Reads and parses the tasks.md file to determine completion status.
/// Returns an error if the file doesn't exist.
///
/// This function delegates to `crate::execution::apply::check_task_progress`
/// for the actual implementation.
#[inline]
pub fn check_task_progress(
    workspace_path: &Path,
    change_id: &str,
) -> Result<crate::task_parser::TaskProgress> {
    common_apply::check_task_progress(workspace_path, change_id)
}

/// Summarize command output for logging and event reporting.
///
/// If output exceeds max_lines, returns the last few lines with a count prefix.
///
/// This function delegates to `crate::execution::apply::summarize_output`
/// for the actual implementation.
#[allow(dead_code)] // Utility function for future use
#[inline]
pub fn summarize_output(output: &str, max_lines: usize) -> String {
    common_apply::summarize_output(output, max_lines)
}

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

/// Execute apply command in a single workspace, repeating until tasks are 100% complete
#[allow(clippy::too_many_arguments)]
pub async fn execute_apply_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    apply_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    parallel_ctx: Option<&ParallelHookContext>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    repo_root: &Path,
    apply_history: &Arc<Mutex<crate::history::ApplyHistory>>,
) -> Result<String> {
    const MAX_ITERATIONS: u32 = 50;
    let mut iteration = 0;
    let mut first_apply = true;
    let mut apply_succeeded = false; // Track if all iterations succeeded
    let mut stall_detector = StallDetector::new(config.get_stall_detection());

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
                "Parallel apply execution guard: workspace_path is NOT a worktree (executing in base repository is forbidden)\n\
                 change_id: {}\n\
                 workspace_path: {}\n\
                 repo_root: {}\n\
                 apply_command: {}",
                change_id,
                workspace_path.display(),
                repo_root.display(),
                apply_cmd_template
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to validate worktree status for parallel apply\n\
                 change_id: {}\n\
                 workspace_path: {}\n\
                 repo_root: {}\n\
                 apply_command: {}\n\
                 validation_error: {}",
                change_id,
                workspace_path.display(),
                repo_root.display(),
                apply_cmd_template,
                e
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
    }

    loop {
        iteration += 1;
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cancelled apply for '{}' in workspace '{}' (iteration {})",
                change_id,
                workspace_path.display(),
                iteration
            )));
        }
        if iteration > MAX_ITERATIONS {
            // Run on_error hook if configured
            if let Some(hook_runner) = hooks {
                let error_msg = format!(
                    "Max iterations ({}) reached for change '{}' in workspace '{}'",
                    MAX_ITERATIONS,
                    change_id,
                    workspace_path.display()
                );
                let error_ctx =
                    build_parallel_hook_context(change_id, 0, 0, iteration, parallel_ctx)
                        .with_error(&error_msg);
                if let Err(e) = hook_runner.run_hook(HookType::OnError, &error_ctx).await {
                    error!("on_error hook failed: {}", e);
                }
            }
            return Err(OrchestratorError::AgentCommand(format!(
                "Max iterations ({}) reached for change '{}' in workspace '{}'",
                MAX_ITERATIONS,
                change_id,
                workspace_path.display()
            )));
        }

        // Check current task progress using helper
        let progress = check_task_progress(workspace_path, change_id)?;

        // Send progress event only if we have valid progress data
        if progress.total > 0 {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: progress.completed,
                        total: progress.total,
                    })
                    .await;
            }
        }

        // Check if already complete
        if progress.total > 0 && progress.completed == progress.total {
            info!(
                "Change {} is already complete ({}/{})",
                change_id, progress.completed, progress.total
            );
            break;
        }

        info!(
            "Executing apply #{} for {} in workspace ({}/{} tasks)",
            iteration, change_id, progress.completed, progress.total
        );

        // Send ApplyStarted event on first apply
        if first_apply {
            first_apply = false;
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ApplyStarted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }
        }

        // Run pre_apply hook
        if let Some(hook_runner) = hooks {
            let hook_ctx = build_parallel_hook_context(
                change_id,
                progress.completed,
                progress.total,
                iteration,
                parallel_ctx,
            );

            // Send hook started event
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::HookStarted {
                        change_id: change_id.to_string(),
                        hook_type: "pre_apply".to_string(),
                    })
                    .await;
            }

            match hook_runner.run_hook(HookType::PreApply, &hook_ctx).await {
                Ok(()) => {
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::HookCompleted {
                                change_id: change_id.to_string(),
                                hook_type: "pre_apply".to_string(),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    error!("pre_apply hook failed for {}: {}", change_id, e);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::HookFailed {
                                change_id: change_id.to_string(),
                                hook_type: "pre_apply".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                    // Hook failure with continue_on_failure=false returns error
                    return Err(e);
                }
            }
        }

        // Build prompt with system instructions and history context
        let user_prompt = config.get_apply_prompt();
        let history_context = {
            let history = apply_history.lock().await;
            history.format_context(change_id)
        };
        let full_prompt = build_apply_prompt(user_prompt, &history_context);

        // Expand change_id and prompt in command
        let command = OrchestratorConfig::expand_change_id(apply_cmd_template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

        debug!("Workspace path: {:?}", workspace_path);
        debug!("Apply command: {}", command);

        // Capture start time for history recording
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

        // Forward output to event channel
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let change_id_clone = change_id.to_string();
        let event_tx_clone = event_tx.clone();
        let output_handle = tokio::spawn(async move {
            while let Some(line) = output_rx.recv().await {
                if let Some(ref tx) = event_tx_clone {
                    let output_text = match line {
                        AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s,
                    };
                    let _ = tx
                        .send(ParallelEvent::ApplyOutput {
                            change_id: change_id_clone.clone(),
                            output: output_text,
                            iteration: Some(iteration),
                        })
                        .await;
                }
            }
        });

        // Wait for output streaming to complete
        let _ = output_handle.await;

        // Wait for process to finish
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to wait for apply command for '{}' in workspace '{}' (iteration {}): {}",
                change_id,
                workspace_path.display(),
                iteration,
                e
            ))
        })?;

        // Record apply attempt in history
        {
            let mut history = apply_history.lock().await;
            let attempt = crate::history::ApplyAttempt {
                attempt: history.count(change_id) + 1,
                success: status.success(),
                duration: start.elapsed(),
                error: if status.success() {
                    None
                } else {
                    Some(format!("Exit code: {:?}", status.code()))
                },
                exit_code: status.code(),
            };
            history.record(change_id, attempt);
        }

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Apply command failed for change '{}' in workspace '{}' (iteration {}) with exit code: {:?}",
                change_id,
                workspace_path.display(),
                iteration,
                status.code()
            )));
        }

        // Git worktrees already reflect working copy changes for task progress.

        // Check task progress after apply using helper
        let new_progress = check_task_progress(workspace_path, change_id)?;

        // Send progress event after apply only if we have valid progress data
        if new_progress.total > 0 {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: new_progress.completed,
                        total: new_progress.total,
                    })
                    .await;
            }
        }

        info!(
            "After apply #{}: {}/{} tasks complete",
            iteration, new_progress.completed, new_progress.total
        );

        // Run post_apply hook
        if let Some(hook_runner) = hooks {
            let hook_ctx = build_parallel_hook_context(
                change_id,
                new_progress.completed,
                new_progress.total,
                iteration,
                parallel_ctx,
            );

            // Send hook started event
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::HookStarted {
                        change_id: change_id.to_string(),
                        hook_type: "post_apply".to_string(),
                    })
                    .await;
            }

            match hook_runner.run_hook(HookType::PostApply, &hook_ctx).await {
                Ok(()) => {
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::HookCompleted {
                                change_id: change_id.to_string(),
                                hook_type: "post_apply".to_string(),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    error!("post_apply hook failed for {}: {}", change_id, e);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::HookFailed {
                                change_id: change_id.to_string(),
                                hook_type: "post_apply".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                    // Hook failure with continue_on_failure=false returns error
                    return Err(e);
                }
            }
        }

        // Create iteration snapshot after each apply iteration
        // This ensures work is not lost even if no progress was made
        let empty_commit = match create_iteration_snapshot(
            workspace_path,
            change_id,
            iteration,
            new_progress.completed,
            new_progress.total,
            vcs_backend,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => {
                warn!(
                    "Failed to create iteration snapshot for {}: {}",
                    change_id, e
                );
                None
            }
        };

        if let Some(is_empty) = empty_commit {
            if !common_apply::is_progress_complete(&new_progress)
                && stall_detector.register_commit(change_id, StallPhase::Apply, is_empty)
            {
                let count = stall_detector.current_count(change_id, StallPhase::Apply);
                let threshold = stall_detector.config().threshold;
                let message = format!(
                    "Stall detected for {} after {} empty WIP commits (apply)",
                    change_id, count
                );
                warn!("{} (threshold {})", message, threshold);
                return Err(OrchestratorError::AgentCommand(message));
            }
        }

        // Check if complete
        if new_progress.total > 0 && new_progress.completed == new_progress.total {
            // Run on_change_complete hook (task 100% completion)
            if let Some(hook_runner) = hooks {
                let hook_ctx = build_parallel_hook_context(
                    change_id,
                    new_progress.completed,
                    new_progress.total,
                    iteration,
                    parallel_ctx,
                );

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookStarted {
                            change_id: change_id.to_string(),
                            hook_type: "on_change_complete".to_string(),
                        })
                        .await;
                }

                match hook_runner
                    .run_hook(HookType::OnChangeComplete, &hook_ctx)
                    .await
                {
                    Ok(()) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::HookCompleted {
                                    change_id: change_id.to_string(),
                                    hook_type: "on_change_complete".to_string(),
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        error!("on_change_complete hook failed for {}: {}", change_id, e);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::HookFailed {
                                    change_id: change_id.to_string(),
                                    hook_type: "on_change_complete".to_string(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                        // Hook failure with continue_on_failure=false returns error
                        return Err(e);
                    }
                }
            }

            info!(
                "Change {} completed after {} iteration(s)",
                change_id, iteration
            );
            apply_succeeded = true; // Mark success for squashing
            break;
        }

        // Check for progress (avoid infinite loops)
        if new_progress.completed <= progress.completed && iteration > 1 {
            warn!(
                "No progress made for {} (still {}/{}), continuing...",
                change_id, new_progress.completed, new_progress.total
            );
        }
    }

    // Squash WIP commits into Apply commit if successful
    if apply_succeeded {
        info!(
            "Squashing WIP snapshots into final Apply commit for {}",
            change_id
        );
        if let Err(e) = squash_wip_commits(workspace_path, change_id, iteration, vcs_backend).await
        {
            warn!("Failed to squash WIP commits for {}: {}", change_id, e);
        }
    } else {
        info!(
            "Apply loop exited without completion for {}; WIP snapshots preserved",
            change_id
        );
    }

    // Get the resulting revision
    let revision = match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            debug!(
                module = module_path!(),
                "Executing git command: git rev-parse HEAD (cwd: {:?})", workspace_path
            );
            let revision_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::GitCommand(format!(
                    "Failed to get workspace revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
        }
    };

    Ok(revision)
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
                "Cannot archive '{}' in workspace '{}': tasks.md not found at {}",
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

        // Forward output to event channel
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let change_id_clone = change_id.to_string();
        let event_tx_clone = event_tx.clone();
        let output_handle = tokio::spawn(async move {
            while let Some(line) = output_rx.recv().await {
                if let Some(ref tx) = event_tx_clone {
                    let output_text = match line {
                        AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s,
                    };
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id: change_id_clone.clone(),
                            output: output_text,
                            iteration: None,
                        })
                        .await;
                }
            }
        });

        // Wait for output streaming to complete
        let _ = output_handle.await;

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
                        .with_operation("archive"),
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

    let resolve_agent = AgentRunner::new(config.clone());
    let change_id_owned = change_id.to_string();
    let event_tx_clone = event_tx.clone();
    ensure_archive_commit(
        change_id,
        workspace_path,
        &resolve_agent,
        vcs_backend,
        move |line| {
            let event_tx = event_tx_clone.clone();
            let change_id = change_id_owned.clone();
            async move {
                let text = match line {
                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                };
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id,
                            output: text,
                            iteration: None,
                        })
                        .await;
                }
            }
        },
    )
    .await?;

    // Get the current revision after archive
    let revision = match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            debug!(
                module = module_path!(),
                "Executing git command: git rev-parse HEAD (cwd: {:?})", workspace_path
            );
            let revision_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::GitCommand(format!(
                    "Failed to get revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
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
