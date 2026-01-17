//! Workspace state detection module for OpenSpec Orchestrator.
//!
//! This module provides state detection logic for workspace resume operations,
//! enabling idempotent parallel execution. It detects the current state of a
//! workspace and determines the appropriate action to take during resume.
//!
//! # Workspace States
//!
//! 1. **Created**: Workspace created, no apply commits → Start apply
//! 2. **Applying**: WIP commits exist, apply in progress → Resume apply (next iteration)
//! 3. **Applied**: Apply complete, archive not complete → Archive only
//! 4. **Archived**: Archive complete, not merged to main → Merge only
//! 5. **Merged**: Merged to main → Skip & Cleanup
//!
//! # Example
//!
//! ```ignore
//! use crate::execution::state::{detect_workspace_state, WorkspaceState};
//!
//! let state = detect_workspace_state("add-feature", &workspace_path).await?;
//! match state {
//!     WorkspaceState::Created => { /* start apply */ }
//!     WorkspaceState::Applying { iteration } => { /* resume from iteration */ }
//!     WorkspaceState::Applied => { /* archive only */ }
//!     WorkspaceState::Archived => { /* merge only */ }
//!     WorkspaceState::Merged => { /* skip & cleanup */ }
//! }
//! ```

use std::path::Path;
use tokio::process::Command;
use tracing::debug;

use crate::error::{OrchestratorError, Result};
use crate::execution::archive::is_archive_commit_complete;

/// Workspace state for resume detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceState {
    /// Workspace created, no apply commits yet.
    Created,
    /// Apply in progress, WIP commits exist.
    /// The iteration number indicates the next iteration to resume from.
    Applying { iteration: u32 },
    /// Apply complete, archive not complete.
    Applied,
    /// Archive complete, not merged to main.
    Archived,
    /// Merged to main.
    Merged,
}

/// Check if a change has been merged to the base branch.
///
/// This function checks if the archive commit for the given change exists in
/// the base branch's commit history AND the change directory has been removed
/// from openspec/changes/.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
/// * `base_branch` - The base branch to check against
///
/// # Returns
///
/// * `Ok(true)` - Change has been merged to base branch and change directory removed
/// * `Ok(false)` - Change has not been merged to base branch or change directory still exists
/// * `Err` - Failed to check merge status
pub async fn is_merged_to_base(
    change_id: &str,
    repo_root: &Path,
    base_branch: &str,
) -> Result<bool> {
    // Check if the archive commit exists in main branch
    let expected_subject = format!("Archive: {}", change_id);

    // Get the merge-base between current HEAD and base branch
    let merge_base_output = Command::new("git")
        .args(["merge-base", "HEAD", base_branch])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to get merge-base: {}", e)))?;

    if !merge_base_output.status.success() {
        // If merge-base fails, base branch might not exist or we're on base branch
        // Check if we're on the base branch
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(repo_root)
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::GitCommand(format!("Failed to get current branch: {}", e))
            })?;

        if !branch_output.status.success() {
            return Ok(false);
        }

        let current_branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();
        if current_branch != base_branch {
            return Ok(false);
        }
    }

    // Search for the archive commit in base branch history
    let log_output = Command::new("git")
        .args([
            "log",
            base_branch,
            "--format=%s",
            "--all-match",
            "--grep",
            &expected_subject,
        ])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to search git log: {}", e)))?;

    if !log_output.status.success() {
        let stderr = String::from_utf8_lossy(&log_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to search git log: {}",
            stderr
        )));
    }

    let commits = String::from_utf8_lossy(&log_output.stdout);
    let archive_commit_found = commits.lines().any(|line| line.trim() == expected_subject);

    // Check if the changes directory still exists
    let change_path = repo_root.join("openspec/changes").join(change_id);
    let change_dir_exists = change_path.exists();

    debug!(
        change_id = %change_id,
        base_branch = %base_branch,
        expected_subject = %expected_subject,
        archive_commit_found = archive_commit_found,
        change_dir_exists = change_dir_exists,
        "is_merged_to_base: checking base branch and change directory"
    );

    // Only consider merged if archive commit exists AND change directory is gone
    if archive_commit_found && change_dir_exists {
        tracing::warn!(
            change_id = %change_id,
            change_path = %change_path.display(),
            "Archive commit found in base branch but change directory still exists at {}",
            change_path.display()
        );
        return Ok(false);
    }

    Ok(archive_commit_found && !change_dir_exists)
}

/// Get the latest WIP snapshot iteration number.
///
/// This function searches for WIP commit messages in the format
/// `WIP(apply): <change_id> (iteration N/M)` and returns the highest
/// iteration number found.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
///
/// # Returns
///
/// * `Ok(Some(n))` - WIP commits exist, highest iteration is n
/// * `Ok(None)` - No WIP commits found
/// * `Err` - Failed to check WIP commits
pub async fn get_latest_wip_snapshot(change_id: &str, repo_root: &Path) -> Result<Option<u32>> {
    let wip_prefix = format!("WIP(apply): {}", change_id);

    let log_output = Command::new("git")
        .args(["log", "--format=%s", "--grep", &wip_prefix, "--all-match"])
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

    let commits = String::from_utf8_lossy(&log_output.stdout);
    let mut max_iteration = None;

    // Parse WIP commit messages: "WIP(apply): <change_id> (iteration N/M)"
    for line in commits.lines() {
        if let Some(iteration_part) = line.strip_prefix(&wip_prefix) {
            // Extract iteration number from "(iteration N/M)"
            if let Some(captures) = iteration_part
                .trim()
                .strip_prefix("(iteration ")
                .and_then(|s| s.split_once('/'))
            {
                if let Ok(iteration) = captures.0.trim().parse::<u32>() {
                    max_iteration =
                        Some(max_iteration.map_or(iteration, |m: u32| m.max(iteration)));
                }
            }
        }
    }

    debug!(
        change_id = %change_id,
        max_iteration = ?max_iteration,
        "get_latest_wip_snapshot: found WIP commits"
    );

    Ok(max_iteration)
}

