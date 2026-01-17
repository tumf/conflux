//! Common archive operation logic for OpenSpec Orchestrator.
//!
//! This module provides shared archive functionality used by both serial (TUI) and
//! parallel execution modes. It consolidates duplicate code from:
//! - `src/tui/orchestrator.rs::archive_single_change()`
//! - `src/parallel/executor.rs::execute_archive_in_workspace()`
//!
//! # Common Operations
//!
//! - Task completion verification (100% check before archiving)
//! - Archive path verification (change moved to archive directory)
//! - Archive command execution with streaming output
//!
//! # Differences Between Modes
//!
//! | Aspect | Serial (TUI) | Parallel |
//! |--------|--------------|----------|
//! | Hooks  | Supported    | Not supported (future: add-parallel-hooks) |
//! | Working directory | Current directory | Workspace path |

use std::future::Future;
use std::path::Path;

use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::agent::{AgentRunner, OutputLine};
use crate::error::{OrchestratorError, Result};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::task_parser;
use crate::vcs::git::commands as git_commands;
use crate::vcs::VcsBackend;

/// Maximum number of archive retries after a verification failure.
pub const ARCHIVE_COMMAND_MAX_RETRIES: u32 = 2;

/// Result of archive path verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveVerificationResult {
    /// Archive was successful - change moved to archive directory.
    Success,
    /// Archive failed - change still exists in original location.
    NotArchived {
        /// The change ID that was not archived.
        change_id: String,
    },
}

impl ArchiveVerificationResult {
    /// Check if the verification indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

fn archive_entry_exists(change_id: &str, archive_dir: &Path) -> bool {
    if !archive_dir.exists() {
        return false;
    }

    std::fs::read_dir(archive_dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|entry| {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                name_str == change_id || name_str.ends_with(&format!("-{}", change_id))
            })
        })
        .unwrap_or(false)
}

/// Check if a change has already been archived in the given base path.
///
/// This stricter check requires that the change directory is gone and
/// that an archive entry exists for the change.
#[allow(dead_code)]
pub fn is_change_archived(change_id: &str, base_path: Option<&Path>) -> bool {
    let (change_path, archive_dir) = match base_path {
        Some(base) => (
            base.join("openspec/changes").join(change_id),
            base.join("openspec/changes/archive"),
        ),
        None => (
            Path::new("openspec/changes").join(change_id),
            Path::new("openspec/changes/archive").to_path_buf(),
        ),
    };

    let change_exists = change_path.exists();
    let archive_exists = archive_entry_exists(change_id, &archive_dir);

    debug!(
        change_id = %change_id,
        change_path = %change_path.display(),
        archive_dir = %archive_dir.display(),
        change_exists = change_exists,
        archive_exists = archive_exists,
        "is_change_archived: checking paths"
    );

    archive_exists && !change_exists
}

/// Check if the archive commit is complete for a change.
///
/// The archive commit is considered complete when:
/// 1. The working tree is clean
/// 2. The latest commit subject matches `Archive: <change_id>`
/// 3. The change directory does not exist in `openspec/changes/<change_id>`
pub async fn is_archive_commit_complete(change_id: &str, base_path: Option<&Path>) -> Result<bool> {
    let repo_root = base_path.unwrap_or_else(|| Path::new("."));

    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to check git status: {}", e)))?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to check git status: {}",
            stderr
        )));
    }

    let is_clean = String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .is_empty();

    let log_output = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to read git log: {}", e)))?;

    if !log_output.status.success() {
        let stderr = String::from_utf8_lossy(&log_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to read git log: {}",
            stderr
        )));
    }

    let subject = String::from_utf8_lossy(&log_output.stdout)
        .trim()
        .to_string();
    let expected_subject = format!("Archive: {}", change_id);

    // Check if openspec/changes/<change_id> exists (should NOT exist for complete archive)
    let change_path = repo_root.join("openspec/changes").join(change_id);
    let change_exists = change_path.exists();

    debug!(
        change_id = %change_id,
        is_clean = is_clean,
        subject = %subject,
        expected_subject = %expected_subject,
        change_path = %change_path.display(),
        change_exists = change_exists,
        "is_archive_commit_complete: checking commit state"
    );

    Ok(is_clean && subject == expected_subject && !change_exists)
}

