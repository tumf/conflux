//! Git command execution helpers.
//!
//! This module provides utilities for running Git commands,
//! built on top of the common VCS command helpers.

use crate::vcs::commands::{check_vcs_available, run_vcs_command, run_vcs_command_ignore_error};
use crate::vcs::{VcsBackend, VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// Execute a Git command and return the trimmed stdout output.
///
/// # Arguments
/// * `args` - Arguments to pass to git
/// * `cwd` - Working directory for the command
///
/// # Returns
/// The trimmed stdout output on success, or an error if the command fails.
pub async fn run_git<P: AsRef<Path>>(args: &[&str], cwd: P) -> VcsResult<String> {
    run_vcs_command("git", args, cwd, VcsBackend::Git).await
}

/// Execute a Git command without capturing output (fire-and-forget).
///
/// Returns Ok(()) on success, error on failure.
#[allow(dead_code)]
pub async fn run_git_silent<P: AsRef<Path>>(args: &[&str], cwd: P) -> VcsResult<()> {
    crate::vcs::commands::run_vcs_command_silent("git", args, cwd, VcsBackend::Git).await
}

/// Execute a Git command, ignoring errors.
///
/// Useful for cleanup operations where failure is acceptable.
#[allow(dead_code)]
pub async fn run_git_ignore_error<P: AsRef<Path>>(args: &[&str], cwd: P) {
    run_vcs_command_ignore_error("git", args, cwd).await;
}

/// Check if Git is available and the directory is a Git repository.
#[allow(dead_code)]
pub async fn check_git_repo<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    // Check git --version
    if !check_vcs_available("git", cwd.as_ref()).await? {
        return Ok(false);
    }

    // Check if directory is a git repo
    debug!(
        "Executing git command: git rev-parse --git-dir (cwd: {:?})",
        cwd.as_ref()
    );
    let root_result = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;

    match root_result {
        Ok(out) if out.status.success() => Ok(true),
        _ => Ok(false),
    }
}

/// Check if the working directory has uncommitted changes or untracked files.
///
/// Returns a tuple of (has_changes, status_output) where:
/// - has_changes: true if there are uncommitted changes or untracked files
/// - status_output: the output from `git status --porcelain` for error messages
pub async fn has_uncommitted_changes<P: AsRef<Path>>(cwd: P) -> VcsResult<(bool, String)> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    let has_changes = !output.is_empty();
    Ok((has_changes, output))
}

/// Get the current commit hash (HEAD).
pub async fn get_current_commit<P: AsRef<Path>>(cwd: P) -> VcsResult<String> {
    run_git(&["rev-parse", "HEAD"], cwd).await
}

/// List change IDs from the HEAD commit tree.
///
/// Reads directories under `openspec/changes` in the HEAD tree and filters out
/// archive and hidden entries.
pub async fn list_changes_in_head<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let output = run_git(
        &["ls-tree", "-d", "--name-only", "HEAD:openspec/changes"],
        cwd,
    )
    .await?;

    let mut change_ids: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|name| *name != "archive" && !name.starts_with('.'))
        .map(String::from)
        .collect();

    change_ids.sort();
    Ok(change_ids)
}

/// Get the current branch name.
/// Returns None if in detached HEAD state.
pub async fn get_current_branch<P: AsRef<Path>>(cwd: P) -> VcsResult<Option<String>> {
    let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"], cwd).await?;
    if branch == "HEAD" {
        // Detached HEAD state
        Ok(None)
    } else {
        Ok(Some(branch))
    }
}

/// Get git status output.
pub async fn get_status<P: AsRef<Path>>(cwd: P) -> VcsResult<String> {
    run_git(&["status"], cwd).await
}

/// Get list of conflicted files.
pub async fn get_conflict_files<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let output = run_git(&["diff", "--name-only", "--diff-filter=U"], cwd).await?;
    Ok(output
        .lines()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect())
}

/// Create a new worktree at the specified path.
///
/// Creates a new branch with the given name based on the commit.
pub async fn worktree_add<P: AsRef<Path>>(
    cwd: P,
    worktree_path: &str,
    branch_name: &str,
    base_commit: &str,
) -> VcsResult<()> {
    debug!(
        "Creating worktree at {} with branch {} from {}",
        worktree_path, branch_name, base_commit
    );
    run_git(
        &[
            "worktree",
            "add",
            worktree_path,
            "-b",
            branch_name,
            base_commit,
        ],
        cwd,
    )
    .await?;
    Ok(())
}

