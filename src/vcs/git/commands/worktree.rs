//! Git worktree operations.
//!
//! This module provides functions for creating, managing, and querying Git worktrees.

use super::basic::run_git;
use crate::vcs::{VcsError, VcsResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Classification of worktree add failures based on stderr output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeAddFailure {
    /// Path already exists (possibly stale worktree)
    PathExists,
    /// Branch is already checked out in another worktree
    BranchDuplicate,
    /// Branch already exists (but not checked out in any worktree)
    BranchExists,
    /// Invalid reference (base commit/branch doesn't exist)
    InvalidReference,
    /// Permission denied
    PermissionDenied,
    /// Unknown/other error
    Unknown,
}

impl WorktreeAddFailure {
    /// Classify the failure based on stderr output.
    pub fn classify(stderr: &str) -> Self {
        let stderr_lower = stderr.to_lowercase();

        // Check for branch duplicate first (more specific pattern)
        if stderr_lower.contains("is already checked out")
            || (stderr_lower.contains("already checked out") && stderr_lower.contains("at"))
        {
            Self::BranchDuplicate
        } else if stderr_lower.contains("a branch named") && stderr_lower.contains("already exists")
        {
            // Branch exists but not checked out in any worktree
            Self::BranchExists
        } else if stderr_lower.contains("already exists")
            || stderr_lower.contains("path already exists")
        {
            Self::PathExists
        } else if stderr_lower.contains("invalid reference")
            || stderr_lower.contains("not a valid")
            || stderr_lower.contains("unknown revision")
            || stderr_lower.contains("bad revision")
        {
            Self::InvalidReference
        } else if stderr_lower.contains("permission denied")
            || stderr_lower.contains("operation not permitted")
        {
            Self::PermissionDenied
        } else {
            Self::Unknown
        }
    }

    /// Get a human-readable description of the failure.
    pub fn description(&self) -> &'static str {
        match self {
            Self::PathExists => "path already exists (possibly stale worktree)",
            Self::BranchDuplicate => "branch is already checked out in another worktree",
            Self::BranchExists => "branch already exists (not checked out in any worktree)",
            Self::InvalidReference => "invalid reference (base commit/branch not found)",
            Self::PermissionDenied => "permission denied",
            Self::Unknown => "unknown error",
        }
    }
}

