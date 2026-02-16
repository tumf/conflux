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
use tracing::{debug, warn};

use crate::agent::{AgentRunner, OutputLine};
use crate::error::{OrchestratorError, Result};
use crate::hooks::HookContext;
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

/// Find the path to an archived change directory.
///
/// This function searches for the archive directory entry matching the change_id,
/// supporting both direct match (`{change_id}`) and date-prefixed format (`{date}-{change_id}`).
///
/// # Arguments
///
/// * `change_id` - The ID of the change to find
/// * `archive_dir` - Path to the archive directory
///
/// # Returns
///
/// * `Some(PathBuf)` - Path to the archived change directory if found
/// * `None` - Archive entry not found
fn find_archive_entry_path(change_id: &str, archive_dir: &Path) -> Option<std::path::PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    std::fs::read_dir(archive_dir)
        .ok()
        .and_then(|entries| {
            entries.filter_map(|e| e.ok()).find(|entry| {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                name_str == change_id || name_str.ends_with(&format!("-{}", change_id))
            })
        })
        .map(|entry| entry.path())
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
/// 2. The change directory does not exist in `openspec/changes/<change_id>`
/// 3. An archive entry exists in `openspec/changes/archive/`
///
/// This function uses file state only and does NOT check commit messages,
/// making it reliable for workspace resume scenarios.
pub async fn is_archive_commit_complete(change_id: &str, base_path: Option<&Path>) -> Result<bool> {
    let repo_root = base_path.unwrap_or_else(|| Path::new("."));

    // Check if working tree is clean
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

    // Check if openspec/changes/<change_id> exists (should NOT exist for complete archive)
    let change_path = repo_root.join("openspec/changes").join(change_id);
    let change_exists = change_path.exists();

    // Check if archive entry exists
    let archive_dir = repo_root.join("openspec/changes/archive");
    let archive_exists = archive_entry_exists(change_id, &archive_dir);

    debug!(
        change_id = %change_id,
        is_clean = is_clean,
        change_path = %change_path.display(),
        change_exists = change_exists,
        archive_dir = %archive_dir.display(),
        archive_exists = archive_exists,
        "is_archive_commit_complete: checking file state (clean={}, change_gone={}, archive_exists={})",
        is_clean,
        !change_exists,
        archive_exists
    );

    // Archive commit is complete when:
    // 1. Working tree is clean
    // 2. Change directory is gone
    // 3. Archive entry exists
    Ok(is_clean && !change_exists && archive_exists)
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
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
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
                    // Check if the directory exists at all
                    if !repo_root.exists() {
                        warn!(
                            "Workspace directory {:?} no longer exists (likely deleted by archive command), skipping archive commit creation",
                            repo_root
                        );
                        return Ok(());
                    }
                    return Err(OrchestratorError::GitCommand(format!(
                        "Git repository not found at {}",
                        repo_root.display()
                    )));
                }
                debug!(
                    "Workspace {:?} is not a Git repository (likely deleted by archive command), skipping archive commit creation",
                    repo_root
                );
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
                .run_resolve_streaming_in_dir_with_runner(&prompt, repo_root, ai_runner)
                .await?;

            while let Some(line) = rx.recv().await {
                handle_output(line).await;
            }

            let status = child.wait().await.map_err(|e| {
                OrchestratorError::AgentCommand(format!(
                    "Archive resolve command failed for change '{}' in workspace '{}': {}",
                    change_id,
                    repo_root.display(),
                    e
                ))
            })?;

            if !status.success() {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Archive resolve command failed for change '{}' in workspace '{}' with exit code: {:?}",
                    change_id,
                    repo_root.display(),
                    status.code()
                )));
            }

            if !is_archive_commit_complete(change_id, Some(repo_root)).await? {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Archive commit verification failed for change '{}' in workspace '{}'",
                    change_id,
                    repo_root.display()
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
#[cfg(test)]
pub fn delete_change_directory(change_id: &str, base_path: Option<&Path>) -> Result<()> {
    use tracing::info;

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
/// This function implements a fallback strategy when the primary tasks.md file
/// is not found in `openspec/changes/{change_id}/tasks.md`:
/// 1. First checks the primary location
/// 2. If not found, checks the archive directory for `openspec/changes/archive/{change_id}/tasks.md`
///    or `openspec/changes/archive/{date}-{change_id}/tasks.md`
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
/// * `Ok(None)` - Tasks file doesn't exist in either primary or archive location
/// * `Err` - Failed to parse tasks file
pub fn get_task_progress(
    change_id: &str,
    base_path: Option<&Path>,
) -> Result<Option<task_parser::TaskProgress>> {
    // Try primary location: openspec/changes/{change_id}/tasks.md
    let tasks_path = match base_path {
        Some(base) => base
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
        None => Path::new("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
    };

    if tasks_path.exists() {
        let progress = task_parser::parse_file(&tasks_path, Some(change_id))?;
        return Ok(Some(progress));
    }

    // Fallback: try archive location
    let archive_dir = match base_path {
        Some(base) => base.join("openspec/changes/archive"),
        None => Path::new("openspec/changes/archive").to_path_buf(),
    };

    if let Some(archive_entry_path) = find_archive_entry_path(change_id, &archive_dir) {
        let archive_tasks_path = archive_entry_path.join("tasks.md");
        if archive_tasks_path.exists() {
            debug!(
                change_id = %change_id,
                archive_tasks_path = %archive_tasks_path.display(),
                "get_task_progress: using archive fallback"
            );
            let progress = task_parser::parse_file(&archive_tasks_path, Some(change_id))?;
            return Ok(Some(progress));
        }
    }

    Ok(None)
}

/// Build an error message for failed archive verification.
///
/// Creates a descriptive error message when archive verification fails,
/// suitable for logging or sending to the user.
pub fn build_archive_error_message(change_id: &str, workspace_path: Option<&Path>) -> String {
    match workspace_path {
        Some(path) => format!(
            "Archive command succeeded but change '{}' in workspace '{}' was not actually archived. \
             The change directory still exists in openspec/changes/. \
             The archive command may not have executed 'openspec archive' correctly.",
            change_id,
            path.display()
        ),
        None => format!(
            "Archive command succeeded but change '{}' was not actually archived. \
             The change directory still exists in openspec/changes/. \
             The archive command may not have executed 'openspec archive' correctly.",
            change_id
        ),
    }
}

/// Event handler for archive loop events.
///
/// This trait allows the archive loop to send events to different handlers
/// (e.g., TUI event channel, CLI logger, parallel event bus).
#[allow(dead_code)]
pub trait ArchiveEventHandler {
    /// Called when archive iteration starts
    fn on_archive_started(&self, change_id: &str, command: &str);
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
    fn on_archive_started(&self, _change_id: &str, _command: &str) {}
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

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/change-a");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Archived").unwrap();

        // Commit the archive structure
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

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/change-a");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Archived").unwrap();

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

        // Make working tree dirty
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
        let agent = AgentRunner::new(config.clone());

        // Create AiCommandRunner for test
        use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
        use crate::command_queue::CommandQueueConfig;
        use crate::config::defaults::*;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);

        ensure_archive_commit(
            "change-a",
            repo_root,
            &agent,
            &ai_runner,
            VcsBackend::Git,
            |_| async {},
        )
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

    #[test]
    fn test_get_task_progress_archive_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "archived-change";
        // Create archive directory with tasks.md
        let archive_dir = base.join("openspec/changes/archive").join(change_id);
        fs::create_dir_all(&archive_dir).unwrap();

        let tasks_content = "# Tasks\n\n- [x] Task 1\n- [x] Task 2\n- [ ] Task 3\n";
        fs::write(archive_dir.join("tasks.md"), tasks_content).unwrap();

        // Primary location does not exist, should fall back to archive
        let progress = get_task_progress(change_id, Some(base)).unwrap().unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_get_task_progress_archive_fallback_date_prefixed() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "archived-change";
        // Create date-prefixed archive directory with tasks.md
        let archive_dir = base
            .join("openspec/changes/archive")
            .join("2024-01-15-archived-change");
        fs::create_dir_all(&archive_dir).unwrap();

        let tasks_content = "# Tasks\n\n- [x] Task 1\n- [x] Task 2\n- [x] Task 3\n";
        fs::write(archive_dir.join("tasks.md"), tasks_content).unwrap();

        // Primary location does not exist, should fall back to date-prefixed archive
        let progress = get_task_progress(change_id, Some(base)).unwrap().unwrap();
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_get_task_progress_primary_takes_precedence() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "both-locations";

        // Create primary location with tasks.md
        let primary_dir = base.join("openspec/changes").join(change_id);
        fs::create_dir_all(&primary_dir).unwrap();
        let primary_tasks = "# Tasks\n\n- [x] Task 1\n- [ ] Task 2\n";
        fs::write(primary_dir.join("tasks.md"), primary_tasks).unwrap();

        // Create archive location with different tasks.md
        let archive_dir = base.join("openspec/changes/archive").join(change_id);
        fs::create_dir_all(&archive_dir).unwrap();
        let archive_tasks = "# Tasks\n\n- [x] Task 1\n- [x] Task 2\n- [x] Task 3\n";
        fs::write(archive_dir.join("tasks.md"), archive_tasks).unwrap();

        // Should use primary location (1/2 tasks), not archive (3/3 tasks)
        let progress = get_task_progress(change_id, Some(base)).unwrap().unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);
    }

    #[test]
    fn test_get_task_progress_archive_without_tasks_md() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let change_id = "archived-no-tasks";
        // Create archive directory without tasks.md
        let archive_dir = base.join("openspec/changes/archive").join(change_id);
        fs::create_dir_all(&archive_dir).unwrap();

        // Should return None when archive exists but has no tasks.md
        let progress = get_task_progress(change_id, Some(base)).unwrap();
        assert!(progress.is_none());
    }

    // ===========================
    // build_archive_error_message tests
    // ===========================

    #[test]
    fn test_build_archive_error_message() {
        let msg = build_archive_error_message("add-feature", None);
        assert!(msg.contains("add-feature"));
        assert!(msg.contains("not actually archived"));
        assert!(msg.contains("openspec/changes"));

        let msg_with_path = build_archive_error_message("add-feature", Some(Path::new("/tmp/ws")));
        assert!(msg_with_path.contains("add-feature"));
        assert!(msg_with_path.contains("in workspace '/tmp/ws'"));
        assert!(msg_with_path.contains("not actually archived"));
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

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Archived").unwrap();

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

        // Create openspec/changes/test-change directory (simulating archive reversion or incomplete archive)
        let change_dir = repo_root.join("openspec/changes/test-change");
        fs::create_dir_all(&change_dir).unwrap();

        // Archive commit should be incomplete because change directory still exists
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

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Archived").unwrap();

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
            "Archive commit should be complete when change directory does not exist and archive entry exists"
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
        let agent = AgentRunner::new(config.clone());

        // Create AiCommandRunner for test
        use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
        use crate::command_queue::CommandQueueConfig;
        use crate::config::defaults::*;
        use std::sync::Arc;
        use tokio::sync::Mutex;
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);

        // ensure_archive_commit should fail because change directory exists
        let result = ensure_archive_commit(
            "test-change",
            repo_root,
            &agent,
            &ai_runner,
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
