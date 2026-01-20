//! Common apply iteration logic for serial and parallel modes.
//!
//! This module provides shared functionality for executing apply operations,
//! including:
//! - Task progress checking
//! - Progress commit creation
//! - Apply iteration management
//!
//! Both serial and parallel modes use these common functions to ensure
//! consistent behavior across execution modes.

// Allow dead_code as this is a foundation module - types and functions will be used
// incrementally as parallel/executor.rs is refactored to use common functions.
#![allow(dead_code)]

use crate::agent::{AgentRunner, OutputLine};
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::OutputCollector;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::stall::{StallDetector, StallPhase};
use crate::task_parser::TaskProgress;
use crate::vcs::{VcsBackend, VcsResult, WorkspaceManager};
use std::fs;
use std::future::Future;
use std::path::Path;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Default maximum iterations for apply loops.
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

/// Configuration for apply iteration behavior.
#[derive(Debug, Clone)]
pub struct ApplyConfig {
    /// Maximum number of apply iterations before giving up.
    /// Default is 50.
    pub max_iterations: u32,

    /// Whether to create progress commits after each iteration.
    /// Useful for parallel mode where work should be preserved.
    pub progress_commits_enabled: bool,

    /// Whether streaming output is enabled.
    /// Used to determine how to report progress.
    pub streaming_enabled: bool,
}

impl Default for ApplyConfig {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            progress_commits_enabled: true,
            streaming_enabled: false,
        }
    }
}

impl ApplyConfig {
    /// Create a new ApplyConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum iterations.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Enable or disable progress commits.
    pub fn with_progress_commits(mut self, enabled: bool) -> Self {
        self.progress_commits_enabled = enabled;
        self
    }

    /// Enable or disable streaming output.
    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.streaming_enabled = enabled;
        self
    }
}

/// Result of a single apply iteration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyIterationResult {
    /// Tasks are now complete.
    Complete,
    /// Progress was made but not yet complete.
    Progress { completed: u32, total: u32 },
    /// No progress was made in this iteration.
    NoProgress { completed: u32, total: u32 },
    /// Apply command failed.
    Failed { error: String },
}

impl ApplyIterationResult {
    /// Check if the result indicates completion.
    pub fn is_complete(&self) -> bool {
        matches!(self, ApplyIterationResult::Complete)
    }

    /// Check if the result indicates failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, ApplyIterationResult::Failed { .. })
    }
}

/// Check task progress for a change in the given workspace.
///
/// Reads and parses the tasks.md file to determine completion status.
/// Returns an error if the file doesn't exist, with the exact path checked.
///
/// # Arguments
///
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
///
/// # Returns
///
/// * `Ok(TaskProgress)` - Progress information if tasks.md exists
/// * `Err(OrchestratorError)` - If tasks.md doesn't exist
pub fn check_task_progress(workspace_path: &Path, change_id: &str) -> Result<TaskProgress> {
    let change_dir = workspace_path.join("openspec/changes").join(change_id);
    let tasks_path = change_dir.join("tasks.md");

    debug!(
        change_id = change_id,
        workspace_path = %workspace_path.display(),
        tasks_path = %tasks_path.display(),
        "Checking tasks path in workspace"
    );

    if tasks_path.exists() {
        let progress = crate::task_parser::parse_file(&tasks_path, Some(change_id))?;
        debug!(
            "Tasks file found for {}: {}/{} complete",
            change_id, progress.completed, progress.total
        );
        return Ok(progress);
    }

    let archive_root = if change_dir.is_dir() {
        change_dir.join("archive")
    } else {
        workspace_path.join("openspec/changes/archive")
    };
    let archive_root_exists = archive_root.is_dir();
    let latest_archive_dir = if archive_root_exists {
        let mut latest: Option<String> = None;
        for entry in fs::read_dir(&archive_root)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = match name.to_str() {
                Some(value) => value,
                None => continue,
            };
            if !name.ends_with(change_id) {
                continue;
            }
            if latest
                .as_ref()
                .is_none_or(|current| name > current.as_str())
            {
                latest = Some(name.to_string());
            }
        }
        latest
    } else {
        None
    };

    if let Some(latest_dir) = latest_archive_dir {
        let archive_tasks_path = archive_root.join(latest_dir).join("tasks.md");
        if archive_tasks_path.exists() {
            let progress = crate::task_parser::parse_file(&archive_tasks_path, Some(change_id))?;
            debug!(
                "Tasks file found in archive for {}: {}/{} complete",
                change_id, progress.completed, progress.total
            );
            return Ok(progress);
        }
    }

    let change_dir_exists = change_dir.is_dir();
    Err(OrchestratorError::AgentCommand(format!(
        "Tasks file not found; change_id={}; workspace_path=\"{}\"; tasks_path=\"{}\"; change_dir_exists={}; archive_root=\"{}\"; archive_root_exists={}; exists=false",
        change_id,
        workspace_path.display(),
        tasks_path.display(),
        change_dir_exists,
        archive_root.display(),
        archive_root_exists
    )))
}