/// Create a new worktree at the specified path.
///
/// Creates a new branch with the given name based on the commit.
/// If the worktree add fails due to a stale path, attempts to prune and retry once.
pub async fn worktree_add<P: AsRef<Path>>(
    cwd: P,
    worktree_path: &str,
    branch_name: &str,
    base_commit: &str,
) -> VcsResult<()> {
    let cwd_ref = cwd.as_ref();
    debug!(
        "Creating worktree at {} with branch {} from {}",
        worktree_path, branch_name, base_commit
    );

    let result = run_git(
        &[
            "worktree",
            "add",
            worktree_path,
            "-b",
            branch_name,
            base_commit,
        ],
        cwd_ref,
    )
    .await;

    // If successful, return immediately
    if result.is_ok() {
        return Ok(());
    }

    // Classify the error and attempt retry if appropriate
    let err = result.unwrap_err();

    // Extract stderr from the error to classify the failure
    let (classification, should_retry) = match &err {
        VcsError::Command { stderr, .. } => {
            let stderr_str = stderr.as_deref().unwrap_or("");
            let classification = WorktreeAddFailure::classify(stderr_str);

            // Retry for PathExists or BranchExists failures
            let should_retry = classification == WorktreeAddFailure::PathExists
                || classification == WorktreeAddFailure::BranchExists;

            debug!(
                "Worktree add failed with classification: {:?} ({})",
                classification,
                classification.description()
            );

            (classification, should_retry)
        }
        _ => (WorktreeAddFailure::Unknown, false),
    };

    if !should_retry {
        // Not a retryable error, return the original error with classification info
        return Err(enhance_error_with_classification(err, classification));
    }

    // Handle BranchExists failure: try to attach the existing branch
    if classification == WorktreeAddFailure::BranchExists {
        // Check if branch is checked out in another worktree
        let is_checked_out = match is_branch_checked_out(cwd_ref, branch_name).await {
            Ok(checked_out) => checked_out,
            Err(e) => {
                warn!("Failed to check if branch is checked out: {}", e);
                return Err(enhance_error_with_classification(err, classification));
            }
        };

        if is_checked_out {
            // Branch is checked out elsewhere, can't attach
            debug!(
                "Branch {} is checked out in another worktree, cannot attach",
                branch_name
            );
            return Err(enhance_error_with_classification(err, classification));
        }

        // Branch exists but not checked out, try to attach it
        info!(
            "Branch {} exists but not checked out, attempting to attach existing branch",
            branch_name
        );

        let retry_result = run_git(&["worktree", "add", worktree_path, branch_name], cwd_ref).await;

        match retry_result {
            Ok(_) => {
                info!("Worktree add succeeded by attaching existing branch");
                return Ok(());
            }
            Err(retry_err) => {
                warn!("Failed to attach existing branch");
                return Err(enhance_error_with_retry_info(
                    err,
                    retry_err,
                    classification,
                ));
            }
        }
    }

    // Handle PathExists failure: check if path is stale
    let is_stale = match check_stale_worktree_path(cwd_ref, worktree_path).await {
        Ok(stale) => stale,
        Err(e) => {
            warn!("Failed to check if worktree path is stale: {}", e);
            false
        }
    };

    if !is_stale {
        // Path exists but is actually registered, don't retry
        return Err(enhance_error_with_classification(err, classification));
    }

    // Path is stale, attempt prune and retry
    info!(
        "Detected stale worktree path at {}, attempting prune and retry",
        worktree_path
    );

    if let Err(prune_err) = run_git(&["worktree", "prune"], cwd_ref).await {
        warn!("git worktree prune failed: {}", prune_err);
        return Err(enhance_error_with_classification(err, classification));
    }

    // After pruning, remove the stale directory
    let path_buf = PathBuf::from(worktree_path);
    if path_buf.exists() {
        if let Err(remove_err) = std::fs::remove_dir_all(&path_buf) {
            warn!(
                "Failed to remove stale directory {}: {}",
                worktree_path, remove_err
            );
            return Err(enhance_error_with_classification(err, classification));
        }
    }

    // Retry the worktree add
    let retry_result = run_git(
        &[
            "worktree",
            "add",
            worktree_path,
            "-b",
            branch_name,
            base_commit,
        ],
        cwd_ref,
    )
    .await;

    match retry_result {
        Ok(_) => {
            info!("Worktree add succeeded after prune");
            Ok(())
        }
        Err(retry_err) => {
            // Retry failed after prune. Check if the retry failure is due to the branch
            // already existing (but not checked out elsewhere). If so, fall through to
            // the safe existing-branch attach flow.
            let retry_classification = match &retry_err {
                VcsError::Command { stderr, .. } => {
                    let stderr_str = stderr.as_deref().unwrap_or("");
                    WorktreeAddFailure::classify(stderr_str)
                }
                _ => WorktreeAddFailure::Unknown,
            };

            if retry_classification == WorktreeAddFailure::BranchExists {
                info!(
                    "Stale-path retry failed with BranchExists, attempting safe existing-branch attach for '{}'",
                    branch_name
                );

                // Check if branch is checked out in another worktree
                let is_checked_out = match is_branch_checked_out(cwd_ref, branch_name).await {
                    Ok(checked_out) => checked_out,
                    Err(e) => {
                        warn!("Failed to check if branch is checked out: {}", e);
                        return Err(enhance_error_with_retry_info(
                            err,
                            retry_err,
                            classification,
                        ));
                    }
                };

                if is_checked_out {
                    debug!(
                        "Branch {} is checked out in another worktree, cannot attach after stale-path retry",
                        branch_name
                    );
                    return Err(enhance_error_with_retry_info(
                        err,
                        retry_err,
                        classification,
                    ));
                }

                // Branch exists but not checked out, try to attach it
                info!(
                    "Branch {} exists but not checked out after stale-path prune, attempting attach",
                    branch_name
                );

                let attach_result =
                    run_git(&["worktree", "add", worktree_path, branch_name], cwd_ref).await;

                match attach_result {
                    Ok(_) => {
                        info!("Worktree add succeeded by attaching existing branch after stale-path prune");
                        return Ok(());
                    }
                    Err(attach_err) => {
                        warn!("Failed to attach existing branch after stale-path prune");
                        return Err(enhance_error_with_retry_info(
                            err,
                            attach_err,
                            classification,
                        ));
                    }
                }
            }

            // Not a BranchExists retry failure, return combined error info
            warn!("Worktree add retry failed after prune");
            Err(enhance_error_with_retry_info(
                err,
                retry_err,
                classification,
            ))
        }
    }
}

