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

use crate::error::{OrchestratorError, Result};
use crate::task_parser;
use std::path::Path;
use tokio::process::Command;
use tracing::debug;

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
/// The archive commit is considered complete when the working tree is clean
/// and the latest commit subject matches `Archive: <change_id>`.
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

    debug!(
        change_id = %change_id,
        is_clean = is_clean,
        subject = %subject,
        expected_subject = %expected_subject,
        "is_archive_commit_complete: checking commit state"
    );

    Ok(is_clean && subject == expected_subject)
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

    // Archive is successful if:
    // 1. Change no longer exists in original location, OR
    // 2. Change exists in archive directory
    // We check archive_exists first because the change may briefly exist in both locations
    if archive_exists || !change_exists {
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

    let progress = task_parser::parse_file(&tasks_path)?;

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

    let progress = task_parser::parse_file(&tasks_path)?;
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

#[cfg(test)]
mod tests {
    use super::*;
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

        // Edge case: both change and archive exist (transient state)
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";
        let change_path = changes_dir.join(change_id);
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&change_path).unwrap();
        fs::create_dir(&archive_path).unwrap();

        // If archive exists, consider it success even if change also exists
        let result = verify_archive_completion(change_id, Some(base));
        assert!(result.is_success());
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
}