/// Ensure the archive commit exists for a change.
///
/// When the working tree is dirty after archive, this function runs the resolve
/// command to create a commit with subject `Archive: <change_id>`.
///
/// Returns an error if `openspec/changes/<change_id>` still exists, indicating
/// the change was not properly archived.
pub async fn ensure_archive_commit<F, Fut>(
    change_id: &str,
    repo_root: &Path,
    agent: &AgentRunner,
    vcs_backend: VcsBackend,
    mut handle_output: F,
) -> Result<()>
where
    F: FnMut(OutputLine) -> Fut,
    Fut: Future<Output = ()>,
{
    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            let is_git_repo = git_commands::check_git_repo(repo_root)
                .await
                .map_err(OrchestratorError::from_vcs_error)?;

            if !is_git_repo {
                if matches!(vcs_backend, VcsBackend::Git) {
                    return Err(OrchestratorError::GitCommand(format!(
                        "Git repository not found at {}",
                        repo_root.display()
                    )));
                }
                return Ok(());
            }

            // Check if openspec/changes/<change_id> exists before attempting to create archive commit
            let change_path = repo_root.join("openspec/changes").join(change_id);
            if change_path.exists() {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot create archive commit for '{}': change directory still exists at {}. \
                     The archive operation did not properly move the change to the archive directory.",
                    change_id,
                    change_path.display()
                )));
            }

            if is_archive_commit_complete(change_id, Some(repo_root)).await? {
                return Ok(());
            }

            let (has_changes, _) = git_commands::has_uncommitted_changes(repo_root)
                .await
                .map_err(OrchestratorError::from_vcs_error)?;
            if !has_changes {
                let subject = git_commands::run_git(&["log", "-1", "--format=%s"], repo_root)
                    .await
                    .map_err(OrchestratorError::from_vcs_error)?;
                let subject = subject.trim();
                let wip_prefix = format!("WIP(archive): {}", change_id);
                if subject.starts_with(&wip_prefix) {
                    match git_commands::squash_archive_wip_commits(repo_root, change_id).await {
                        Ok(()) => {
                            if is_archive_commit_complete(change_id, Some(repo_root)).await? {
                                return Ok(());
                            }
                        }
                        Err(err) => {
                            warn!(
                                change_id = %change_id,
                                error = %err,
                                "Failed to squash WIP(archive) commits before resolving archive"
                            );
                        }
                    }
                }
            }

            let prompt = format!(
                "You are finalizing the archive commit for change '{change_id}'.\n\n\
Requirements:\n\
1) Ensure `git status --porcelain` is empty when done.\n\
2) If there are changes, run `git add -A` and commit with message \"Archive: {change_id}\".\n\
3) If a pre-commit hook modifies files or stops the commit, re-run `git add -A` and commit with the same message.\n\
4) If the latest commit already has subject \"Archive: {change_id}\" and the working tree is clean, do nothing.\n\
5) Do not use destructive commands like `git reset --hard`.",
                change_id = change_id
            );

            let (mut child, mut rx) = agent
                .run_resolve_streaming_in_dir(&prompt, repo_root)
                .await?;

            while let Some(line) = rx.recv().await {
                handle_output(line).await;
            }

            let status = child.wait().await.map_err(|e| {
                OrchestratorError::AgentCommand(format!("Archive resolve command failed: {}", e))
            })?;

            if !status.success() {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Archive resolve command failed with exit code: {:?}",
                    status.code()
                )));
            }

            if !is_archive_commit_complete(change_id, Some(repo_root)).await? {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Archive commit verification failed for {}",
                    change_id
                )));
            }
        }
    }

    Ok(())
}

/// Delete the change directory after successful archive.
///
/// This function removes the `openspec/changes/{change_id}` directory after
/// the archive command has successfully moved it to the archive directory.
///
/// # Arguments
///
/// * `change_id` - The ID of the change to delete
/// * `base_path` - Base path to delete from. Pass `None` for current directory (serial mode),
///   or `Some(path)` for workspace directory (parallel mode).
///
/// # Returns
///
/// * `Ok(())` - Directory was deleted successfully or didn't exist
/// * `Err(e)` - Failed to delete the directory
pub fn delete_change_directory(change_id: &str, base_path: Option<&Path>) -> Result<()> {
    let change_path = match base_path {
        Some(base) => base.join("openspec/changes").join(change_id),
        None => Path::new("openspec/changes").join(change_id),
    };

    debug!(
        change_id = %change_id,
        change_path = %change_path.display(),
        "delete_change_directory: attempting to delete"
    );

    // If the directory doesn't exist, consider it success (idempotent)
    if !change_path.exists() {
        debug!(
            change_id = %change_id,
            "delete_change_directory: directory does not exist, skipping"
        );
        return Ok(());
    }

    // Remove the directory and all its contents
    std::fs::remove_dir_all(&change_path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to delete change directory '{}' at {}: {}",
            change_id,
            change_path.display(),
            e
        ))
    })?;

    info!(
        change_id = %change_id,
        change_path = %change_path.display(),
        "delete_change_directory: successfully deleted"
    );

    Ok(())
}

