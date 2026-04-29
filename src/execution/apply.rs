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
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Default grace period after observing apply completion (tasks.md complete or
/// blocker marker present) before terminating a lingering agent process group.
const APPLY_COMPLETION_GRACE_DEFAULT_SECS: u64 = 30;

/// Minimum interval between workspace-state re-checks during the apply output
/// stream. Keeps completion detection bounded even when the agent process keeps
/// emitting output or holds its pipes open.
const APPLY_COMPLETION_CHECK_INTERVAL_SECS: u64 = 5;

tokio::task_local! {
    /// Test-only task-local override for the apply completion grace period.
    /// Scoped to the calling task so concurrent tests see the default.
    pub(crate) static APPLY_COMPLETION_GRACE_OVERRIDE_SECS: u64;
}

tokio::task_local! {
    /// Test-only task-local override for the apply completion check interval.
    pub(crate) static APPLY_COMPLETION_CHECK_INTERVAL_OVERRIDE_MS: u64;
}

/// Returns the grace period applied after detecting apply completion. Tests may
/// override via [`scoped_apply_completion_grace_secs_for_test`].
pub(crate) fn apply_completion_grace_period() -> Duration {
    let secs = APPLY_COMPLETION_GRACE_OVERRIDE_SECS
        .try_with(|secs| *secs)
        .ok()
        .filter(|secs| *secs > 0)
        .unwrap_or(APPLY_COMPLETION_GRACE_DEFAULT_SECS);
    Duration::from_secs(secs)
}

/// Returns the interval between workspace-state completion probes.
pub(crate) fn apply_completion_check_interval() -> Duration {
    if let Ok(ms) = APPLY_COMPLETION_CHECK_INTERVAL_OVERRIDE_MS.try_with(|ms| *ms) {
        if ms > 0 {
            return Duration::from_millis(ms);
        }
    }
    Duration::from_secs(APPLY_COMPLETION_CHECK_INTERVAL_SECS)
}

/// Run `fut` with the apply-completion grace period overridden to `secs`.
/// Scoped to the current task only. Test-only helper.
#[cfg(test)]
pub(crate) async fn scoped_apply_completion_grace_secs_for_test<F, R>(secs: u64, fut: F) -> R
where
    F: std::future::Future<Output = R>,
{
    APPLY_COMPLETION_GRACE_OVERRIDE_SECS.scope(secs, fut).await
}

/// Run `fut` with the apply-completion check interval overridden to `ms`.
/// Scoped to the current task only. Test-only helper.
#[cfg(test)]
pub(crate) async fn scoped_apply_completion_check_interval_ms_for_test<F, R>(ms: u64, fut: F) -> R
where
    F: std::future::Future<Output = R>,
{
    APPLY_COMPLETION_CHECK_INTERVAL_OVERRIDE_MS
        .scope(ms, fut)
        .await
}

/// Observed reason apply completion was declared before the child process
/// terminated naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplyCompletionKind {
    /// tasks.md reported all tasks complete.
    TasksComplete,
    /// Apply emitted explicit implementation-blocker marker.
    BlockedHandoff,
}

