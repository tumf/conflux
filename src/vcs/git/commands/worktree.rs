//! Git worktree operations.
//!
//! This module provides functions for creating, managing, and querying Git worktrees.

use super::basic::run_git;
use crate::vcs::{VcsError, VcsResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

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

/// Create a new detached worktree at the specified path.
///
/// DEPRECATED: Use worktree_add with a branch name instead to avoid detached HEAD state.
#[allow(dead_code)]
pub async fn worktree_add_detached<P: AsRef<Path>>(
    cwd: P,
    worktree_path: &str,
    base_commit: &str,
) -> VcsResult<()> {
    debug!(
        "Creating detached worktree at {} from {}",
        worktree_path, base_commit
    );
    run_git(
        &["worktree", "add", "--detach", worktree_path, base_commit],
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

/// List all worktrees with detailed information.
///
/// Parses the porcelain format output from `git worktree list --porcelain`.
/// Returns a Vec of tuples: (path, head, branch, is_detached, is_main)
pub async fn list_worktrees<P: AsRef<Path>>(
    cwd: P,
) -> VcsResult<Vec<(String, String, String, bool, bool)>> {
    let output = run_git(&["worktree", "list", "--porcelain"], &cwd).await?;

    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_head: Option<String> = None;
    let mut current_branch: Option<String> = None;
    let mut is_detached = false;
    let mut is_first = true; // First worktree is always the main one

    for line in output.lines() {
        let line = line.trim();

        if line.is_empty() {
            // Empty line signals end of current worktree entry
            if let (Some(path), Some(head)) = (current_path.take(), current_head.take()) {
                let branch = current_branch.take().unwrap_or_default();
                worktrees.push((path, head, branch, is_detached, is_first));
                is_first = false;
                is_detached = false;
            }
        } else if let Some(stripped) = line.strip_prefix("worktree ") {
            current_path = Some(stripped.to_string());
        } else if let Some(stripped) = line.strip_prefix("HEAD ") {
            current_head = Some(stripped.to_string());
        } else if let Some(stripped) = line.strip_prefix("branch ") {
            current_branch = Some(stripped.trim_start_matches("refs/heads/").to_string());
        } else if line == "detached" {
            is_detached = true;
        }
    }

    // Handle the last entry if there's no trailing newline
    if let (Some(path), Some(head)) = (current_path, current_head) {
        let branch = current_branch.unwrap_or_default();
        worktrees.push((path, head, branch, is_detached, is_first));
    }

    Ok(worktrees)
}

/// Check if a path is a git worktree (not the main repository).
///
/// A worktree is considered valid if:
/// 1. The path is listed in `git worktree list --porcelain` output
/// 2. The path is NOT the first worktree (main repository)
///
/// This is used to prevent parallel apply operations from running in the base repository,
/// which would pollute the working tree with unintended changes.
///
/// # Arguments
/// * `repo_root` - Repository root directory (main worktree)
/// * `path` - Path to check if it's a worktree
///
/// # Returns
/// * `Ok(true)` if the path is a valid worktree (not the main repository)
/// * `Ok(false)` if the path is the main repository or not a worktree
/// * `Err(VcsError)` if git command fails
pub async fn is_worktree<P1: AsRef<Path>, P2: AsRef<Path>>(
    repo_root: P1,
    path: P2,
) -> VcsResult<bool> {
    let path = path.as_ref();
    let worktrees = list_worktrees(repo_root).await?;

    // Normalize the path for comparison
    let normalized_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    for (worktree_path, _head, _branch, _is_detached, is_main) in worktrees {
        let worktree_path_buf = PathBuf::from(&worktree_path);
        let normalized_worktree = worktree_path_buf
            .canonicalize()
            .unwrap_or(worktree_path_buf);

        if normalized_path == normalized_worktree {
            // Found the path in worktree list
            // Return true only if it's NOT the main worktree
            return Ok(!is_main);
        }
    }

    // Path not found in worktree list
    Ok(false)
}

/// Count commits ahead of base branch.
///
/// Returns the number of commits that `worktree_branch` has ahead of `base_branch`.
/// Uses `git rev-list --count <base>..<worktree_branch>` to get the count.
///
/// # Arguments
/// * `cwd` - Working directory (can be worktree or main repo)
/// * `base_branch` - Base branch name (e.g., "main", "master")
/// * `worktree_branch` - Worktree branch name
///
/// # Returns
/// The number of commits ahead, or 0 if branches are at the same commit or on error.
pub async fn count_commits_ahead<P: AsRef<Path>>(
    cwd: P,
    base_branch: &str,
    worktree_branch: &str,
) -> VcsResult<usize> {
    let range = format!("{}..{}", base_branch, worktree_branch);
    let output = run_git(&["rev-list", "--count", &range], cwd).await?;
    let count = output
        .trim()
        .parse::<usize>()
        .map_err(|e| VcsError::git_command(format!("Invalid count: {}", e)))?;
    Ok(count)
}

/// Execute the worktree setup script if it exists.
///
/// Checks for `.wt/setup` in the repository root and executes it in the worktree directory.
/// Sets the `ROOT_WORKTREE_PATH` environment variable to the repository root path.
///
/// # Arguments
/// * `repo_root` - Path to the repository root directory
/// * `worktree_path` - Path to the newly created worktree directory
///
/// # Returns
/// Ok(()) if setup script doesn't exist or executes successfully, Err() if setup script fails.
pub async fn run_worktree_setup<P1: AsRef<Path>, P2: AsRef<Path>>(
    repo_root: P1,
    worktree_path: P2,
) -> VcsResult<()> {
    let repo_root = repo_root.as_ref();
    let worktree_path = worktree_path.as_ref();

    // Check if .wt/setup exists in the repository root
    let setup_script = repo_root.join(".wt").join("setup");

    if !setup_script.exists() {
        debug!(
            "Setup script not found at {:?}, skipping setup",
            setup_script
        );
        return Ok(());
    }

    info!(
        "Found setup script at {:?}, executing in worktree {:?}",
        setup_script, worktree_path
    );

    // Make sure the script is executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&setup_script).map_err(|e| {
            VcsError::git_command(format!("Failed to read setup script metadata: {}", e))
        })?;
        let mut permissions = metadata.permissions();
        // Add execute permission for owner, group, and others
        permissions.set_mode(permissions.mode() | 0o111);
        std::fs::set_permissions(&setup_script, permissions).map_err(|e| {
            VcsError::git_command(format!("Failed to set setup script permissions: {}", e))
        })?;
    }

    // Execute the setup script
    debug!(
        module = module_path!(),
        "Executing setup script: {:?} (cwd: {:?}, env: ROOT_WORKTREE_PATH={:?})",
        setup_script,
        worktree_path,
        repo_root
    );

    let output = Command::new(&setup_script)
        .current_dir(worktree_path)
        .env("ROOT_WORKTREE_PATH", repo_root)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to execute setup script: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let exit_code = output.status.code().unwrap_or(-1);

        return Err(VcsError::git_command(format!(
            "Setup script failed with exit code {}\nstdout: {}\nstderr: {}",
            exit_code, stdout, stderr
        )));
    }

    info!("Setup script completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_worktree_add_with_oso_session_branch() {
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

        // Generate unique branch name
        use crate::vcs::git::commands::basic::generate_unique_branch_name;
        let branch_name = generate_unique_branch_name(temp_dir.path(), "oso-session", 10)
            .await
            .unwrap();

        // Create worktree with the oso-session branch
        let worktree_path = temp_dir.path().join("worktrees").join(&branch_name);
        std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

        let result = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            &branch_name,
            "HEAD",
        )
        .await;
        assert!(result.is_ok());

        // Verify worktree exists
        assert!(worktree_path.exists());

        // Verify branch exists and is not detached
        let branch_check = Command::new("git")
            .args([
                "show-ref",
                "--verify",
                &format!("refs/heads/{}", branch_name),
            ])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        assert!(branch_check.status.success());

        // Verify worktree is on the correct branch (not detached)
        let branch_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&worktree_path)
            .output()
            .await
            .unwrap();
        let current_branch = String::from_utf8_lossy(&branch_output.stdout);
        assert_eq!(current_branch.trim(), branch_name);
        assert_ne!(current_branch.trim(), "HEAD"); // Not detached

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path.to_str().unwrap()).await;
    }

    #[tokio::test]
    async fn test_list_worktrees() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init_result.is_err() {
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

        // List worktrees (should have only main)
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();
        assert_eq!(worktrees.len(), 1);
        let (path, _head, branch, is_detached, is_main) = &worktrees[0];
        assert!(path.contains(temp_dir.path().to_str().unwrap()));
        assert_eq!(branch, "main");
        assert!(!is_detached);
        assert!(is_main);

        // Create a worktree
        let worktree_path = temp_dir.path().join("worktrees").join("test-wt");
        std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();
        let _ = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            "test-branch",
            "HEAD",
        )
        .await;

        // List worktrees again
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();
        assert_eq!(worktrees.len(), 2);

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path.to_str().unwrap()).await;
    }
}
