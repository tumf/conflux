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
        module = module_path!(),
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

/// Check whether the current HEAD commit has no file changes.
pub async fn is_head_empty_commit<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--root",
            "HEAD",
        ],
        cwd,
    )
    .await?;
    Ok(output.trim().is_empty())
}

/// Create a WIP archive commit for a retry attempt.
pub async fn create_archive_wip_commit<P: AsRef<Path>>(
    cwd: P,
    change_id: &str,
    attempt: u32,
) -> VcsResult<()> {
    let message = format!("WIP(archive): {} (attempt#{})", change_id, attempt);
    run_git(&["add", "-A"], &cwd).await?;
    run_git(&["commit", "--allow-empty", "-m", &message], cwd).await?;
    Ok(())
}

/// Squash all archive WIP commits into a final Archive commit.
pub async fn squash_archive_wip_commits<P: AsRef<Path>>(cwd: P, change_id: &str) -> VcsResult<()> {
    let wip_pattern = format!("^WIP\\(archive\\): {} ", change_id);
    let wip_commits = run_git(
        &["rev-list", "--reverse", "--grep", &wip_pattern, "HEAD"],
        &cwd,
    )
    .await?;
    let first_wip = wip_commits
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| {
            VcsError::git_command(format!("No archive WIP commits found for {}", change_id))
        })?;

    let parent_revision = run_git(&["rev-parse", &format!("{}^", first_wip)], &cwd).await?;
    let parent_revision = parent_revision.trim();

    run_git(&["reset", "--soft", parent_revision], &cwd).await?;
    let archive_message = format!("Archive: {}", change_id);
    run_git(&["commit", "--allow-empty", "-m", &archive_message], cwd).await?;
    Ok(())
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
        module = module_path!(),
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

/// Check whether a merge is currently in progress.
///
/// Returns `Ok(true)` when `MERGE_HEAD` exists.
pub async fn is_merge_in_progress<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to check merge state: {}", e)))?;

    Ok(output.status.success())
}

/// Return any change_ids missing merge commits since base_revision.
///
/// A merge commit is recognized by the subject exactly matching `Merge change: <change_id>`.
pub async fn missing_merge_commits_since<P: AsRef<Path>>(
    cwd: P,
    base_revision: &str,
    change_ids: &[String],
) -> VcsResult<Vec<String>> {
    if change_ids.is_empty() {
        return Ok(Vec::new());
    }

    let output = run_git(
        &[
            "log",
            "--merges",
            "--format=%s",
            &format!("{}..HEAD", base_revision),
        ],
        cwd,
    )
    .await?;

    let merge_messages: Vec<&str> = output.lines().collect();
    let mut missing = Vec::new();
    for change_id in change_ids {
        let expected = format!("Merge change: {}", change_id);
        if !merge_messages.iter().any(|line| line.trim() == expected) {
            missing.push(change_id.clone());
        }
    }

    Ok(missing)
}

/// Find the most recent merge commit hash whose subject matches exactly.
///
/// Returns `Ok(None)` when no such merge commit exists in the given range.
pub async fn merge_commit_hash_by_subject_since<P: AsRef<Path>>(
    cwd: P,
    base_revision: &str,
    expected_subject: &str,
) -> VcsResult<Option<String>> {
    let output = run_git(
        &[
            "log",
            "--merges",
            "--format=%H\t%s",
            &format!("{}..HEAD", base_revision),
        ],
        cwd,
    )
    .await?;

    for line in output.lines().map(str::trim).filter(|s| !s.is_empty()) {
        let mut parts = line.splitn(2, '\t');
        let Some(hash) = parts.next() else {
            continue;
        };
        let subject = parts.next().unwrap_or("");
        if subject == expected_subject {
            return Ok(Some(hash.to_string()));
        }
    }

    Ok(None)
}

/// Return the first parent of a commit (e.g. the target branch state before a merge commit).
pub async fn first_parent_of<P: AsRef<Path>>(cwd: P, commit: &str) -> VcsResult<String> {
    run_git(&["rev-parse", &format!("{}^1", commit)], cwd).await
}

/// Check whether `ancestor` is an ancestor of `descendant`.
///
/// Returns `Ok(false)` when not an ancestor.
pub async fn is_ancestor<P: AsRef<Path>>(
    cwd: P,
    ancestor: &str,
    descendant: &str,
) -> VcsResult<bool> {
    let output = Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to execute git merge-base: {}", e)))?;

    Ok(output.status.success())
}

/// Return merge commit subjects that start with "Pre-sync base into" but do not match the
/// expected subject for this change.
///
/// This is used to validate pre-sync merge commit message conventions inside worktrees.
pub async fn presync_merge_subject_mismatches_since<P: AsRef<Path>>(
    cwd: P,
    base_revision: &str,
    change_id: &str,
) -> VcsResult<Vec<String>> {
    let expected = format!("Pre-sync base into {}", change_id);

    let output = run_git(
        &[
            "log",
            "--merges",
            "--format=%s",
            &format!("{}..HEAD", base_revision),
        ],
        cwd,
    )
    .await?;

    let mut mismatches = Vec::new();
    for subject in output.lines().map(str::trim).filter(|s| !s.is_empty()) {
        if subject.starts_with("Pre-sync base into") && subject != expected {
            mismatches.push(subject.to_string());
        }
    }

    Ok(mismatches)
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
    async fn test_presync_merge_subject_mismatches_since() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init.is_err() {
            // Skip if git not available
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

        std::fs::write(temp_dir.path().join("README.md"), "base\n").unwrap();
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_revision = run_git(&["rev-parse", "HEAD"], temp_dir.path())
            .await
            .unwrap();

        // Create worktree-like branch
        let _ = Command::new("git")
            .args(["checkout", "-b", "ws-change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Advance main
        let _ = Command::new("git")
            .args(["checkout", "main"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        std::fs::write(temp_dir.path().join("main.txt"), "main\n").unwrap();
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "Main advance"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Pre-sync merge on ws-change-a, but with wrong message
        let _ = Command::new("git")
            .args(["checkout", "ws-change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["merge", "--no-ff", "-m", "Pre-sync base into WRONG", "main"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let mismatches = presync_merge_subject_mismatches_since(
            temp_dir.path(),
            base_revision.trim(),
            "change-a",
        )
        .await
        .unwrap();

        assert!(
            mismatches.iter().any(|s| s == "Pre-sync base into WRONG"),
            "Expected mismatch to include wrong pre-sync subject"
        );
    }

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
            module = module_path!(),
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
            module = module_path!(),
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
            module = module_path!(),
            "Executing git command: git config user.email test@example.com (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
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
            module = module_path!(),
            "Executing git command: git add . (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
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
            module = module_path!(),
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
            module = module_path!(),
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

        debug!(
            module = module_path!(),
            "Executing git command: git config user.email test@example.com (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
            "Executing git command: git config user.name Test User (cwd: {:?})",
            temp_dir.path()
        );
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

        debug!(
            module = module_path!(),
            "Executing git command: git add . (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
            "Executing git command: git commit -m add changes (cwd: {:?})",
            temp_dir.path()
        );
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