/// Verify that a change was actually archived.
///
/// This function checks that the archive operation actually moved the change
/// from `openspec/changes/{change_id}` to `openspec/changes/archive/`.
///
/// The archive directory may contain entries in different formats:
/// - `{change_id}` - Simple archive
/// - `{date}-{change_id}` - Date-prefixed archive (e.g., `2024-01-15-add-feature`)
///
/// # Arguments
///
/// * `change_id` - The ID of the change to verify
/// * `base_path` - Base path to check from. Pass `None` for current directory (serial mode),
///   or `Some(path)` for workspace directory (parallel mode).
///
/// # Returns
///
/// * `ArchiveVerificationResult::Success` - Change was archived successfully
/// * `ArchiveVerificationResult::NotArchived` - Change still exists in original location
///
/// # Examples
///
/// ```ignore
/// // Serial mode - check in current directory
/// let result = verify_archive_completion("add-feature", None);
///
/// // Parallel mode - check in workspace
/// let result = verify_archive_completion("add-feature", Some(&workspace_path));
/// ```
pub fn verify_archive_completion(
    change_id: &str,
    base_path: Option<&Path>,
) -> ArchiveVerificationResult {
    let (change_path, archive_dir) = match base_path {
        Some(base) => (
            base.join("openspec/changes").join(change_id),
            base.join("openspec/changes/archive"),
        ),
        None => (
            Path::new("openspec/changes").join(change_id),
            Path::new("openspec/changes/archive").to_path_buf(),
        ),
    };

    let change_exists = change_path.exists();

    // Check if archive directory contains this change
    // Supports both direct match and date-prefixed format
    let archive_exists = archive_entry_exists(change_id, &archive_dir);

    debug!(
        change_id = %change_id,
        change_path = %change_path.display(),
        archive_dir = %archive_dir.display(),
        change_exists = change_exists,
        archive_exists = archive_exists,
        "verify_archive_completion: checking paths"
    );

    // Archive is successful ONLY if:
    // 1. Change no longer exists in openspec/changes/{change_id}
    // If the change directory exists, the archive is incomplete regardless of
    // whether an archive entry exists (the archive command may have failed
    // to move/remove the original change directory).
    if !change_exists {
        ArchiveVerificationResult::Success
    } else {
        ArchiveVerificationResult::NotArchived {
            change_id: change_id.to_string(),
        }
    }
}