/// Check if a worktree path is stale (exists but not registered).
async fn check_stale_worktree_path<P: AsRef<Path>>(cwd: P, worktree_path: &str) -> VcsResult<bool> {
    let path_buf = PathBuf::from(worktree_path);

    // If the path doesn't exist, it's not stale
    if !path_buf.exists() {
        return Ok(false);
    }

    // Get list of registered worktrees
    let worktrees = list_worktrees(cwd).await?;

    // Check if this path is in the registered list
    let normalized_path = path_buf.canonicalize().unwrap_or_else(|_| path_buf.clone());

    for (wt_path, _, _, _, _) in worktrees {
        let wt_path_buf = PathBuf::from(&wt_path);
        let normalized_wt = wt_path_buf.canonicalize().unwrap_or(wt_path_buf);

        if normalized_path == normalized_wt {
            // Path is registered, not stale
            return Ok(false);
        }
    }

    // Path exists but not registered - it's stale
    Ok(true)
}

/// Check if a branch is checked out in any worktree.
async fn is_branch_checked_out<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<bool> {
    let worktrees = list_worktrees(cwd).await?;

    for (_, _, branch, is_detached, _) in worktrees {
        if !is_detached && branch == branch_name {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Enhance error with classification information.
fn enhance_error_with_classification(
    err: VcsError,
    classification: WorktreeAddFailure,
) -> VcsError {
    match err {
        VcsError::Command {
            backend,
            message,
            command,
            working_dir,
            stderr,
            stdout,
        } => VcsError::Command {
            backend,
            message: format!(
                "{} (classified as: {})",
                message,
                classification.description()
            ),
            command,
            working_dir,
            stderr,
            stdout,
        },
        other => other,
    }
}

/// Enhance error with retry information.
fn enhance_error_with_retry_info(
    original_err: VcsError,
    retry_err: VcsError,
    classification: WorktreeAddFailure,
) -> VcsError {
    match (original_err, retry_err) {
        (
            VcsError::Command {
                backend,
                message: orig_msg,
                command,
                working_dir,
                stderr: orig_stderr,
                stdout: orig_stdout,
            },
            VcsError::Command {
                message: retry_msg,
                stderr: retry_stderr,
                ..
            },
        ) => VcsError::Command {
            backend,
            message: format!(
                "{} (classified as: {}). Retry after prune also failed: {}",
                orig_msg,
                classification.description(),
                retry_msg
            ),
            command,
            working_dir,
            stderr: Some(format!(
                "Original: {}\nRetry: {}",
                orig_stderr.unwrap_or_default(),
                retry_stderr.unwrap_or_default()
            )),
            stdout: orig_stdout,
        },
        (orig, _) => orig,
    }
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

    #[test]
    fn test_worktree_add_error_classification() {
        // Test PathExists
        let stderr = "fatal: '/path/to/worktree' already exists";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::PathExists
        );

        // Test BranchDuplicate
        let stderr = "fatal: 'my-branch' is already checked out at '/other/path'";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::BranchDuplicate
        );

        // Test BranchExists
        let stderr = "fatal: a branch named 'my-branch' already exists";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::BranchExists
        );

        // Test InvalidReference
        let stderr = "fatal: invalid reference: nonexistent-branch";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::InvalidReference
        );

        // Test PermissionDenied
        let stderr = "fatal: could not create worktree: Permission denied";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::PermissionDenied
        );

        // Test Unknown
        let stderr = "fatal: some other error";
        assert_eq!(
            WorktreeAddFailure::classify(stderr),
            WorktreeAddFailure::Unknown
        );
    }

    #[tokio::test]
    async fn test_worktree_add_retry_on_stale_path() {
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

        // Clean up the worktree registration, but keep the directory
        // This simulates a stale worktree
        let _ = Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                worktree_path.to_str().unwrap(),
            ])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Recreate the directory to simulate stale state
        std::fs::create_dir_all(&worktree_path).unwrap();

        // Verify the directory exists but is not registered
        assert!(worktree_path.exists());
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();
        let is_registered = worktrees
            .iter()
            .any(|(path, _, _, _, _)| *path == worktree_path.to_str().unwrap());
        assert!(!is_registered, "Worktree should not be registered");

        // Try to create a worktree at the same path
        // This should detect the stale path, prune, remove the directory, and retry successfully
        let result = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            "test-branch-2",
            "HEAD",
        )
        .await;

        // The retry should succeed after pruning and removing the stale directory
        if let Err(e) = &result {
            eprintln!("Retry failed with error: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Expected retry to succeed after prune and cleanup"
        );

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path.to_str().unwrap()).await;
    }

    #[tokio::test]
    async fn test_worktree_add_retry_preserves_error() {
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

        // Try to create a worktree with an invalid base commit
        let worktree_path = temp_dir.path().join("worktrees").join("test-wt");
        std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

        let result = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            "test-branch",
            "nonexistent-commit",
        )
        .await;

        // Should fail with InvalidReference classification
        assert!(result.is_err());
        let err = result.unwrap_err();

        match err {
            VcsError::Command { message, .. } => {
                // Error message should contain classification
                assert!(
                    message.contains("invalid reference") || message.contains("classified as"),
                    "Expected error message to contain classification info, got: {}",
                    message
                );
            }
            _ => panic!("Expected VcsError::Command"),
        }
    }

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
    async fn test_worktree_add_existing_branch_attach_success() {
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

        // Create a branch manually (not checked out in any worktree)
        let _ = Command::new("git")
            .args(["branch", "existing-branch"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Try to create a worktree with -b flag for a branch that already exists
        // This should fail initially but then succeed by attaching the existing branch
        let worktree_path = temp_dir.path().join("worktrees").join("test-wt");
        std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

        let result = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            "existing-branch",
            "HEAD",
        )
        .await;

        // Should succeed by attaching existing branch
        if let Err(e) = &result {
            eprintln!("Attach existing branch failed: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Expected to succeed by attaching existing branch"
        );

        // Verify worktree exists and is on the correct branch
        assert!(worktree_path.exists());

        let branch_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&worktree_path)
            .output()
            .await
            .unwrap();
        let current_branch = String::from_utf8_lossy(&branch_output.stdout);
        assert_eq!(current_branch.trim(), "existing-branch");

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path.to_str().unwrap()).await;
    }

    #[tokio::test]
    async fn test_worktree_add_existing_branch_attach_failure() {
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

        // Create a worktree with a branch (checked out)
        let worktree_path1 = temp_dir.path().join("worktrees").join("test-wt-1");
        std::fs::create_dir_all(worktree_path1.parent().unwrap()).unwrap();
        let _ = worktree_add(
            temp_dir.path(),
            worktree_path1.to_str().unwrap(),
            "existing-branch",
            "HEAD",
        )
        .await;

        // Try to create another worktree with the same branch name
        // This should fail because the branch is already checked out
        let worktree_path2 = temp_dir.path().join("worktrees").join("test-wt-2");
        std::fs::create_dir_all(worktree_path2.parent().unwrap()).unwrap();

        let result = worktree_add(
            temp_dir.path(),
            worktree_path2.to_str().unwrap(),
            "existing-branch",
            "HEAD",
        )
        .await;

        // Should fail because branch is already checked out
        assert!(
            result.is_err(),
            "Expected to fail when branch is already checked out"
        );

        // Verify error message contains classification
        if let Err(VcsError::Command { message, .. }) = &result {
            assert!(
                message.contains("branch") || message.contains("checked out"),
                "Expected error message to indicate branch is checked out, got: {}",
                message
            );
        }

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path1.to_str().unwrap()).await;
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

    /// Test: stale-path retry falls through to existing-branch attach when the
    /// branch exists but is NOT checked out in another worktree.
    ///
    /// Corresponds to spec scenario: "Stale path retry falls through to existing branch attach"
    #[tokio::test]
    async fn test_stale_path_retry_falls_through_to_existing_branch_attach() {
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

        let worktree_path = temp_dir.path().join("worktrees").join("stale-wt");
        std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

        // Step 1: Create the branch manually (so it exists as a local branch)
        let _ = Command::new("git")
            .args(["branch", "stale-branch"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Step 2: Create a stale directory at the worktree path (exists but not registered)
        std::fs::create_dir_all(&worktree_path).unwrap();

        // Verify preconditions:
        // - Directory exists
        assert!(worktree_path.exists());
        // - Not registered in worktree list
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();
        let is_registered = worktrees
            .iter()
            .any(|(path, _, _, _, _)| *path == worktree_path.to_string_lossy());
        assert!(!is_registered, "Worktree path should NOT be registered");
        // - Branch exists
        let branch_check = Command::new("git")
            .args(["show-ref", "--verify", "refs/heads/stale-branch"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        assert!(branch_check.status.success(), "Branch should exist");

        // Step 3: Try to create worktree with -b for the existing branch at the stale path.
        // The flow should be:
        //   1. Initial `git worktree add <path> -b stale-branch HEAD` → fails (path exists)
        //   2. Detect stale path → prune + remove directory
        //   3. Retry `git worktree add <path> -b stale-branch HEAD` → fails (branch exists)
        //   4. Fall through to safe attach → `git worktree add <path> stale-branch` → succeeds
        let result = worktree_add(
            temp_dir.path(),
            worktree_path.to_str().unwrap(),
            "stale-branch",
            "HEAD",
        )
        .await;

        if let Err(e) = &result {
            eprintln!(
                "Stale-path → existing-branch attach failed unexpectedly: {:?}",
                e
            );
        }
        assert!(
            result.is_ok(),
            "Expected stale-path retry to fall through to existing-branch attach"
        );

        // Verify the worktree is on the correct branch
        assert!(worktree_path.exists());
        let branch_output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(&worktree_path)
            .output()
            .await
            .unwrap();
        let current_branch = String::from_utf8_lossy(&branch_output.stdout);
        assert_eq!(current_branch.trim(), "stale-branch");

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_path.to_str().unwrap()).await;
    }

    /// Test: stale-path retry does NOT attach a branch that is already checked out
    /// in another worktree.
    ///
    /// Corresponds to spec scenario: "Stale path retry does not attach a checked-out branch"
    #[tokio::test]
    async fn test_stale_path_retry_does_not_attach_checked_out_branch() {
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

        // Step 1: Create a worktree that checks out the branch (so it's "in use")
        let worktree_existing = temp_dir.path().join("worktrees").join("existing-wt");
        std::fs::create_dir_all(worktree_existing.parent().unwrap()).unwrap();
        let create_result = worktree_add(
            temp_dir.path(),
            worktree_existing.to_str().unwrap(),
            "shared-branch",
            "HEAD",
        )
        .await;
        assert!(
            create_result.is_ok(),
            "Should succeed creating the first worktree"
        );

        // Step 2: Create a stale directory at a second worktree path
        let worktree_stale = temp_dir.path().join("worktrees").join("stale-wt-2");
        std::fs::create_dir_all(&worktree_stale).unwrap();

        // Verify preconditions:
        // - Stale directory exists but is not registered
        assert!(worktree_stale.exists());
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();
        let is_registered = worktrees
            .iter()
            .any(|(path, _, _, _, _)| *path == worktree_stale.to_string_lossy());
        assert!(
            !is_registered,
            "Stale worktree path should NOT be registered"
        );
        // - Branch is checked out in existing worktree
        let is_checked = is_branch_checked_out(temp_dir.path(), "shared-branch")
            .await
            .unwrap();
        assert!(
            is_checked,
            "Branch should be checked out in the first worktree"
        );

        // Step 3: Try to create a worktree with -b for the checked-out branch at the stale path.
        // Should fail: stale-path prune succeeds, but branch is checked out elsewhere.
        let result = worktree_add(
            temp_dir.path(),
            worktree_stale.to_str().unwrap(),
            "shared-branch",
            "HEAD",
        )
        .await;

        assert!(
            result.is_err(),
            "Expected failure because branch is checked out in another worktree"
        );

        // Verify the error contains classification info
        if let Err(VcsError::Command { message, .. }) = &result {
            assert!(
                message.contains("classified as") || message.contains("checked out"),
                "Expected classified error, got: {}",
                message
            );
        }

        // Cleanup
        let _ = worktree_remove(temp_dir.path(), worktree_existing.to_str().unwrap()).await;
    }
}
