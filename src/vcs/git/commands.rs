//! Git command execution helpers.
//!
//! This module provides utilities for running Git commands,
//! built on top of the common VCS command helpers.

use crate::vcs::commands::{check_vcs_available, run_vcs_command, run_vcs_command_ignore_error};
use crate::vcs::{VcsBackend, VcsError, VcsResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

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
    let wip_pattern = format!("WIP(archive): {}", change_id);
    let wip_commits = run_git(
        &[
            "rev-list",
            "--reverse",
            "--grep",
            &wip_pattern,
            "--fixed-strings",
            "HEAD",
        ],
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
/// archive, hidden entries, and changes without proposal.md.
pub async fn list_changes_in_head<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let cwd_ref = cwd.as_ref();

    let output = run_git(
        &["ls-tree", "-d", "--name-only", "HEAD:openspec/changes"],
        cwd_ref,
    )
    .await?;

    let mut change_ids: Vec<String> = Vec::new();

    for name in output.lines().map(str::trim) {
        if name.is_empty() || name == "archive" || name.starts_with('.') {
            continue;
        }

        // Check if proposal.md exists in HEAD for this change
        let proposal_path = format!("HEAD:openspec/changes/{}/proposal.md", name);
        match run_git(&["cat-file", "-e", &proposal_path], cwd_ref).await {
            Ok(_) => {
                // proposal.md exists, include this change
                change_ids.push(name.to_string());
            }
            Err(_) => {
                // proposal.md doesn't exist, skip this change
                debug!("Skipping change '{}' in HEAD - no proposal.md found", name);
            }
        }
    }

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

/// Check if the working directory is clean (no uncommitted changes).
///
/// Returns true if working directory is clean, false otherwise.
pub async fn is_working_directory_clean<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    Ok(output.trim().is_empty())
}

#[allow(dead_code)]
/// Check for merge conflicts without modifying the working tree.
///
/// Uses `git merge-tree` to simulate a merge and detect conflicts without touching
/// the working directory or index. This is safe to run while agents are working
/// in the worktree.
///
/// Returns Ok(Some(conflict_files)) if conflicts are detected, Ok(None) if no conflicts.
pub async fn check_merge_conflicts<P: AsRef<Path>>(
    cwd: P,
    branch_name: &str,
) -> VcsResult<Option<Vec<String>>> {
    let cwd = cwd.as_ref();

    // Get the current HEAD commit
    let head_commit = run_git(&["rev-parse", "HEAD"], cwd).await?;
    let head_commit = head_commit.trim();

    // Get the branch commit
    let branch_commit = run_git(&["rev-parse", branch_name], cwd).await?;
    let branch_commit = branch_commit.trim();

    // Get the merge base
    let merge_base = run_git(&["merge-base", head_commit, branch_commit], cwd).await?;
    let merge_base = merge_base.trim();

    // Use git merge-tree to simulate the merge (available in Git 2.38+)
    // Format: git merge-tree --write-tree --merge-base <base> <branch1> <branch2>
    let output = Command::new("git")
        .args([
            "merge-tree",
            "--write-tree",
            "--merge-base",
            merge_base,
            head_commit,
            branch_commit,
        ])
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| VcsError::git_command(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    // According to git merge-tree documentation:
    // - Exit code 1 indicates conflicts (this is the primary indicator)
    // - Stdout format for conflicts:
    //   Line 1: <OID of toplevel tree>
    //   Lines 2+: <Conflicted file info> (mode, object, stage, filename)
    //   Last section: <Informational messages> (CONFLICT notices)
    // - Exit code 0 means clean merge (no conflicts)
    // - Other exit codes indicate command failure

    if exit_code == 1 {
        // Conflicts detected - parse stdout for conflicted file info
        // Stdout format: tree OID on line 1, then conflicted file info, then messages
        let conflict_files = parse_conflict_files_from_stdout(&stdout);

        // If stdout parsing didn't find files, fall back to stderr parsing
        let conflict_files = if conflict_files.is_empty() {
            parse_conflict_files_from_stderr(&stderr)
        } else {
            conflict_files
        };

        debug!(
            "Detected {} conflicts in worktree at {} (exit_code: {}, files: {:?})",
            conflict_files.len(),
            cwd.display(),
            exit_code,
            conflict_files
        );

        // Even if we can't parse specific files, exit code 1 means conflicts exist
        // Return a generic indicator if no files were parsed
        if conflict_files.is_empty() {
            debug!(
                "Exit code 1 but no conflict files parsed. stdout: {}, stderr: {}",
                stdout.trim(),
                stderr.trim()
            );
            Ok(Some(vec!["<unknown>".to_string()]))
        } else {
            Ok(Some(conflict_files))
        }
    } else if exit_code == 0 {
        // No conflicts - merge would succeed cleanly
        debug!(
            "No conflicts detected for {} in {}",
            branch_name,
            cwd.display()
        );
        Ok(None)
    } else {
        // merge-tree failed for another reason (not conflict-related)
        debug!(
            "Merge tree command failed: exit_code={}, stdout={}, stderr={}",
            exit_code,
            stdout.trim(),
            stderr.trim()
        );
        Err(VcsError::git_command(format!(
            "Merge tree simulation failed (exit {}): {}",
            exit_code, stderr
        )))
    }
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

/// Parse conflict files from git merge-tree stdout.
///
/// Parses the "Conflicted file info" section from stdout.
/// Format: `<mode> <object> <stage> <filename>`
///
/// According to git documentation, stdout for conflicted merge contains:
/// - Line 1: Tree OID
/// - Lines 2+: Conflicted file info (until empty line or informational messages)
/// - Last section: Informational messages (CONFLICT notices)
fn parse_conflict_files_from_stdout(stdout: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut lines = stdout.lines();

    // Skip first line (tree OID)
    if lines.next().is_none() {
        return files;
    }

    // Parse conflicted file info section
    // Format: <mode> <object> <stage> <filename>
    // Example: 100644 abc123... 2 src/main.rs
    for line in lines {
        let line = line.trim();

        // Empty line or start of informational messages section
        if line.is_empty() {
            break;
        }

        // Check if line matches conflicted file info format
        // Format: <mode> <object> <stage> <filename>
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() == 4 {
            // parts[0] = mode (e.g., "100644")
            // parts[1] = object (e.g., "abc123...")
            // parts[2] = stage (e.g., "1", "2", "3")
            // parts[3] = filename

            // Validate stage is a number (1, 2, or 3 for conflicts)
            if let Ok(stage) = parts[2].parse::<u8>() {
                if (1..=3).contains(&stage) {
                    let filename = parts[3].trim();
                    // Avoid duplicates
                    if !files.contains(&filename.to_string()) {
                        files.push(filename.to_string());
                    }
                }
            }
        }
    }

    files
}

/// Parse conflict files from git merge-tree stderr (fallback).
///
/// Extracts file paths from lines like "CONFLICT (content): Merge conflict in src/main.rs"
fn parse_conflict_files_from_stderr(stderr: &str) -> Vec<String> {
    let mut files = Vec::new();

    for line in stderr.lines() {
        if line.contains("CONFLICT") {
            // Extract filename from patterns like:
            // "CONFLICT (content): Merge conflict in <file>"
            // "CONFLICT (modify/delete): <file> deleted in ..."
            // "CONFLICT (rename/rename): Rename <file1>-><file2> ..."

            if let Some(idx) = line.find(" in ") {
                // "CONFLICT (content): Merge conflict in <file>"
                let file = line[idx + 4..].trim();
                files.push(file.to_string());
            } else if line.contains("deleted in") || line.contains("added in") {
                // "CONFLICT (modify/delete): <file> deleted in ..."
                if let Some(start) = line.find("): ") {
                    let rest = &line[start + 3..];
                    if let Some(end) = rest.find(" deleted") {
                        files.push(rest[..end].trim().to_string());
                    } else if let Some(end) = rest.find(" added") {
                        files.push(rest[..end].trim().to_string());
                    }
                }
            } else if line.contains("Rename") {
                // "CONFLICT (rename/rename): Rename <file1>-><file2> ..."
                if let Some(start) = line.find("Rename ") {
                    let rest = &line[start + 7..];
                    if let Some(end) = rest.find("->") {
                        let file1 = rest[..end].trim();
                        files.push(file1.to_string());
                        // Also add the target file
                        let after_arrow = &rest[end + 2..];
                        if let Some(space_idx) = after_arrow.find(' ') {
                            let file2 = after_arrow[..space_idx].trim();
                            files.push(file2.to_string());
                        }
                    }
                }
            }
        }
    }

    files
}

/// Merge a branch into the current branch.
///
/// Performs `git merge --no-ff --no-edit <branch>` to merge the specified branch.
/// Checks for a clean working directory first. If merge conflicts occur, aborts the merge.
/// Returns Ok(()) on successful merge, Err() on conflict or other errors.
pub async fn merge_branch<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<()> {
    let cwd = cwd.as_ref();

    // Check working directory is clean
    if !is_working_directory_clean(cwd).await? {
        return Err(VcsError::git_command(
            "Working directory is not clean. Commit or stash changes before merging.".to_string(),
        ));
    }

    // Perform the merge
    let output = Command::new("git")
        .args(["merge", "--no-ff", "--no-edit", branch_name])
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| VcsError::git_command(e.to_string()))?;

    if output.status.success() {
        debug!("Merged branch {} successfully", branch_name);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check if it's a conflict
        if stderr.contains("CONFLICT") {
            // Abort the merge
            let _ = run_git(&["merge", "--abort"], cwd).await;

            Err(VcsError::git_command(format!(
                "Merge conflict detected. Merge aborted. Files: {}",
                parse_conflict_files_from_stderr(&stderr).join(", ")
            )))
        } else {
            // Other error
            Err(VcsError::git_command(format!("Merge failed: {}", stderr)))
        }
    }
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

    #[tokio::test]
    async fn test_list_changes_in_head_excludes_without_proposal() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init"])
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

        let base_dir = temp_dir.path().join("openspec/changes");

        // Create change-a WITH proposal.md
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("change-a").join("tasks.md"), "- [ ] Task 1").unwrap();

        // Create change-b WITHOUT proposal.md (only tasks.md)
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b").join("tasks.md"), "- [ ] Task 1").unwrap();

        // Create change-c WITH proposal.md
        std::fs::create_dir_all(base_dir.join("change-c")).unwrap();
        std::fs::write(base_dir.join("change-c").join("proposal.md"), "test").unwrap();

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

        // Should only include change-a and change-c (have proposal.md)
        // change-b should be excluded (no proposal.md)
        assert_eq!(
            changes,
            vec!["change-a".to_string(), "change-c".to_string()]
        );
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

        // Create a worktree
        let worktree_path = temp_dir.path().join("worktree1");
        let _ = Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "feature-branch",
                "HEAD",
            ])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // List worktrees
        let worktrees = list_worktrees(temp_dir.path()).await.unwrap();

        assert_eq!(worktrees.len(), 2);

        // First worktree is the main one
        let (_path0, _head0, branch0, detached0, is_main0) = &worktrees[0];
        assert_eq!(branch0, "main");
        assert!(!detached0);
        assert!(is_main0);

        // Second worktree is the feature branch
        let (_path1, _head1, branch1, detached1, is_main1) = &worktrees[1];
        assert_eq!(branch1, "feature-branch");
        assert!(!detached1);
        assert!(!is_main1);
    }

    #[tokio::test]
    async fn test_is_working_directory_clean() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
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

        // Initially clean (no commits yet, but no uncommitted changes either)
        let _is_clean = is_working_directory_clean(temp_dir.path()).await.unwrap();
        // Actually, empty repo with no commits shows untracked files, so not clean

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

        // Should be clean now
        let is_clean = is_working_directory_clean(temp_dir.path()).await.unwrap();
        assert!(is_clean);

        // Add a file but don't commit
        std::fs::write(temp_dir.path().join("test.txt"), "test").unwrap();
        let is_clean = is_working_directory_clean(temp_dir.path()).await.unwrap();
        assert!(!is_clean);
    }

    #[tokio::test]
    async fn test_count_commits_ahead() {
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

        // Create initial commit on main
        std::fs::write(temp_dir.path().join("file1.txt"), "base").unwrap();
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

        // Create a branch and add commits
        let _ = Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        std::fs::write(temp_dir.path().join("file2.txt"), "feature1").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "Feature commit 1"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        std::fs::write(temp_dir.path().join("file3.txt"), "feature2").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "Feature commit 2"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Count commits ahead
        let count = count_commits_ahead(temp_dir.path(), "main", "feature")
            .await
            .unwrap();
        assert_eq!(count, 2, "Feature branch should be 2 commits ahead of main");

        // Check reverse (main should be 0 commits ahead of feature)
        let count_reverse = count_commits_ahead(temp_dir.path(), "feature", "main")
            .await
            .unwrap();
        assert_eq!(
            count_reverse, 0,
            "Main branch should be 0 commits ahead of feature"
        );

        // Check same branch
        let count_same = count_commits_ahead(temp_dir.path(), "main", "main")
            .await
            .unwrap();
        assert_eq!(count_same, 0, "Branch should be 0 commits ahead of itself");
    }

    #[tokio::test]
    async fn test_run_worktree_setup_no_script() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("worktree");
        std::fs::create_dir(&worktree_dir).unwrap();

        // No .wt/setup script exists - should succeed without error
        let result = run_worktree_setup(temp_dir.path(), &worktree_dir).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_worktree_setup_success() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("worktree");
        std::fs::create_dir(&worktree_dir).unwrap();

        // Create .wt/setup script
        let wt_dir = temp_dir.path().join(".wt");
        std::fs::create_dir(&wt_dir).unwrap();
        let setup_script = wt_dir.join("setup");

        // Create a simple test script that creates a marker file
        #[cfg(unix)]
        let script_content = "#!/bin/sh\ntouch $ROOT_WORKTREE_PATH/setup_ran\n";
        #[cfg(windows)]
        let script_content = "@echo off\ntype nul > %ROOT_WORKTREE_PATH%\\setup_ran\n";

        std::fs::write(&setup_script, script_content).unwrap();

        // Execute setup
        let result = run_worktree_setup(temp_dir.path(), &worktree_dir).await;
        assert!(result.is_ok());

        // Verify marker file was created
        assert!(temp_dir.path().join("setup_ran").exists());
    }

    #[tokio::test]
    async fn test_run_worktree_setup_failure() {
        let temp_dir = TempDir::new().unwrap();
        let worktree_dir = temp_dir.path().join("worktree");
        std::fs::create_dir(&worktree_dir).unwrap();

        // Create .wt/setup script that fails
        let wt_dir = temp_dir.path().join(".wt");
        std::fs::create_dir(&wt_dir).unwrap();
        let setup_script = wt_dir.join("setup");

        // Create a script that exits with error
        #[cfg(unix)]
        let script_content = "#!/bin/sh\nexit 1\n";
        #[cfg(windows)]
        let script_content = "@echo off\nexit /b 1\n";

        std::fs::write(&setup_script, script_content).unwrap();

        // Execute setup - should fail
        let result = run_worktree_setup(temp_dir.path(), &worktree_dir).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Setup script failed"));
    }

    #[tokio::test]
    async fn test_is_worktree_main_repo() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let _ = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
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
        std::fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Check that main repo is NOT a worktree
        let result = is_worktree(temp_dir.path(), temp_dir.path()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_is_worktree_valid_worktree() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let _ = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
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
        std::fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create a worktree
        let worktree_path = temp_dir.path().join("worktree1");
        let _ = Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "feature-branch",
            ])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Check that worktree is detected as a worktree
        let result = is_worktree(temp_dir.path(), &worktree_path).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_is_worktree_non_worktree_path() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let _ = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
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
        std::fs::write(temp_dir.path().join("file.txt"), "content").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Check a random path that is not a worktree
        let random_path = temp_dir.path().join("random");
        let result = is_worktree(temp_dir.path(), &random_path).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