fn detect_apply_completion(workspace_path: &Path, change_id: &str) -> Option<ApplyCompletionKind> {
    if detect_apply_blocked_handoff(workspace_path, change_id).is_some() {
        return Some(ApplyCompletionKind::BlockedHandoff);
    }
    match check_task_progress(workspace_path, change_id) {
        Ok(progress) if is_progress_complete(&progress) => Some(ApplyCompletionKind::TasksComplete),
        _ => None,
    }
}

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
            // Warn when using archive fallback: the active change directory is gone and we
            // are reading task progress from a previously archived copy.  In a resumed
            // workspace this can make the apply loop exit immediately ("already complete")
            // even though the workspace has not actually run apply.  Callers that reach
            // this branch for workspaces in Archived/Merged state should have been
            // short-circuited by workspace-state detection before invoking check_task_progress.
            warn!(
                "Tasks file for '{}' not found in active change directory; \
                 falling back to archived copy at '{}' ({}/{} tasks complete). \
                 This is expected for Archiving state but unexpected for fresh workspaces.",
                change_id,
                archive_tasks_path.display(),
                progress.completed,
                progress.total
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
/// * `change_id` - The change identifier
/// * `history` - Optional apply history for context
/// * `acceptance_tail` - Optional acceptance tail context (for first retry after acceptance failure)
///
/// # Returns
///
/// The full prompt string to use for the apply command.
pub fn build_apply_prompt(
    config: &OrchestratorConfig,
    change_id: &str,
    history: &str,
    acceptance_tail: &str,
) -> String {
    let user_prompt = config.get_apply_prompt();
    crate::agent::build_apply_prompt(change_id, user_prompt, history, acceptance_tail)
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
    fn on_apply_started(&self, change_id: &str, command: &str);
    /// Called when progress is updated
    fn on_progress_updated(&self, change_id: &str, completed: u32, total: u32);
    /// Called when hook starts
    fn on_hook_started(&self, change_id: &str, hook_type: &str);
    /// Called when hook completes
    fn on_hook_completed(&self, change_id: &str, hook_type: &str);
    /// Called when hook fails
    fn on_hook_failed(&self, change_id: &str, hook_type: &str, error: &str);
    /// Called when apply output is generated
    fn on_apply_output(&self, change_id: &str, line: &OutputLine, iteration: u32);
}

/// No-op event handler for cases where events are not needed
pub struct NoOpEventHandler;

impl ApplyEventHandler for NoOpEventHandler {
    fn on_apply_started(&self, _change_id: &str, _command: &str) {}
    fn on_progress_updated(&self, _change_id: &str, _completed: u32, _total: u32) {}
    fn on_hook_started(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_completed(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_failed(&self, _change_id: &str, _hook_type: &str, _error: &str) {}
    fn on_apply_output(&self, _change_id: &str, _line: &OutputLine, _iteration: u32) {}
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
    /// Apply detected implementation-blocker handoff and stopped apply loop.
    pub blocked_handoff: Option<ApplyBlockedHandoff>,
}

/// Structured metadata for apply-blocked handoff marker artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyBlockedHandoff {
    /// Absolute path to detected blocker marker file.
    pub blocker_path: PathBuf,
}

fn detect_apply_blocked_handoff(
    workspace_path: &Path,
    change_id: &str,
) -> Option<ApplyBlockedHandoff> {
    let blocker_path = workspace_path
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("APPLY_BLOCKED")
        .join("marker.md");

    blocker_path
        .is_file()
        .then_some(ApplyBlockedHandoff { blocker_path })
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

        // Apply-blocked handoff: if apply has produced blocker marker,
        // stop apply loop and return blocked state even when tasks remain unchecked.
        if let Some(blocked_handoff) = detect_apply_blocked_handoff(workspace_path, change_id) {
            info!(
                change_id = change_id,
                blocker_path = %blocked_handoff.blocker_path.display(),
                completed = progress.completed,
                total = progress.total,
                "Apply stalled handoff detected via APPLY_BLOCKED marker; exiting apply loop as stalled"
            );
            break false;
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

        // Execute apply command with history context via AiCommandRunner
        let (mut child, mut rx, start_time, command) = agent
            .run_apply_streaming_with_runner(change_id, ai_runner, Some(workspace_path))
            .await?;

        // Send ApplyStarted event on first iteration (after getting command)
        if first_apply {
            first_apply = false;
            event_handler.on_apply_started(change_id, &command);
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

        // Create output collector for history
        let mut output_collector = OutputCollector::new();

        // Stream output with apply-completion detection.
        //
        // apply commands occasionally keep their stdout/stderr pipes open even
        // after tasks.md reaches a completion condition (all tasks checked or
        // blocker marker produced). Without a grace-period guard, the orchestrator
        // blocked on `rx.recv()` indefinitely, which previously left the apply
        // handoff stuck and prevented acceptance from starting. Mirror the
        // acceptance-verdict grace period: once we observe the completion
        // condition via workspace state, start a bounded grace timer and
        // terminate the child when it expires.
        let grace_period = apply_completion_grace_period();
        let check_interval = apply_completion_check_interval();
        let mut completion_kind: Option<ApplyCompletionKind> = None;
        let mut completion_deadline: Option<tokio::time::Instant> = None;
        let mut early_terminated = false;
        let mut next_check_at = tokio::time::Instant::now() + check_interval;

        loop {
            // Probe workspace state before awaiting the next line so a completion
            // that lands between output bursts is observed promptly and does not
            // depend on receiving further stdout/stderr data.
            if completion_kind.is_none() && tokio::time::Instant::now() >= next_check_at {
                completion_kind = detect_apply_completion(workspace_path, change_id);
                next_check_at = tokio::time::Instant::now() + check_interval;
                if let Some(kind) = completion_kind {
                    completion_deadline = Some(tokio::time::Instant::now() + grace_period);
                    info!(
                        change_id = change_id,
                        kind = ?kind,
                        grace_secs = grace_period.as_secs(),
                        "Apply completion observed; starting grace period before terminating lingering apply child"
                    );
                }
            }

            // Bound the receive so completion detection stays responsive even if
            // the child floods or stalls the output pipe.
            let wait_deadline = match completion_deadline {
                Some(deadline) => deadline,
                None => next_check_at,
            };

            let recv_result = tokio::time::timeout_at(wait_deadline, rx.recv()).await;

            match recv_result {
                Ok(Some(line)) => {
                    match &line {
                        OutputLine::Stdout(s) => output_collector.add_stdout(s),
                        OutputLine::Stderr(s) => output_collector.add_stderr(s),
                    }
                    event_handler.on_apply_output(change_id, &line, iteration);
                    output_handler(line).await;
                }
                Ok(None) => break,
                Err(_) => {
                    if let Some(deadline) = completion_deadline {
                        if tokio::time::Instant::now() >= deadline {
                            info!(
                                change_id = change_id,
                                kind = ?completion_kind,
                                grace_secs = grace_period.as_secs(),
                                "Apply completion grace period expired; terminating lingering apply child"
                            );
                            let _ = child.terminate();
                            early_terminated = true;
                            break;
                        }
                    }
                    // Periodic wakeup: loop back and re-probe workspace state.
                }
            }
        }

        // Drain any remaining output that arrived after we signalled terminate,
        // so the history tail reflects what the agent produced before exit.
        while let Ok(line) = rx.try_recv() {
            match &line {
                OutputLine::Stdout(s) => output_collector.add_stdout(s),
                OutputLine::Stderr(s) => output_collector.add_stderr(s),
            }
            event_handler.on_apply_output(change_id, &line, iteration);
            output_handler(line).await;
        }

        // Wait for child process. After an early grace-driven terminate this
        // returns the signalled exit status, which is success-equivalent only
        // when we observed a completion condition (see below).
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to wait for apply command for '{}' in workspace '{}' (iteration {}): {}",
                change_id,
                workspace_path.display(),
                iteration,
                e
            ))
        })?;

        // Completion-finalized run: non-zero exit produced by our own terminate
        // after observing tasks-complete or blocked-handoff. Only this path is
        // allowed to short-circuit the usual command-failure handling so that
        // runs which never reached a completion condition are still treated as
        // failures/retries per existing policy.
        let completion_finalized_run = early_terminated && completion_kind.is_some();

        // Record apply attempt for history
        agent.record_apply_attempt(
            change_id,
            &status,
            start_time,
            output_collector.stdout_tail(),
            output_collector.stderr_tail(),
        );

        let permission_reject = crate::permission::detect_permission_reject(
            output_collector.stdout_tail().as_deref(),
            output_collector.stderr_tail().as_deref(),
        );

        if let Some(reject) = &permission_reject {
            warn!(
                "Permission auto-reject detected for {}: {}",
                change_id, reject.denied_path
            );
        }

        if !status.success() && permission_reject.is_none() && !completion_finalized_run {
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

        // If apply was finalized via grace-driven terminate because blocker marker
        // appeared, short-circuit immediately so the outer loop does not spawn
        // another apply child or treat the empty snapshot as a stall. Tasks-
        // complete runs fall through to the normal post_apply/final-commit path.
        if completion_finalized_run
            && matches!(completion_kind, Some(ApplyCompletionKind::BlockedHandoff))
        {
            info!(
                change_id = change_id,
                "Apply loop exiting for rejecting review handoff after grace-driven terminate"
            );
            break false;
        }

        // Permission auto-reject is handled as a soft error.
        // Log the error and continue the loop to give the agent another iteration.
        // The stall detector will catch repeated failures if no progress is made.
        if let Some(reject) = permission_reject {
            let task_state_changed =
                new_progress.completed > progress.completed || new_progress.total != progress.total;

            if !task_state_changed {
                warn!(
                    "Permission auto-reject detected for {} but task state unchanged; continuing to next iteration",
                    change_id
                );
                warn!("Denied path: {}", reject.denied_path);
                warn!("Guidance: {}", reject.format_error_message());
            } else {
                info!(
                    "Permission auto-reject detected for {} but task state changed; continuing",
                    change_id
                );
            }

            if !status.success() {
                warn!(
                    "Apply command for {} exited non-zero after permission auto-reject; continuing to next iteration",
                    change_id
                );
            }
        }

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

    let blocked_handoff = if apply_succeeded {
        None
    } else {
        detect_apply_blocked_handoff(workspace_path, change_id)
    };

    Ok(ApplyLoopResult {
        revision,
        completed: apply_succeeded,
        iterations: iteration,
        blocked_handoff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    #[test]
    fn test_detect_apply_blocked_handoff_absent_without_blocked_marker() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();

        std::fs::create_dir_all(workspace.join("openspec/changes/change-a")).unwrap();

        let handoff = detect_apply_blocked_handoff(workspace, "change-a");
        assert!(handoff.is_none());
    }

    #[test]
    fn test_detect_apply_blocked_handoff_present_with_blocked_marker() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();
        let blocker_path = workspace
            .join("openspec")
            .join("changes")
            .join("change-a")
            .join("APPLY_BLOCKED")
            .join("marker.md");
        std::fs::create_dir_all(blocker_path.parent().unwrap()).unwrap();
        std::fs::write(&blocker_path, "# APPLY_BLOCKED\n- reason: blocked\n").unwrap();

        let handoff = detect_apply_blocked_handoff(workspace, "change-a");
        assert!(handoff.is_some());
        assert_eq!(
            handoff.unwrap().blocker_path,
            blocker_path,
            "detected handoff should point to APPLY_BLOCKED marker"
        );
    }

    #[tokio::test]
    async fn test_execute_apply_loop_returns_blocked_handoff_without_stall_loop() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();
        let change_id = "blocked-change";
        let change_dir = workspace.join("openspec").join("changes").join(change_id);
        let blocked_dir = change_dir.join("APPLY_BLOCKED");
        std::fs::create_dir_all(&blocked_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [ ] pending\n",
        )
        .unwrap();
        std::fs::write(
            blocked_dir.join("marker.md"),
            "# APPLY_BLOCKED\n\n- change_id: blocked-change\n- reason: apply blocked\n",
        )
        .unwrap();

        let config = OrchestratorConfig::default();
        let mut agent = AgentRunner::new(config.clone());
        let queue_config = crate::command_queue::CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: Vec::new(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 0,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: false,
        };
        let shared_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        let ai_runner = crate::ai_command_runner::AiCommandRunner::new(queue_config, shared_state);

        let result = execute_apply_loop(
            change_id,
            workspace,
            &config,
            &mut agent,
            VcsBackend::Auto,
            None,
            None,
            &ApplyLoopHookContext::serial(0, 1, 1),
            &NoOpEventHandler,
            None,
            &ai_runner,
            |_line| async move {},
        )
        .await
        .expect("apply loop should return blocked handoff without error");

        assert!(
            !result.completed,
            "blocked handoff should not be treated as completed apply"
        );
        assert_eq!(
            result.iterations, 1,
            "blocked handoff should exit before retry/stall loop"
        );
        assert!(
            result.blocked_handoff.is_some(),
            "blocked handoff metadata must be returned"
        );
    }

    fn make_test_ai_runner() -> crate::ai_command_runner::AiCommandRunner {
        let queue_config = crate::command_queue::CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: Vec::new(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 0,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: false,
        };
        let shared_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        crate::ai_command_runner::AiCommandRunner::new(queue_config, shared_state)
    }

    #[cfg_attr(windows, ignore)]
    #[tokio::test]
    async fn test_execute_apply_loop_terminates_lingering_child_after_tasks_complete() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();
        let change_id = "linger-complete";
        let change_dir = workspace.join("openspec").join("changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [ ] one\n",
        )
        .unwrap();

        // Apply command: mark tasks complete, then sleep far beyond the test
        // budget. The grace period must terminate the sleeper.
        let apply_cmd = "sh -c 'printf \"## Implementation Tasks\\n- [x] one\\n\" > openspec/changes/{change_id}/tasks.md; echo applied; sleep 120'".to_string();

        let config = OrchestratorConfig {
            apply_command: Some(apply_cmd),
            ..Default::default()
        };
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = make_test_ai_runner();

        let start = std::time::Instant::now();
        let result = scoped_apply_completion_grace_secs_for_test(
            1,
            scoped_apply_completion_check_interval_ms_for_test(
                200,
                execute_apply_loop(
                    change_id,
                    workspace,
                    &config,
                    &mut agent,
                    VcsBackend::Auto,
                    None,
                    None,
                    &ApplyLoopHookContext::serial(0, 1, 1),
                    &NoOpEventHandler,
                    None,
                    &ai_runner,
                    |_line| async move {},
                ),
            ),
        )
        .await
        .expect("apply loop must finish without error despite lingering child");

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(20),
            "apply loop must exit within grace period + buffer, but took {:?}",
            elapsed
        );
        assert!(
            result.completed,
            "tasks-complete run must be reported as completed"
        );
        assert!(
            result.blocked_handoff.is_none(),
            "tasks-complete run must not report a blocked handoff"
        );
        assert_eq!(
            result.iterations, 1,
            "tasks-complete grace-terminated run should exit in a single iteration"
        );
    }

    #[cfg_attr(windows, ignore)]
    #[tokio::test]
    async fn test_execute_apply_loop_terminates_lingering_child_after_blocked_handoff() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();
        let change_id = "linger-blocked";
        let change_dir = workspace.join("openspec").join("changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [ ] one\n",
        )
        .unwrap();

        // Apply command: write APPLY_BLOCKED marker then sleep. The apply-blocked
        // completion detector should terminate this lingering process during grace.
        let apply_cmd = "sh -c 'mkdir -p openspec/changes/{change_id}/APPLY_BLOCKED; printf \"# APPLY_BLOCKED\\n\\n- change_id: linger-blocked\\n- reason: test\\n\" > openspec/changes/{change_id}/APPLY_BLOCKED/marker.md; echo blocked; sleep 120'".to_string();

        let config = OrchestratorConfig {
            apply_command: Some(apply_cmd),
            ..Default::default()
        };
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = make_test_ai_runner();

        let start = std::time::Instant::now();
        let result = scoped_apply_completion_grace_secs_for_test(
            1,
            scoped_apply_completion_check_interval_ms_for_test(
                200,
                execute_apply_loop(
                    change_id,
                    workspace,
                    &config,
                    &mut agent,
                    VcsBackend::Auto,
                    None,
                    None,
                    &ApplyLoopHookContext::serial(0, 1, 1),
                    &NoOpEventHandler,
                    None,
                    &ai_runner,
                    |_line| async move {},
                ),
            ),
        )
        .await
        .expect("apply loop must finish without error despite lingering child");

        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(20),
            "blocked-handoff apply loop must exit within grace period + buffer, but took {:?}",
            elapsed
        );
        assert!(
            !result.completed,
            "blocked-handoff grace-terminated run must not be reported as completed"
        );
        assert!(
            result.blocked_handoff.is_some(),
            "blocked-handoff grace-terminated run must expose blocker_path"
        );
        assert_eq!(
            result.iterations, 1,
            "blocked-handoff grace-terminated run should exit in a single iteration"
        );
    }
}