/// Create a progress commit to save current work state.
///
/// This function creates a WIP (work-in-progress) commit after each apply iteration
/// where progress was made. This ensures that work is not lost if the process is
/// interrupted or reaches the maximum iteration limit.
///
/// # Arguments
///
/// * `workspace_manager` - The workspace manager for VCS operations
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
/// * `progress` - Current task progress (completed/total)
///
/// # Commit Message Format
///
/// The commit message follows the format: `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})`
/// For example: `WIP: add-feature (5/10 tasks, apply#3)`
pub fn format_wip_commit_message(
    change_id: &str,
    progress: &TaskProgress,
    iteration: u32,
) -> String {
    format!(
        "WIP: {} ({}/{} tasks, apply#{})",
        change_id, progress.completed, progress.total, iteration
    )
}

pub async fn create_progress_commit<W: WorkspaceManager + ?Sized>(
    workspace_manager: &W,
    workspace_path: &Path,
    change_id: &str,
    progress: &TaskProgress,
    iteration: u32,
) -> VcsResult<()> {
    let commit_message = format_wip_commit_message(change_id, progress, iteration);

    debug!(
        "Creating progress commit for {}: {}",
        change_id, commit_message
    );

    // Snapshot working copy changes first to capture workspace state.
    workspace_manager
        .snapshot_working_copy(workspace_path)
        .await?;

    workspace_manager
        .create_iteration_snapshot(
            workspace_path,
            change_id,
            iteration,
            progress.completed,
            progress.total,
        )
        .await?;

    debug!(
        "Progress commit created for {} ({})",
        change_id,
        workspace_manager.backend_type()
    );

    Ok(())
}

/// Create a final commit for a completed change.
///
/// This function creates the final commit message after all tasks are complete.
///
/// # Arguments
///
/// * `workspace_manager` - The workspace manager for VCS operations
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
pub async fn create_final_commit<W: WorkspaceManager + ?Sized>(
    workspace_manager: &W,
    workspace_path: &Path,
    change_id: &str,
) -> VcsResult<()> {
    let commit_message = format!("Apply: {}", change_id);

    debug!(
        "Creating final commit for {}: {}",
        change_id, commit_message
    );

    // Snapshot working copy changes first to capture workspace state.
    workspace_manager
        .snapshot_working_copy(workspace_path)
        .await?;

    // Set the commit message
    workspace_manager
        .set_commit_message(workspace_path, &commit_message)
        .await?;

    info!(
        "Final commit created for {} ({})",
        change_id,
        workspace_manager.backend_type()
    );

    Ok(())
}

/// Get the current revision in a workspace.
///
/// # Arguments
///
/// * `workspace_manager` - The workspace manager for VCS operations
/// * `workspace_path` - Path to the workspace directory
///
/// # Returns
///
/// The revision ID as a string.
pub async fn get_workspace_revision<W: WorkspaceManager + ?Sized>(
    workspace_manager: &W,
    workspace_path: &Path,
) -> VcsResult<String> {
    workspace_manager
        .get_revision_in_workspace(workspace_path)
        .await
}