/// Verify that all tasks are complete for a change.
///
/// This function reads and parses the tasks.md file to check if all tasks
/// are marked as complete (100% completion rate).
///
/// # Arguments
///
/// * `change_id` - The ID of the change to verify
/// * `base_path` - Base path to check from. Pass `None` for current directory (serial mode),
///   or `Some(path)` for workspace directory (parallel mode).
///
/// # Returns
///
/// * `Ok(true)` - All tasks are complete
/// * `Ok(false)` - Some tasks are incomplete
/// * `Err` - Failed to read or parse tasks file
///
/// # Examples
///
/// ```ignore
/// // Serial mode
/// if verify_task_completion("add-feature", None)? {
///     // Ready to archive
/// }
///
/// // Parallel mode
/// if verify_task_completion("add-feature", Some(&workspace_path))? {
///     // Ready to archive
/// }
/// ```
#[allow(dead_code)] // Provided for API completeness; used by get_task_progress pattern
pub fn verify_task_completion(change_id: &str, base_path: Option<&Path>) -> Result<bool> {
    let tasks_path = match base_path {
        Some(base) => base
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
        None => Path::new("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
    };

    debug!(
        change_id = %change_id,
        tasks_path = %tasks_path.display(),
        "verify_task_completion: checking tasks"
    );

    if !tasks_path.exists() {
        // If tasks file doesn't exist, we can't verify completion
        // Return error to let caller decide how to handle
        return Err(OrchestratorError::ConfigLoad(format!(
            "Tasks file not found for change '{}': {:?}",
            change_id, tasks_path
        )));
    }

    let progress = task_parser::parse_file(&tasks_path, Some(change_id))?;

    debug!(
        change_id = %change_id,
        completed = progress.completed,
        total = progress.total,
        "verify_task_completion: parsed progress"
    );

    // Complete if all tasks are done (and there are tasks to complete)
    Ok(progress.total > 0 && progress.completed >= progress.total)
}

/// Get task progress for a change.
///
/// This is a convenience function that returns the full progress information
/// rather than just a boolean completion status.
///
/// # Arguments
///
/// * `change_id` - The ID of the change to check
/// * `base_path` - Base path to check from. Pass `None` for current directory,
///   or `Some(path)` for workspace directory.
///
/// # Returns
///
/// * `Ok(Some(progress))` - Progress information with completed/total counts
/// * `Ok(None)` - Tasks file doesn't exist
/// * `Err` - Failed to parse tasks file
pub fn get_task_progress(
    change_id: &str,
    base_path: Option<&Path>,
) -> Result<Option<task_parser::TaskProgress>> {
    let tasks_path = match base_path {
        Some(base) => base
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
        None => Path::new("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
    };

    if !tasks_path.exists() {
        return Ok(None);
    }

    let progress = task_parser::parse_file(&tasks_path, Some(change_id))?;
    Ok(Some(progress))
}

/// Build an error message for failed archive verification.
///
/// Creates a descriptive error message when archive verification fails,
/// suitable for logging or sending to the user.
pub fn build_archive_error_message(change_id: &str) -> String {
    format!(
        "Archive command succeeded but change '{}' was not actually archived. \
         The change directory still exists in openspec/changes/. \
         The archive command may not have executed 'openspec archive' correctly.",
        change_id
    )
}

/// Event handler for archive loop events.
///
/// This trait allows the archive loop to send events to different handlers
/// (e.g., TUI event channel, CLI logger, parallel event bus).
#[allow(dead_code)]
pub trait ArchiveEventHandler {
    /// Called when archive iteration starts
    fn on_archive_started(&self, change_id: &str);
    /// Called when hook starts
    fn on_hook_started(&self, change_id: &str, hook_type: &str);
    /// Called when hook completes
    fn on_hook_completed(&self, change_id: &str, hook_type: &str);
    /// Called when hook fails
    fn on_hook_failed(&self, change_id: &str, hook_type: &str, error: &str);
    /// Called when archive output is generated
    fn on_archive_output(&self, change_id: &str, line: &OutputLine);
}

/// No-op event handler for cases where events are not needed
#[allow(dead_code)]
pub struct NoOpArchiveEventHandler;

impl ArchiveEventHandler for NoOpArchiveEventHandler {
    fn on_archive_started(&self, _change_id: &str) {}
    fn on_hook_started(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_completed(&self, _change_id: &str, _hook_type: &str) {}
    fn on_hook_failed(&self, _change_id: &str, _hook_type: &str, _error: &str) {}
    fn on_archive_output(&self, _change_id: &str, _line: &OutputLine) {}
}

/// Context for building hook contexts in the archive loop
#[allow(dead_code)]
pub struct ArchiveLoopHookContext {
    /// Changes processed so far
    pub changes_processed: usize,
    /// Total changes in this run
    pub total_changes: usize,
    /// Remaining changes
    pub remaining_changes: usize,
    /// Apply count for this change
    pub apply_count: u32,
    /// Workspace path for parallel mode (optional)
    pub workspace_path: Option<String>,
    /// Group index for parallel mode (optional)
    pub group_index: Option<usize>,
}

#[allow(dead_code)]
impl ArchiveLoopHookContext {
    /// Create a new hook context for serial mode
    pub fn serial(
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
            workspace_path: None,
            group_index: None,
        }
    }

    /// Create a new hook context for parallel mode
    pub fn parallel(
        changes_processed: usize,
        total_changes: usize,
        remaining_changes: usize,
        apply_count: u32,
        workspace_path: String,
        group_index: usize,
    ) -> Self {
        Self {
            changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
            workspace_path: Some(workspace_path),
            group_index: Some(group_index),
        }
    }

    /// Build a HookContext from this archive loop context
    fn build_hook_context(&self, change_id: &str, completed: u32, total: u32) -> HookContext {
        let mut ctx = HookContext::new(
            self.changes_processed,
            self.total_changes,
            self.remaining_changes,
            false,
        )
        .with_change(change_id, completed, total)
        .with_apply_count(self.apply_count);

        if let Some(ref workspace_path) = self.workspace_path {
            if let Some(group_index) = self.group_index {
                ctx = ctx.with_parallel_context(workspace_path, Some(group_index as u32));
            }
        }

        ctx
    }
}

/// Result of the unified archive loop
#[derive(Debug)]
#[allow(dead_code)]
pub struct ArchiveLoopResult {
    /// Whether the archive succeeded
    pub succeeded: bool,
    /// Number of attempts made
    pub attempts: u32,
}

/// Execute archive iterations until verification succeeds or max retries reached.
///
/// This is the unified archive loop used by both serial and parallel modes.
///
/// # Arguments
///
/// * `change_id` - The change to archive
/// * `workspace_path` - Working directory (worktree for parallel, repo root for serial)
/// * `agent` - Agent runner for executing commands
/// * `vcs_backend` - VCS backend (Git, Auto, etc.)
/// * `hooks` - Optional hook runner
/// * `hook_ctx` - Context for building hook contexts
/// * `event_handler` - Event handler for sending events
/// * `cancel_token` - Optional cancellation token
///
/// # Returns
///
/// * `Ok(ArchiveLoopResult)` - Archive loop completed (success or max attempts)
/// * `Err(e)` - An error occurred (hook failure, command spawn failure, etc.)
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub async fn execute_archive_loop<E, F, Fut>(
    change_id: &str,
    workspace_path: &Path,
    agent: &mut AgentRunner,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    hook_ctx: &ArchiveLoopHookContext,
    event_handler: &E,
    cancel_token: Option<&CancellationToken>,
    mut output_handler: F,
) -> Result<ArchiveLoopResult>
where
    E: ArchiveEventHandler,
    F: FnMut(OutputLine) -> Fut,
    Fut: Future<Output = ()>,
{
    // Check cancellation before starting
    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err(OrchestratorError::AgentCommand("Cancelled".to_string()));
    }

    // Get task progress for hook context
    let progress = get_task_progress(change_id, Some(workspace_path))?.unwrap_or_default();

    // Run on_change_complete hook
    if let Some(hook_runner) = hooks {
        let complete_ctx =
            hook_ctx.build_hook_context(change_id, progress.completed, progress.total);

        event_handler.on_hook_started(change_id, "on_change_complete");

        match hook_runner
            .run_hook(HookType::OnChangeComplete, &complete_ctx)
            .await
        {
            Ok(()) => {
                event_handler.on_hook_completed(change_id, "on_change_complete");
            }
            Err(e) => {
                error!("on_change_complete hook failed for {}: {}", change_id, e);
                event_handler.on_hook_failed(change_id, "on_change_complete", &e.to_string());
                return Err(e);
            }
        }
    }

    // Run pre_archive hook
    if let Some(hook_runner) = hooks {
        let pre_archive_ctx =
            hook_ctx.build_hook_context(change_id, progress.completed, progress.total);

        event_handler.on_hook_started(change_id, "pre_archive");

        match hook_runner
            .run_hook(HookType::PreArchive, &pre_archive_ctx)
            .await
        {
            Ok(()) => {
                event_handler.on_hook_completed(change_id, "pre_archive");
            }
            Err(e) => {
                error!("pre_archive hook failed for {}: {}", change_id, e);
                event_handler.on_hook_failed(change_id, "pre_archive", &e.to_string());
                return Err(e);
            }
        }
    }

    // Send ArchiveStarted event
    event_handler.on_archive_started(change_id);

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;

    let archive_succeeded = loop {
        attempt += 1;

        // Check cancellation
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            return Err(OrchestratorError::AgentCommand("Cancelled".to_string()));
        }

        info!(
            "Executing archive attempt #{} for {} (max: {})",
            attempt, change_id, max_attempts
        );

        // Execute archive command with history context
        let (mut child, mut rx, start_time) = agent
            .run_archive_streaming(change_id, Some(workspace_path))
            .await?;

        // Stream output
        while let Some(line) = rx.recv().await {
            event_handler.on_archive_output(change_id, &line);
            output_handler(line).await;
        }

        // Wait for child process
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to wait for archive command: {}", e))
        })?;

        // If archive command succeeded, delete the change directory
        if status.success() {
            if let Err(e) = delete_change_directory(change_id, Some(workspace_path)) {
                // Deletion failure is treated as archive failure
                let error_msg = format!(
                    "Archive command succeeded but failed to delete change directory: {}",
                    e
                );
                error!("{}", error_msg);

                // Run on_error hook
                if let Some(hook_runner) = hooks {
                    let error_ctx = hook_ctx
                        .build_hook_context(change_id, progress.completed, progress.total)
                        .with_error(&error_msg);
                    let _ = hook_runner.run_hook(HookType::OnError, &error_ctx).await;
                }

                return Err(OrchestratorError::AgentCommand(error_msg));
            }
        }

        // Verify archive completion
        let verification = verify_archive_completion(change_id, Some(workspace_path));

        // Record archive attempt for history
        let verification_msg = if verification.is_success() {
            None
        } else {
            Some(build_archive_error_message(change_id))
        };
        agent.record_archive_attempt(change_id, &status, start_time, verification_msg.clone());

        if !status.success() {
            let error_msg = format!("Archive command failed with exit code: {:?}", status.code());

            // Run on_error hook
            if let Some(hook_runner) = hooks {
                let error_ctx = hook_ctx
                    .build_hook_context(change_id, progress.completed, progress.total)
                    .with_error(&error_msg);
                let _ = hook_runner.run_hook(HookType::OnError, &error_ctx).await;
            }

            return Err(OrchestratorError::AgentCommand(error_msg));
        }

        // Check verification
        if verification.is_success() {
            info!("Archive verification passed for {}", change_id);
            break true;
        }

        // Verification failed
        warn!(
            "Archive verification failed for {} (attempt {}/{})",
            change_id, attempt, max_attempts
        );

        if attempt >= max_attempts {
            let error_msg = build_archive_error_message(change_id);

            // Run on_error hook
            if let Some(hook_runner) = hooks {
                let error_ctx = hook_ctx
                    .build_hook_context(change_id, progress.completed, progress.total)
                    .with_error(&error_msg);
                let _ = hook_runner.run_hook(HookType::OnError, &error_ctx).await;
            }

            return Err(OrchestratorError::AgentCommand(error_msg));
        }
    };

    // Ensure archive commit is clean (Git-only)
    if archive_succeeded && matches!(vcs_backend, VcsBackend::Git | VcsBackend::Auto) {
        info!("Ensuring clean archive commit for {}", change_id);
        ensure_archive_commit(
            change_id,
            workspace_path,
            agent,
            vcs_backend,
            output_handler,
        )
        .await?;
    }

    // Clear history on successful archive
    if archive_succeeded {
        agent.clear_apply_history(change_id);
        agent.clear_archive_history(change_id);
    }

    // Run post_archive hook
    if archive_succeeded {
        if let Some(hook_runner) = hooks {
            let post_archive_ctx =
                hook_ctx.build_hook_context(change_id, progress.completed, progress.total);

            event_handler.on_hook_started(change_id, "post_archive");

            match hook_runner
                .run_hook(HookType::PostArchive, &post_archive_ctx)
                .await
            {
                Ok(()) => {
                    event_handler.on_hook_completed(change_id, "post_archive");
                }
                Err(e) => {
                    error!("post_archive hook failed for {}: {}", change_id, e);
                    event_handler.on_hook_failed(change_id, "post_archive", &e.to_string());
                    return Err(e);
                }
            }
        }
    }

    Ok(ArchiveLoopResult {
        succeeded: archive_succeeded,
        attempts: attempt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRunner;
    use crate::config::OrchestratorConfig;
    use crate::vcs::VcsBackend;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    // ===========================
    // verify_archive_completion tests
    // ===========================

    #[test]
    fn test_verify_archive_change_not_archived() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure: change exists, archive doesn't
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        let change_path = changes_dir.join(change_id);
        fs::create_dir(&change_path).unwrap();

        let result = verify_archive_completion(change_id, Some(base));
        assert!(!result.is_success());
        assert_eq!(
            result,
            ArchiveVerificationResult::NotArchived {
                change_id: "my-change".to_string()
            }
        );
    }

    #[test]
    fn test_verify_archive_change_moved_to_archive() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure: change moved to archive
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&archive_path).unwrap();

        // Change doesn't exist in original location
        let result = verify_archive_completion(change_id, Some(base));
        assert!(result.is_success());
    }

    #[test]
    fn test_verify_archive_date_prefixed() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure with date-prefixed archive
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        let archive_path = archive_dir.join("2024-01-15-my-change");
        fs::create_dir(&archive_path).unwrap();

        let result = verify_archive_completion(change_id, Some(base));
        assert!(result.is_success());
    }

    #[test]
    fn test_verify_archive_change_removed() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure: neither change nor archive exists
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        // Neither path exists - considered success (change was removed)
        let result = verify_archive_completion(change_id, Some(base));
        assert!(result.is_success());
    }

    #[test]
    fn test_verify_archive_both_exist() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Edge case: both change and archive exist (incomplete archive)
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        let change_path = changes_dir.join(change_id);
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&change_path).unwrap();
        fs::create_dir(&archive_path).unwrap();

        // If change directory still exists, archive is incomplete regardless of archive entry
        let result = verify_archive_completion(change_id, Some(base));
        assert!(!result.is_success());
        assert_eq!(
            result,
            ArchiveVerificationResult::NotArchived {
                change_id: "my-change".to_string()
            }
        );
    }

    // ===========================
    // is_change_archived tests
    // ===========================

    #[test]
    fn test_is_change_archived_when_archive_only() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "archived-change";
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&archive_path).unwrap();

        assert!(is_change_archived(change_id, Some(base)));
    }

    #[test]
    fn test_is_change_archived_false_when_change_exists() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "active-change";
        let change_path = changes_dir.join(change_id);
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&change_path).unwrap();
        fs::create_dir(&archive_path).unwrap();

        assert!(!is_change_archived(change_id, Some(base)));
    }

    #[test]
    fn test_is_change_archived_false_without_archive_entry() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "missing-archive";
        assert!(!is_change_archived(change_id, Some(base)));
    }

    // ===========================
    // is_archive_commit_complete tests
    // ===========================

    #[tokio::test]
    async fn test_is_archive_commit_complete_when_clean() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Archive: change-a"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        let result = is_archive_commit_complete("change-a", Some(repo_root))
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_is_archive_commit_complete_false_when_dirty() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Archive: change-a"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        fs::write(repo_root.join("README.md"), "dirty").unwrap();

        let result = is_archive_commit_complete("change-a", Some(repo_root))
            .await
            .unwrap();
        assert!(!result);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ensure_archive_commit_retries_after_pre_commit() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        let archive_dir = repo_root.join("openspec/changes/archive/change-a");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("archive.txt"), "archived").unwrap();

        let hooks_dir = repo_root.join(".git/hooks");
        let hook_path = hooks_dir.join("pre-commit");
        let hook_contents = "#!/bin/sh\n\
if [ ! -f .git/hooks/pre-commit-ran ]; then\n\
  echo 'hooked' >> openspec/changes/archive/change-a/archive.txt\n\
  git add openspec/changes/archive/change-a/archive.txt\n\
  touch .git/hooks/pre-commit-ran\n\
  exit 1\n\
fi\n\
exit 0\n";
        fs::write(&hook_path, hook_contents).unwrap();
        let mut perms = fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms).unwrap();

        let resolver_script = repo_root.join("archive-resolver.sh");
        let script_contents = "#!/bin/sh\nset -e\n\
git add -A\n\
if ! git commit -m 'Archive: change-a'; then\n\
  git add -A\n\
  git commit -m 'Archive: change-a'\n\
fi\n";
        fs::write(&resolver_script, script_contents).unwrap();
        let mut perms = fs::metadata(&resolver_script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&resolver_script, perms).unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh archive-resolver.sh".to_string()),
            ..Default::default()
        };
        let agent = AgentRunner::new(config);

        ensure_archive_commit("change-a", repo_root, &agent, VcsBackend::Git, |_| async {})
            .await
            .unwrap();

        let result = is_archive_commit_complete("change-a", Some(repo_root))
            .await
            .unwrap();
        assert!(result);
    }

    // ===========================
    // verify_task_completion tests
    // ===========================

    #[test]
    fn test_verify_task_completion_all_complete() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "my-change";
        let tasks_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&tasks_dir).unwrap();

        let tasks_content = "# Tasks\n\n- [x] Task 1\n- [x] Task 2\n- [x] Task 3\n";
        fs::write(tasks_dir.join("tasks.md"), tasks_content).unwrap();

        let result = verify_task_completion(change_id, Some(base)).unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_task_completion_incomplete() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "my-change";
        let tasks_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&tasks_dir).unwrap();

        let tasks_content = "# Tasks\n\n- [x] Task 1\n- [ ] Task 2\n- [x] Task 3\n";
        fs::write(tasks_dir.join("tasks.md"), tasks_content).unwrap();

        let result = verify_task_completion(change_id, Some(base)).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_verify_task_completion_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "my-change";
        let tasks_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&tasks_dir).unwrap();

        let tasks_content = "# Tasks\n\nNo actual task checkboxes here.\n";
        fs::write(tasks_dir.join("tasks.md"), tasks_content).unwrap();

        // No tasks means not complete (0 total)
        let result = verify_task_completion(change_id, Some(base)).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_verify_task_completion_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "nonexistent-change";
        let result = verify_task_completion(change_id, Some(base));
        assert!(result.is_err());
    }

    // ===========================
    // get_task_progress tests
    // ===========================

    #[test]
    fn test_get_task_progress_exists() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "my-change";
        let tasks_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&tasks_dir).unwrap();

        let tasks_content = "# Tasks\n\n- [x] Task 1\n- [ ] Task 2\n- [x] Task 3\n";
        fs::write(tasks_dir.join("tasks.md"), tasks_content).unwrap();

        let progress = get_task_progress(change_id, Some(base)).unwrap().unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_get_task_progress_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "nonexistent-change";
        let progress = get_task_progress(change_id, Some(base)).unwrap();
        assert!(progress.is_none());
    }

    // ===========================
    // build_archive_error_message tests
    // ===========================

    #[test]
    fn test_build_archive_error_message() {
        let msg = build_archive_error_message("add-feature");
        assert!(msg.contains("add-feature"));
        assert!(msg.contains("not actually archived"));
        assert!(msg.contains("openspec/changes"));
    }

    // ===========================
    // Archive guardrail tests
    // ===========================

    #[tokio::test]
    async fn test_is_archive_commit_complete_false_when_change_exists() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create archive commit
        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Archive: test-change"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create openspec/changes/test-change directory (simulating archive reversion)
        let change_dir = repo_root.join("openspec/changes/test-change");
        fs::create_dir_all(&change_dir).unwrap();

        // Archive commit should be incomplete because change directory exists
        let result = is_archive_commit_complete("test-change", Some(repo_root))
            .await
            .unwrap();
        assert!(
            !result,
            "Archive commit should be incomplete when change directory exists"
        );
    }

    #[tokio::test]
    async fn test_is_archive_commit_complete_true_when_change_removed() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create archive commit
        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Archive: test-change"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Do NOT create openspec/changes/test-change directory (proper archive)

        // Archive commit should be complete
        let result = is_archive_commit_complete("test-change", Some(repo_root))
            .await
            .unwrap();
        assert!(
            result,
            "Archive commit should be complete when change directory does not exist"
        );
    }

    #[tokio::test]
    async fn test_ensure_archive_commit_fails_when_change_exists() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create openspec/changes/test-change directory (simulating incomplete archive)
        let change_dir = repo_root.join("openspec/changes/test-change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("spec.md"), "# Test change").unwrap();

        // Add files to working tree
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        let config = OrchestratorConfig::default();
        let agent = AgentRunner::new(config);

        // ensure_archive_commit should fail because change directory exists
        let result = ensure_archive_commit(
            "test-change",
            repo_root,
            &agent,
            VcsBackend::Git,
            |_| async {},
        )
        .await;

        assert!(
            result.is_err(),
            "ensure_archive_commit should fail when change directory exists"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("change directory still exists"),
            "Error message should mention change directory exists, got: {}",
            err_msg
        );
    }

    // ===========================
    // delete_change_directory tests
    // ===========================

    #[test]
    fn test_delete_change_directory_success() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create change directory with content
        let change_id = "test-change";
        let change_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "test content").unwrap();

        // Verify directory exists
        assert!(change_dir.exists());

        // Delete directory
        let result = delete_change_directory(change_id, Some(base));
        assert!(result.is_ok(), "Delete should succeed");

        // Verify directory is gone
        assert!(!change_dir.exists());
    }

    #[test]
    fn test_delete_change_directory_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "nonexistent-change";
        let changes_dir = base.join("openspec/changes");
        fs::create_dir_all(&changes_dir).unwrap();

        // Delete non-existent directory should succeed (idempotent)
        let result = delete_change_directory(change_id, Some(base));
        assert!(
            result.is_ok(),
            "Delete of non-existent directory should succeed"
        );
    }

    #[test]
    fn test_delete_change_directory_with_nested_content() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create change directory with nested structure
        let change_id = "nested-change";
        let change_dir = base.join("openspec/changes").join(change_id);
        let specs_dir = change_dir.join("specs");
        fs::create_dir_all(&specs_dir).unwrap();
        fs::write(change_dir.join("tasks.md"), "tasks").unwrap();
        fs::write(specs_dir.join("spec.md"), "spec content").unwrap();

        // Verify directory exists
        assert!(change_dir.exists());
        assert!(specs_dir.exists());

        // Delete directory
        let result = delete_change_directory(change_id, Some(base));
        assert!(result.is_ok(), "Delete should succeed");

        // Verify entire directory tree is gone
        assert!(!change_dir.exists());
        assert!(!specs_dir.exists());
    }
}
