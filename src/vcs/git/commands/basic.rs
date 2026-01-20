//! Basic Git command operations.
//!
//! This module provides fundamental Git operations such as running commands,
//! checking repository status, and managing branches.

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

/// Get list of changed files between two commits.
/// Returns a list of file paths that changed between `from_commit` and `to_commit`.
/// If `from_commit` is None, returns all files in `to_commit`.
pub async fn get_changed_files<P: AsRef<Path>>(
    cwd: P,
    from_commit: Option<&str>,
    to_commit: &str,
) -> VcsResult<Vec<String>> {
    let output = if let Some(from) = from_commit {
        run_git(
            &["diff", "--name-only", &format!("{}..{}", from, to_commit)],
            cwd,
        )
        .await?
    } else {
        run_git(&["ls-tree", "--name-only", "-r", to_commit], cwd).await?
    };

    Ok(output
        .lines()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect())
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

/// Check if the working directory is clean (no uncommitted changes).
///
/// Returns true if working directory is clean, false otherwise.
pub async fn is_working_directory_clean<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    Ok(output.trim().is_empty())
}

/// Checkout a branch.
pub async fn checkout<P: AsRef<Path>>(cwd: P, branch_or_commit: &str) -> VcsResult<()> {
    run_git(&["checkout", branch_or_commit], cwd).await?;
    Ok(())
}

/// Delete a branch.
pub async fn branch_delete<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<()> {
    debug!("Deleting branch {}", branch_name);
    run_git(&["branch", "-D", branch_name], cwd).await?;
    Ok(())
}

/// Check if a branch exists.
pub async fn branch_exists<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<bool> {
    let output = Command::new("git")
        .args([
            "show-ref",
            "--verify",
            &format!("refs/heads/{}", branch_name),
        ])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to check branch existence: {}", e)))?;

    Ok(output.status.success())
}

/// Generate a unique branch name with the given prefix and random suffix.
/// Retries with new random values if the branch already exists.
pub async fn generate_unique_branch_name<P: AsRef<Path>>(
    cwd: P,
    prefix: &str,
    max_attempts: u32,
) -> VcsResult<String> {
    use rand::Rng;

    for _ in 0..max_attempts {
        // Generate 6-character random hex string
        let random_suffix: String = (0..6)
            .map(|_| format!("{:x}", rand::thread_rng().gen_range(0..16)))
            .collect();
        let branch_name = format!("{}-{}", prefix, random_suffix);

        if !branch_exists(&cwd, &branch_name).await? {
            return Ok(branch_name);
        }

        debug!("Branch '{}' already exists, retrying...", branch_name);
    }

    Err(VcsError::git_command(format!(
        "Failed to generate unique branch name after {} attempts",
        max_attempts
    )))
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
    async fn test_generate_unique_branch_name_oso_session() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init.is_err() {
            return; // Skip if git not available
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

        // Create initial commit
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Generate unique branch name with oso-session prefix
        let branch_name = generate_unique_branch_name(temp_dir.path(), "oso-session", 10)
            .await
            .unwrap();

        // Verify format: oso-session-<6 hex chars>
        assert!(branch_name.starts_with("oso-session-"));
        let suffix = &branch_name["oso-session-".len()..];
        assert_eq!(suffix.len(), 6);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));

        // Verify branch doesn't exist yet
        let exists = branch_exists(temp_dir.path(), &branch_name).await.unwrap();
        assert!(!exists);

        // Create the branch and verify collision avoidance
        let _ = Command::new("git")
            .args(["branch", &branch_name])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Generate another branch - should be different
        let branch_name2 = generate_unique_branch_name(temp_dir.path(), "oso-session", 10)
            .await
            .unwrap();
        assert_ne!(branch_name, branch_name2);
    }
}