/// Build the full apply prompt with system instructions.
///
/// # Arguments
///
/// * `config` - The orchestrator configuration
/// * `history` - Optional apply history for context
///
/// # Returns
///
/// The full prompt string to use for the apply command.
pub fn build_apply_prompt(config: &OrchestratorConfig, history: &str) -> String {
    let user_prompt = config.get_apply_prompt();
    crate::agent::build_apply_prompt(user_prompt, history)
}

/// Expand the apply command template with change_id and prompt.
///
/// # Arguments
///
/// * `template` - The command template
/// * `change_id` - The change identifier
/// * `prompt` - The full prompt to insert
///
/// # Returns
///
/// The expanded command string.
pub fn expand_apply_command(template: &str, change_id: &str, prompt: &str) -> String {
    let command = OrchestratorConfig::expand_change_id(template, change_id);
    OrchestratorConfig::expand_prompt(&command, prompt)
}

/// Check if task progress indicates completion.
///
/// # Arguments
///
/// * `progress` - The task progress to check
///
/// # Returns
///
/// `true` if all tasks are complete, `false` otherwise.
pub fn is_progress_complete(progress: &TaskProgress) -> bool {
    progress.total > 0 && progress.completed >= progress.total
}

/// Check if progress was made between two progress states.
///
/// # Arguments
///
/// * `old` - Previous progress state
/// * `new` - Current progress state
///
/// # Returns
///
/// `true` if completed count increased, `false` otherwise.
pub fn progress_increased(old: &TaskProgress, new: &TaskProgress) -> bool {
    new.completed > old.completed
}

/// Summarize command output for logging and event reporting.
///
/// If output exceeds max_lines, returns the last few lines with a count prefix.
///
/// # Arguments
///
/// * `output` - The output string to summarize
/// * `max_lines` - Maximum lines to show before summarizing
///
/// # Returns
///
/// The summarized output string.
pub fn summarize_output(output: &str, max_lines: usize) -> String {
    if output.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = output.lines().collect();
    if lines.len() > max_lines {
        // Show last 5 lines with total count
        let tail_lines = 5.min(lines.len());
        format!(
            "... ({} lines) ...\n{}",
            lines.len(),
            lines[lines.len() - tail_lines..].join("\n")
        )
    } else {
        output.to_string()
    }
}

/// Event handler for apply loop events.
///
/// This trait allows the apply loop to send events to different handlers
/// (e.g., TUI event channel, CLI logger, parallel event bus).
pub trait ApplyEventHandler {
    /// Called when apply iteration starts
    fn on_apply_started(&self, change_id: &str);
    /// Called when progress is updated
    fn on_progress_updated(&self, change_id: &str, completed: u32, total: u32);
    /// Called when hook starts
    fn on_hook_started(&self, change_id: &str, hook_type: &str);
    /// Called when hook completes
    fn on_hook_completed(&self, change_id: &str, hook_type: &str);
    /// Called when hook fails
    fn on_hook_failed(&self, change_id: &str, hook_type: &str, error: &str);
    /// Called when apply output is generated
    fn on_apply_output(&self, change_id: &str, line: &OutputLine);
}

/// No-op event handler for cases where events are not needed
pub struct NoOpEventHandler;