/// Check if an apply commit exists for a change.
///
/// An apply commit is a non-WIP commit that indicates apply completion.
/// This function checks for commits with the message `Apply: <change_id>`.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
///
/// # Returns
///
/// * `Ok(true)` - Apply commit exists
/// * `Ok(false)` - Apply commit does not exist
/// * `Err` - Failed to check apply commit
pub async fn has_apply_commit(change_id: &str, repo_root: &Path) -> Result<bool> {
    let expected_subject = format!("Apply: {}", change_id);

    let log_output = Command::new("git")
        .args([
            "log",
            "--format=%s",
            "--grep",
            &expected_subject,
            "--all-match",
        ])
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

    let commits = String::from_utf8_lossy(&log_output.stdout);
    let found = commits.lines().any(|line| line.trim() == expected_subject);

    debug!(
        change_id = %change_id,
        expected_subject = %expected_subject,
        found = found,
        "has_apply_commit: checking apply commit"
    );

    Ok(found)
}

/// Detect the current state of a workspace.
///
/// This function analyzes the workspace's git history and filesystem to determine
/// the current state for resume operations.
///
/// # State Detection Logic
///
/// 1. Check if merged to base branch → `Merged`
/// 2. Check if archive commit complete → `Archived`
/// 3. Check if apply commit exists → `Applied`
/// 4. Check for WIP commits → `Applying { iteration }`
/// 5. Otherwise → `Created`
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path (workspace path)
/// * `base_branch` - The base branch to check against
///
/// # Returns
///
/// * `Ok(WorkspaceState)` - Detected workspace state
/// * `Err` - Failed to detect state
pub async fn detect_workspace_state(
    change_id: &str,
    repo_root: &Path,
    base_branch: &str,
) -> Result<WorkspaceState> {
    // 1. Check if merged to base branch
    if is_merged_to_base(change_id, repo_root, base_branch).await? {
        debug!(change_id = %change_id, "State: Merged");
        return Ok(WorkspaceState::Merged);
    }

    // 2. Check if archive commit is complete
    if is_archive_commit_complete(change_id, Some(repo_root)).await? {
        debug!(change_id = %change_id, "State: Archived");
        return Ok(WorkspaceState::Archived);
    }

    // 3. Check if apply commit exists
    if has_apply_commit(change_id, repo_root).await? {
        debug!(change_id = %change_id, "State: Applied");
        return Ok(WorkspaceState::Applied);
    }

    // 4. Check for WIP commits
    if let Some(iteration) = get_latest_wip_snapshot(change_id, repo_root).await? {
        debug!(change_id = %change_id, iteration = iteration, "State: Applying");
        return Ok(WorkspaceState::Applying {
            iteration: iteration + 1,
        });
    }

    // 5. No commits found - workspace just created
    debug!(change_id = %change_id, "State: Created");
    Ok(WorkspaceState::Created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn init_git_repo(repo_root: &Path) {
        StdCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();
    }

    fn commit(repo_root: &Path, message: &str) {
        fs::write(repo_root.join("test.txt"), message).unwrap();
        StdCommand::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_root)
            .output()
            .unwrap();
    }

    #[tokio::test]
    async fn test_detect_workspace_state_created() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Created);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_applying() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "WIP(apply): test-change (iteration 1/5)");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Applying { iteration: 2 });
    }

    #[tokio::test]
    async fn test_detect_workspace_state_applied() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Apply: test-change");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Applied);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_archived() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch to simulate workspace
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-test-change"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        commit(repo_root, "Archive: test-change");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Archived);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_merged() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Archive: test-change");

        // We're on main, so the archive commit is in main
        // State should be Merged when change directory is gone
        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Merged);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_merged_with_remaining_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create changes directory before archiving
        let changes_dir = repo_root.join("openspec/changes/test-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Test Change").unwrap();

        commit(repo_root, "Archive: test-change");

        // Archive commit exists in main, but change directory still exists
        // With the new guardrails, this should NOT be considered Archived
        // because the archive is incomplete (change directory still exists)
        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();

        // Should be Created state (archive incomplete, no apply commit, no WIP)
        assert_eq!(state, WorkspaceState::Created);
    }

    #[tokio::test]
    async fn test_get_latest_wip_snapshot_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "WIP(apply): test-change (iteration 1/5)");
        commit(repo_root, "WIP(apply): test-change (iteration 2/5)");
        commit(repo_root, "WIP(apply): test-change (iteration 3/5)");

        let iteration = get_latest_wip_snapshot("test-change", repo_root)
            .await
            .unwrap();
        assert_eq!(iteration, Some(3));
    }

    #[tokio::test]
    async fn test_get_latest_wip_snapshot_none() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let iteration = get_latest_wip_snapshot("test-change", repo_root)
            .await
            .unwrap();
        assert_eq!(iteration, None);
    }

    #[tokio::test]
    async fn test_has_apply_commit_true() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Apply: test-change");

        let result = has_apply_commit("test-change", repo_root).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_has_apply_commit_false() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let result = has_apply_commit("test-change", repo_root).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_merged_to_base_true() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Archive: test-change");

        let result = is_merged_to_base("test-change", repo_root, "main")
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_is_merged_to_base_false_on_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature-branch"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        commit(repo_root, "Archive: test-change");

        let result = is_merged_to_base("test-change", repo_root, "main")
            .await
            .unwrap();
        assert!(!result);
    }
}