/// Remove a worktree.
pub async fn worktree_remove<P: AsRef<Path>>(cwd: P, worktree_path: &str) -> VcsResult<()> {
    debug!("Removing worktree at {}", worktree_path);
    run_git(&["worktree", "remove", worktree_path, "--force"], cwd).await?;
    Ok(())
}

/// Delete a branch.
pub async fn branch_delete<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<()> {
    debug!("Deleting branch {}", branch_name);
    run_git(&["branch", "-D", branch_name], cwd).await?;
    Ok(())
}

/// Checkout a branch.
pub async fn checkout<P: AsRef<Path>>(cwd: P, branch_or_commit: &str) -> VcsResult<()> {
    run_git(&["checkout", branch_or_commit], cwd).await?;
    Ok(())
}

/// Merge a branch into the current branch.
///
/// Returns Ok(()) on success, or GitConflict error if there are conflicts.
pub async fn merge<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<()> {
    debug!(
        "Executing git command: git merge {} --no-edit (cwd: {:?})",
        branch_name,
        cwd.as_ref()
    );
    let output = Command::new("git")
        .args(["merge", branch_name, "--no-edit"])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to execute git merge: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}\n{}", stdout, stderr);

        // Check for merge conflicts
        if combined.contains("CONFLICT") || combined.contains("Automatic merge failed") {
            return Err(VcsError::git_conflict(combined.to_string()));
        }

        return Err(VcsError::git_command(format!(
            "git merge {} failed: {}",
            branch_name, combined
        )));
    }

    Ok(())
}

/// Abort a merge in progress.
#[allow(dead_code)]
pub async fn merge_abort<P: AsRef<Path>>(cwd: P) -> VcsResult<()> {
    run_git(&["merge", "--abort"], cwd).await?;
    Ok(())
}

/// Stage all changes and commit with the given message.
#[allow(dead_code)]
pub async fn add_and_commit<P: AsRef<Path>>(cwd: P, message: &str) -> VcsResult<()> {
    run_git(&["add", "-A"], &cwd).await?;
    run_git(&["commit", "-m", message], cwd).await?;
    Ok(())
}

/// Check if there are staged or unstaged changes to commit.
#[allow(dead_code)]
pub async fn has_changes_to_commit<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    Ok(!output.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_check_git_repo_non_repo() {
        let temp_dir = TempDir::new().unwrap();
        // Non-git directory should return false (not error)
        let result = check_git_repo(temp_dir.path()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_check_git_repo_initialized() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        debug!(
            "Executing git command: git init (cwd: {:?})",
            temp_dir.path()
        );
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            // Skip test if git is not available
            return;
        }

        let result = check_git_repo(temp_dir.path()).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_has_uncommitted_changes_clean() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        debug!(
            "Executing git command: git init (cwd: {:?})",
            temp_dir.path()
        );
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            return;
        }

        // Configure git user for commit
        debug!(
            "Executing git command: git config user.email test@example.com (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            "Executing git command: git config user.name Test User (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create and commit a file
        std::fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        debug!(
            "Executing git command: git add . (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            "Executing git command: git commit -m initial (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let result = has_uncommitted_changes(temp_dir.path()).await;
        assert!(result.is_ok());
        let (has_changes, _) = result.unwrap();
        assert!(!has_changes);
    }

    #[tokio::test]
    async fn test_has_uncommitted_changes_dirty() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        debug!(
            "Executing git command: git init (cwd: {:?})",
            temp_dir.path()
        );
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            return;
        }

        // Create an uncommitted file
        std::fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let result = has_uncommitted_changes(temp_dir.path()).await;
        assert!(result.is_ok());
        let (has_changes, output) = result.unwrap();
        assert!(has_changes);
        assert!(output.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_list_changes_in_head_filters_special_dirs() {
        let temp_dir = TempDir::new().unwrap();

        debug!(
            "Executing git command: git init (cwd: {:?})",
            temp_dir.path()
        );
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            return;
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::create_dir_all(base_dir.join("archive")).unwrap();
        std::fs::create_dir_all(base_dir.join(".hidden")).expect("create hidden dir");
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("change-b").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("archive").join("proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add changes"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let changes = list_changes_in_head(temp_dir.path()).await.unwrap();
        assert_eq!(
            changes,
            vec!["change-a".to_string(), "change-b".to_string()]
        );
    }
}
