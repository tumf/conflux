//! Git merge operations.
//!
//! This module provides functions for merging branches, detecting conflicts,
//! and managing merge-related operations.

use super::basic::{is_working_directory_clean, run_git};
use crate::vcs::{VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

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
}