impl ApplyEventHandler for NoOpEventHandler {
    fn on_apply_started(&self, _change_id: &str) {}
    fn on_progress_updated(&self, _change_id: &str, _completed: u32, _total: u32) {}
    fn on_hook_started(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_completed(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_failed(&self, _change_id: &str, _hook_type: &str, _error: &str) {}
    fn on_apply_output(&self, _change_id: &str, _line: &OutputLine) {}
}

/// Context for building hook contexts in the apply loop
pub struct ApplyLoopHookContext {
    /// Changes processed so far
    pub changes_processed: usize,
    /// Total changes in this run
    pub total_changes: usize,
    /// Remaining changes
    pub remaining_changes: usize,
    /// Workspace path for parallel mode (optional)
    pub workspace_path: Option<String>,
    /// Group index for parallel mode (optional)
    pub group_index: Option<usize>,
}

impl ApplyLoopHookContext {
    /// Create a new hook context for serial mode
    pub fn serial(
        changes_processed: usize,
        total_changes: usize,
        remaining_changes: usize,
    ) -> Self {
        Self {
            changes_processed,
            total_changes,
            remaining_changes,
            workspace_path: None,
            group_index: None,
        }
    }

    /// Create a new hook context for parallel mode
    pub fn parallel(
        changes_processed: usize,
        total_changes: usize,
        remaining_changes: usize,
        workspace_path: String,
        group_index: usize,
    ) -> Self {
        Self {
            changes_processed,
            total_changes,
            remaining_changes,
            workspace_path: Some(workspace_path),
            group_index: Some(group_index),
        }
    }

    /// Build a HookContext from this apply loop context
    fn build_hook_context(
        &self,
        change_id: &str,
        completed: u32,
        total: u32,
        apply_count: u32,
    ) -> HookContext {
        let mut ctx = HookContext::new(
            self.changes_processed,
            self.total_changes,
            self.remaining_changes,
            false,
        )
        .with_change(change_id, completed, total)
        .with_apply_count(apply_count);

        if let Some(ref workspace_path) = self.workspace_path {
            if let Some(group_index) = self.group_index {
                ctx = ctx.with_parallel_context(workspace_path, Some(group_index as u32));
            }
        }

        ctx
    }
}

/// Result of the unified apply loop
#[derive(Debug)]
pub struct ApplyLoopResult {
    /// Final revision ID (e.g., git commit hash)
    pub revision: String,
    /// Whether all tasks were completed
    pub completed: bool,
    /// Number of iterations executed
    pub iterations: u32,
}

/// Execute apply iterations until tasks are complete or max iterations reached.
///
/// This is the unified apply loop used by both serial and parallel modes.
///
/// # Arguments
///
/// * `change_id` - The change to apply
/// * `workspace_path` - Working directory (worktree for parallel, repo root for serial)
/// * `config` - Orchestrator configuration
/// * `agent` - Agent runner for executing commands
/// * `vcs_backend` - VCS backend (Git, Auto, etc.)
/// * `hooks` - Optional hook runner
/// * `hook_ctx` - Context for building hook contexts
/// * `event_handler` - Event handler for sending progress/hook events
/// * `cancel_token` - Optional cancellation token
///
/// # Returns
///
/// * `Ok(ApplyLoopResult)` - Apply loop completed (success or max iterations)
/// * `Err(e)` - An error occurred (hook failure, command spawn failure, etc.)
#[allow(clippy::too_many_arguments)]
pub async fn execute_apply_loop<E, F, Fut>(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    agent: &mut AgentRunner,
    vcs_backend: VcsBackend,
    workspace_manager: Option<&dyn WorkspaceManager>,
    hooks: Option<&HookRunner>,
    hook_ctx: &ApplyLoopHookContext,
    event_handler: &E,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    mut output_handler: F,
) -> Result<ApplyLoopResult>
where
    E: ApplyEventHandler,
    F: FnMut(OutputLine) -> Fut,
    Fut: Future<Output = ()>,
{
    let max_iterations = config.get_max_iterations();
    let mut iteration = 0;
    let mut first_apply = true;
    let mut stall_detector = StallDetector::new(config.get_stall_detection());

    // Check if VCS is Git for WIP/stall features
    let is_git = matches!(vcs_backend, VcsBackend::Git);

    let apply_succeeded = loop {
        iteration += 1;

        // Check cancellation
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cancelled apply for '{}' in workspace '{}'",
                change_id,
                workspace_path.display()
            )));
        }

        // Check max iterations
        if iteration > max_iterations {
            let error_msg = format!(
                "Max iterations ({}) reached for change '{}' in workspace '{}'",
                max_iterations,
                change_id,
                workspace_path.display()
            );

            // Run on_error hook
            if let Some(hook_runner) = hooks {
                let progress = check_task_progress(workspace_path, change_id)
                    .unwrap_or_else(|_| TaskProgress::default());
                let error_ctx = hook_ctx
                    .build_hook_context(change_id, progress.completed, progress.total, iteration)
                    .with_error(&error_msg);
                if let Err(e) = hook_runner.run_hook(HookType::OnError, &error_ctx).await {
                    error!("on_error hook failed: {}", e);
                }
            }

            return Err(OrchestratorError::AgentCommand(error_msg));
        }

        // Check current task progress
        let progress = check_task_progress(workspace_path, change_id)?;

        // Send progress event
        if progress.total > 0 {
            event_handler.on_progress_updated(change_id, progress.completed, progress.total);
        }

        // Check if already complete
        if is_progress_complete(&progress) {
            info!(
                "Change {} is already complete ({}/{})",
                change_id, progress.completed, progress.total
            );
            break true;
        }

        info!(
            "Executing apply #{} for {} ({}/{} tasks)",
            iteration, change_id, progress.completed, progress.total
        );

        // Send ApplyStarted event on first iteration
        if first_apply {
            first_apply = false;
            event_handler.on_apply_started(change_id);
        }

        // Run pre_apply hook
        if let Some(hook_runner) = hooks {
            let current_hook_ctx = hook_ctx.build_hook_context(
                change_id,
                progress.completed,
                progress.total,
                iteration,
            );

            event_handler.on_hook_started(change_id, "pre_apply");

            match hook_runner
                .run_hook(HookType::PreApply, &current_hook_ctx)
                .await
            {
                Ok(()) => {
                    event_handler.on_hook_completed(change_id, "pre_apply");
                }
                Err(e) => {
                    error!("pre_apply hook failed for {}: {}", change_id, e);
                    event_handler.on_hook_failed(change_id, "pre_apply", &e.to_string());
                    return Err(e);
                }
            }
        }

        // Execute apply command with history context via AiCommandRunner
        let (mut child, mut rx, start_time) = agent
            .run_apply_streaming_with_runner(change_id, ai_runner, Some(workspace_path))
            .await?;

        // Create output collector for history
        let mut output_collector = OutputCollector::new();

        // Stream output
        while let Some(line) = rx.recv().await {
            // Collect output for history
            match &line {
                OutputLine::Stdout(s) => output_collector.add_stdout(s),
                OutputLine::Stderr(s) => output_collector.add_stderr(s),
            }
            event_handler.on_apply_output(change_id, &line);
            output_handler(line).await;
        }

        // Wait for child process
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to wait for apply command for '{}' in workspace '{}' (iteration {}): {}",
                change_id,
                workspace_path.display(),
                iteration,
                e
            ))
        })?;

        // Record apply attempt for history
        agent.record_apply_attempt(
            change_id,
            &status,
            start_time,
            output_collector.stdout_tail(),
            output_collector.stderr_tail(),
        );

        if !status.success() {
            let error_msg = format!("Apply command failed with exit code: {:?}", status.code());

            // Run on_error hook
            if let Some(hook_runner) = hooks {
                let error_ctx = hook_ctx
                    .build_hook_context(change_id, progress.completed, progress.total, iteration)
                    .with_error(&error_msg);
                let _ = hook_runner.run_hook(HookType::OnError, &error_ctx).await;
            }

            return Err(OrchestratorError::AgentCommand(error_msg));
        }

        // Check task progress after apply
        let new_progress = check_task_progress(workspace_path, change_id)?;

        // Send progress event after apply
        if new_progress.total > 0 {
            event_handler.on_progress_updated(
                change_id,
                new_progress.completed,
                new_progress.total,
            );
        }

        info!(
            "After apply #{}: {}/{} tasks complete",
            iteration, new_progress.completed, new_progress.total
        );

        // Run post_apply hook
        if let Some(hook_runner) = hooks {
            let current_hook_ctx = hook_ctx.build_hook_context(
                change_id,
                new_progress.completed,
                new_progress.total,
                iteration,
            );

            event_handler.on_hook_started(change_id, "post_apply");

            match hook_runner
                .run_hook(HookType::PostApply, &current_hook_ctx)
                .await
            {
                Ok(()) => {
                    event_handler.on_hook_completed(change_id, "post_apply");
                }
                Err(e) => {
                    error!("post_apply hook failed for {}: {}", change_id, e);
                    event_handler.on_hook_failed(change_id, "post_apply", &e.to_string());
                    return Err(e);
                }
            }
        }

        // Create iteration snapshot (Git-only)
        if is_git {
            if let Some(ws_mgr) = workspace_manager {
                match create_progress_commit(
                    ws_mgr,
                    workspace_path,
                    change_id,
                    &new_progress,
                    iteration,
                )
                .await
                {
                    Ok(()) => {
                        // Check for stall (Git-only)
                        if let Ok(is_empty) =
                            crate::vcs::git::commands::is_head_empty_commit(workspace_path).await
                        {
                            if !is_progress_complete(&new_progress)
                                && stall_detector.register_commit(
                                    change_id,
                                    StallPhase::Apply,
                                    is_empty,
                                )
                            {
                                let count =
                                    stall_detector.current_count(change_id, StallPhase::Apply);
                                let threshold = stall_detector.config().threshold;
                                let message = format!(
                                    "Stall detected for {} after {} empty WIP commits (apply)",
                                    change_id, count
                                );
                                warn!("{} (threshold {})", message, threshold);
                                return Err(OrchestratorError::AgentCommand(message));
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to create iteration snapshot for {}: {}",
                            change_id, e
                        );
                    }
                }
            }
        } else {
            debug!("Skipping WIP snapshot for {} (non-Git backend)", change_id);
        }

        // Check if complete
        if is_progress_complete(&new_progress) {
            // Run on_change_complete hook
            if let Some(hook_runner) = hooks {
                let current_hook_ctx = hook_ctx.build_hook_context(
                    change_id,
                    new_progress.completed,
                    new_progress.total,
                    iteration,
                );

                event_handler.on_hook_started(change_id, "on_change_complete");

                match hook_runner
                    .run_hook(HookType::OnChangeComplete, &current_hook_ctx)
                    .await
                {
                    Ok(()) => {
                        event_handler.on_hook_completed(change_id, "on_change_complete");
                    }
                    Err(e) => {
                        error!("on_change_complete hook failed for {}: {}", change_id, e);
                        event_handler.on_hook_failed(
                            change_id,
                            "on_change_complete",
                            &e.to_string(),
                        );
                        return Err(e);
                    }
                }
            }

            info!(
                "Change {} completed after {} iteration(s)",
                change_id, iteration
            );
            break true;
        }

        // Warn if no progress
        if new_progress.completed <= progress.completed && iteration > 1 {
            warn!(
                "No progress made for {} (still {}/{}), continuing...",
                change_id, new_progress.completed, new_progress.total
            );
        }
    };

    // Create final commit (Git-only)
    if apply_succeeded && is_git {
        if let Some(ws_mgr) = workspace_manager {
            info!(
                "Creating final Apply commit for {} after {} iterations",
                change_id, iteration
            );
            if let Err(e) = create_final_commit(ws_mgr, workspace_path, change_id).await {
                warn!("Failed to create final commit for {}: {}", change_id, e);
            }
        }
    } else if !apply_succeeded {
        info!(
            "Apply loop exited without completion for {}; WIP snapshots preserved",
            change_id
        );
    }

    // Get final revision
    let revision = if let Some(ws_mgr) = workspace_manager {
        match get_workspace_revision(ws_mgr, workspace_path).await {
            Ok(rev) => rev,
            Err(e) => {
                warn!("Failed to get workspace revision: {}", e);
                String::new()
            }
        }
    } else {
        String::new()
    };

    Ok(ApplyLoopResult {
        revision,
        completed: apply_succeeded,
        iterations: iteration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ApplyConfig tests ===

    #[test]
    fn test_apply_config_default() {
        let config = ApplyConfig::default();
        assert_eq!(config.max_iterations, DEFAULT_MAX_ITERATIONS);
        assert!(config.progress_commits_enabled);
        assert!(!config.streaming_enabled);
    }

    #[test]
    fn test_apply_config_builder() {
        let config = ApplyConfig::new()
            .with_max_iterations(100)
            .with_progress_commits(false)
            .with_streaming(true);

        assert_eq!(config.max_iterations, 100);
        assert!(!config.progress_commits_enabled);
        assert!(config.streaming_enabled);
    }

    // === ApplyIterationResult tests ===

    #[test]
    fn test_apply_iteration_result_complete() {
        let result = ApplyIterationResult::Complete;
        assert!(result.is_complete());
        assert!(!result.is_failed());
    }

    #[test]
    fn test_apply_iteration_result_progress() {
        let result = ApplyIterationResult::Progress {
            completed: 5,
            total: 10,
        };
        assert!(!result.is_complete());
        assert!(!result.is_failed());
    }

    #[test]
    fn test_apply_iteration_result_no_progress() {
        let result = ApplyIterationResult::NoProgress {
            completed: 5,
            total: 10,
        };
        assert!(!result.is_complete());
        assert!(!result.is_failed());
    }

    #[test]
    fn test_apply_iteration_result_failed() {
        let result = ApplyIterationResult::Failed {
            error: "test error".to_string(),
        };
        assert!(!result.is_complete());
        assert!(result.is_failed());
    }

    // === Progress utility tests ===

    #[test]
    fn test_is_progress_complete() {
        assert!(!is_progress_complete(&TaskProgress {
            completed: 0,
            total: 10
        }));
        assert!(!is_progress_complete(&TaskProgress {
            completed: 5,
            total: 10
        }));
        assert!(is_progress_complete(&TaskProgress {
            completed: 10,
            total: 10
        }));
        assert!(is_progress_complete(&TaskProgress {
            completed: 11,
            total: 10
        }));
        assert!(!is_progress_complete(&TaskProgress {
            completed: 0,
            total: 0
        }));
    }

    #[test]
    fn test_progress_increased() {
        let old = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_same = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_increased = TaskProgress {
            completed: 5,
            total: 10,
        };
        let new_decreased = TaskProgress {
            completed: 2,
            total: 10,
        };

        assert!(!progress_increased(&old, &new_same));
        assert!(progress_increased(&old, &new_increased));
        assert!(!progress_increased(&old, &new_decreased));
    }

    // === summarize_output tests ===

    #[test]
    fn test_summarize_output_empty() {
        assert_eq!(summarize_output("", 10), "");
    }

    #[test]
    fn test_summarize_output_short() {
        let output = "line1\nline2\nline3";
        assert_eq!(summarize_output(output, 10), output);
    }

    #[test]
    fn test_summarize_output_long() {
        let output = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10";
        let result = summarize_output(output, 5);
        assert!(result.contains("(10 lines)"));
        assert!(result.contains("6\n7\n8\n9\n10"));
    }

    // === Progress commit message format tests ===

    #[test]
    fn test_progress_commit_message_format() {
        let change_id = "add-feature";
        let progress = TaskProgress {
            completed: 5,
            total: 10,
        };

        let iteration = 3;
        let expected = "WIP: add-feature (5/10 tasks, apply#3)";
        let actual = format_wip_commit_message(change_id, &progress, iteration);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_all_complete() {
        let change_id = "fix-bug";
        let progress = TaskProgress {
            completed: 7,
            total: 7,
        };

        let iteration = 5;
        let expected = "WIP: fix-bug (7/7 tasks, apply#5)";
        let actual = format_wip_commit_message(change_id, &progress, iteration);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_zero_progress() {
        let change_id = "new-change";
        let progress = TaskProgress {
            completed: 0,
            total: 5,
        };

        let iteration = 1;
        let expected = "WIP: new-change (0/5 tasks, apply#1)";
        let actual = format_wip_commit_message(change_id, &progress, iteration);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_special_characters() {
        let change_id = "add-web-monitoring-feature";
        let progress = TaskProgress {
            completed: 50,
            total: 70,
        };

        let iteration = 8;
        let expected = "WIP: add-web-monitoring-feature (50/70 tasks, apply#8)";
        let actual = format_wip_commit_message(change_id, &progress, iteration);

        assert_eq!(actual, expected);
    }
}
