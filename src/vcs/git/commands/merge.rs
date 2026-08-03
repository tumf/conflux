//! Git merge operations.
//!
//! This module provides functions for merging branches, detecting conflicts,
//! and managing merge-related operations.

use super::basic::{get_conflict_files, is_working_directory_clean, run_git};
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

/// What a conflict-preserving base merge did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreservedMergeOutcome {
    /// The merge completed and produced a merge commit.
    Merged,
    /// The merge conflicted; `MERGE_HEAD` and the conflicted index are still in place.
    Conflict {
        /// Repository-relative paths of the unmerged entries.
        files: Vec<String>,
    },
}

/// Merge a branch into the current branch without aborting on conflict.
///
/// Same `git merge --no-ff --no-edit <branch>` as [`merge_branch`] and the same
/// clean-working-directory precondition, but a conflict is reported as a value
/// with the intermediate merge state left in the repository. Callers that want
/// the historical auto-abort behavior keep using [`merge_branch`]; callers whose
/// contract is "preserve the evidence for local resolution" use this one.
pub async fn merge_branch_preserving_conflict<P: AsRef<Path>>(
    cwd: P,
    branch_name: &str,
) -> VcsResult<PreservedMergeOutcome> {
    let cwd = cwd.as_ref();

    if !is_working_directory_clean(cwd).await? {
        return Err(VcsError::git_command(
            "Working directory is not clean. Commit or stash changes before merging.".to_string(),
        ));
    }

    let output = Command::new("git")
        .args(["merge", "--no-ff", "--no-edit", branch_name])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(e.to_string()))?;

    if output.status.success() {
        debug!("Merged branch {} successfully", branch_name);
        return Ok(PreservedMergeOutcome::Merged);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    if combined.contains("CONFLICT") || combined.contains("Automatic merge failed") {
        // The index is the authoritative conflict record; the stderr scrape is
        // only a fallback for the rare case where the index read fails.
        let files = match get_conflict_files(cwd).await {
            Ok(files) if !files.is_empty() => files,
            _ => parse_conflict_files_from_stderr(&combined),
        };
        return Ok(PreservedMergeOutcome::Conflict { files });
    }

    Err(VcsError::git_command(format!(
        "git merge {} failed: {}",
        branch_name, combined
    )))
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

/// Return the commit currently recorded in `MERGE_HEAD`, if a merge is in progress.
pub async fn merge_head<P: AsRef<Path>>(cwd: P) -> VcsResult<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to read MERGE_HEAD: {}", e)))?;

    if !output.status.success() {
        return Ok(None);
    }
    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!head.is_empty()).then_some(head))
}

/// Resolve a revision to a full commit id, returning `Ok(None)` when it does not exist.
pub async fn rev_parse_commit<P: AsRef<Path>>(cwd: P, revision: &str) -> VcsResult<Option<String>> {
    let spec = format!("{}^{{commit}}", revision);
    let output = Command::new("git")
        .args(["rev-parse", "-q", "--verify", spec.as_str()])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to resolve '{}': {}", revision, e)))?;

    if !output.status.success() {
        return Ok(None);
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!commit.is_empty()).then_some(commit))
}

/// Return the complete ordered parent list of a commit.
///
/// The order is Git's own: index 0 is the first parent. An empty result means a
/// root commit.
pub async fn parents_of<P: AsRef<Path>>(cwd: P, commit: &str) -> VcsResult<Vec<String>> {
    let output = run_git(&["rev-list", "-1", "--parents", commit], cwd).await?;
    let line = output.lines().next().unwrap_or("").trim();
    Ok(line
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect())
}

/// Return the first-parent lineage of `tip`, newest first.
pub async fn first_parent_lineage<P: AsRef<Path>>(cwd: P, tip: &str) -> VcsResult<Vec<String>> {
    let output = run_git(&["rev-list", "--first-parent", tip], cwd).await?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// Return the merge base of two revisions, or `Ok(None)` when they are unrelated.
pub async fn merge_base<P: AsRef<Path>>(cwd: P, a: &str, b: &str) -> VcsResult<Option<String>> {
    let output = Command::new("git")
        .args(["merge-base", a, b])
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::git_command(format!("Failed to execute git merge-base: {}", e)))?;

    if !output.status.success() {
        return Ok(None);
    }
    let base = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!base.is_empty()).then_some(base))
}

/// Enumerate every commit in `from..to` (or reachable from `to` when `from` is
/// `None`) whose complete subject equals `subject`.
///
/// Unlike [`merge_commit_hash_by_subject_since`] this does not pre-filter on
/// merge commits and does not collapse duplicates: the caller decides whether
/// zero, one, or many candidates are acceptable, so an ambiguous history can
/// fail closed instead of silently selecting the newest match.
pub async fn commits_with_exact_subject<P: AsRef<Path>>(
    cwd: P,
    from: Option<&str>,
    to: &str,
    subject: &str,
) -> VcsResult<Vec<String>> {
    let range = match from {
        Some(from) => format!("{}..{}", from, to),
        None => to.to_string(),
    };
    let output = run_git(&["log", "--format=%H%x09%s", range.as_str()], cwd).await?;

    let mut matches = Vec::new();
    for line in output.lines() {
        let mut parts = line.splitn(2, '\t');
        let Some(hash) = parts.next() else {
            continue;
        };
        let hash = hash.trim();
        if hash.is_empty() {
            continue;
        }
        if parts.next().unwrap_or("") == subject {
            matches.push(hash.to_string());
        }
    }

    Ok(matches)
}

/// An unmerged index entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexConflictEntry {
    /// Conflict stage (1 = base, 2 = ours, 3 = theirs).
    pub stage: u8,
    /// Repository-relative path.
    pub path: String,
}

/// Return every unmerged (stage 1/2/3) index entry.
pub async fn index_conflict_entries<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<IndexConflictEntry>> {
    let output = run_git(&["ls-files", "--unmerged"], cwd).await?;
    Ok(output.lines().filter_map(parse_index_entry).collect())
}

/// Return the stage-0 (fully merged) index paths under `prefix`.
pub async fn index_stage0_paths<P: AsRef<Path>>(cwd: P, prefix: &str) -> VcsResult<Vec<String>> {
    let output = run_git(&["ls-files", "--stage", "--", prefix], cwd).await?;
    Ok(output
        .lines()
        .filter_map(parse_index_entry)
        .filter(|entry| entry.stage == 0)
        .map(|entry| entry.path)
        .collect())
}

fn parse_index_entry(line: &str) -> Option<IndexConflictEntry> {
    let line = line.trim_end_matches('\n');
    let (meta, path) = line.split_once('\t')?;
    let stage = meta.split_whitespace().nth(2)?.parse::<u8>().ok()?;
    Some(IndexConflictEntry {
        stage,
        path: path.to_string(),
    })
}

/// Return the committed tree paths under `prefix` for `revision`.
pub async fn committed_tree_paths<P: AsRef<Path>>(
    cwd: P,
    revision: &str,
    prefix: &str,
) -> VcsResult<Vec<String>> {
    let output = run_git(
        &["ls-tree", "-r", "--name-only", revision, "--", prefix],
        cwd,
    )
    .await?;
    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// A single `name-status` entry of a commit's own tree diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDiffEntry {
    /// Git status letter (`A`, `M`, `D`, ...).
    pub status: char,
    /// Repository-relative path.
    pub path: String,
}

/// Return the complete tree diff of `commit` against its first parent.
pub async fn commit_diff_entries<P: AsRef<Path>>(
    cwd: P,
    commit: &str,
) -> VcsResult<Vec<CommitDiffEntry>> {
    let output = run_git(
        &[
            "diff-tree",
            "-r",
            "--no-commit-id",
            "--name-status",
            "--root",
            commit,
        ],
        cwd,
    )
    .await?;

    let mut entries = Vec::new();
    for line in output.lines() {
        let Some((status, path)) = line.split_once('\t') else {
            continue;
        };
        let Some(status) = status.trim().chars().next() else {
            continue;
        };
        // Rename/copy entries carry two paths; the destination is the last one.
        let path = path.rsplit('\t').next().unwrap_or(path).trim();
        if path.is_empty() {
            continue;
        }
        entries.push(CommitDiffEntry {
            status,
            path: path.to_string(),
        });
    }
    Ok(entries)
}

/// Check whether the index and working tree exactly match `HEAD`, untracked
/// files included.
pub async fn is_clean_including_untracked<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain", "--untracked-files=normal"], cwd).await?;
    Ok(output.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .await
            .unwrap_or_else(|e| panic!("git {:?}: {}", args, e));
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    async fn init_test_repo(dir: &Path) {
        git(dir, &["init", "-b", "main"]).await;
        git(dir, &["config", "user.email", "test@example.com"]).await;
        git(dir, &["config", "user.name", "Test User"]).await;
        git(dir, &["config", "commit.gpgsign", "false"]).await;
        std::fs::write(dir.join("README.md"), "initial\n").unwrap();
        git(dir, &["add", "-A"]).await;
        git(dir, &["commit", "-m", "Initial commit"]).await;
    }

    async fn commit_file(dir: &Path, name: &str, contents: &str, subject: &str) -> String {
        std::fs::write(dir.join(name), contents).unwrap();
        git(dir, &["add", "-A"]).await;
        git(dir, &["commit", "-m", subject]).await;
        git(dir, &["rev-parse", "HEAD"]).await
    }

    #[tokio::test]
    async fn exact_subject_enumeration_reports_zero_one_and_multiple_candidates() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        init_test_repo(dir).await;
        let base = git(dir, &["rev-parse", "HEAD"]).await;

        let subject = "Merge change: change-a";
        assert!(
            commits_with_exact_subject(dir, Some(&base), "HEAD", subject)
                .await
                .unwrap()
                .is_empty(),
            "no candidate must be reported before any matching commit exists"
        );

        commit_file(dir, "a.txt", "a\n", subject).await;
        assert_eq!(
            commits_with_exact_subject(dir, Some(&base), "HEAD", subject)
                .await
                .unwrap()
                .len(),
            1
        );

        // A near-miss subject must not be collected.
        commit_file(dir, "b.txt", "b\n", "Merge change: change-a-extra").await;
        assert_eq!(
            commits_with_exact_subject(dir, Some(&base), "HEAD", subject)
                .await
                .unwrap()
                .len(),
            1,
            "suffix-similar subjects must not count as exact candidates"
        );

        commit_file(dir, "c.txt", "c\n", subject).await;
        assert_eq!(
            commits_with_exact_subject(dir, Some(&base), "HEAD", subject)
                .await
                .unwrap()
                .len(),
            2,
            "duplicate exact subjects must both be reported so callers can fail closed"
        );
    }

    #[tokio::test]
    async fn parents_and_first_parent_lineage_distinguish_merge_sides() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        init_test_repo(dir).await;
        let base = git(dir, &["rev-parse", "HEAD"]).await;

        git(dir, &["checkout", "-b", "ws-change-a"]).await;
        let side = commit_file(dir, "side.txt", "side\n", "Side commit").await;

        git(dir, &["checkout", "main"]).await;
        let main_tip = commit_file(dir, "main.txt", "main\n", "Main commit").await;
        git(
            dir,
            &[
                "merge",
                "--no-ff",
                "-m",
                "Merge change: change-a",
                "ws-change-a",
            ],
        )
        .await;
        let merge_commit = git(dir, &["rev-parse", "HEAD"]).await;

        let parents = parents_of(dir, &merge_commit).await.unwrap();
        assert_eq!(parents, vec![main_tip.clone(), side.clone()]);

        let lineage = first_parent_lineage(dir, &merge_commit).await.unwrap();
        assert!(
            lineage.contains(&main_tip),
            "the merged-into tip stays on the first-parent lineage"
        );
        assert!(
            !lineage.contains(&side),
            "a side-branch commit must not appear on the first-parent lineage even though it is an ancestor"
        );
        assert!(
            is_ancestor(dir, &side, &merge_commit).await.unwrap(),
            "plain ancestry cannot tell the two sides apart, which is why lineage is checked separately"
        );

        assert_eq!(merge_base(dir, &main_tip, &side).await.unwrap(), Some(base));
        assert_eq!(
            parents_of(dir, &side).await.unwrap().len(),
            1,
            "an ordinary commit has exactly one parent"
        );
    }

    #[tokio::test]
    async fn index_views_separate_stage_zero_from_conflict_stages() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        init_test_repo(dir).await;
        std::fs::create_dir_all(dir.join("openspec/changes/change-a")).unwrap();
        commit_file(
            dir,
            "openspec/changes/change-a/proposal.md",
            "base\n",
            "Add change-a",
        )
        .await;

        let stage0 = index_stage0_paths(dir, "openspec/changes").await.unwrap();
        assert_eq!(stage0, vec!["openspec/changes/change-a/proposal.md"]);
        assert!(index_conflict_entries(dir).await.unwrap().is_empty());
        assert!(is_clean_including_untracked(dir).await.unwrap());

        // Build a real content conflict on the same path.
        git(dir, &["checkout", "-b", "ws-change-a"]).await;
        commit_file(
            dir,
            "openspec/changes/change-a/proposal.md",
            "branch\n",
            "Branch edit",
        )
        .await;
        git(dir, &["checkout", "main"]).await;
        commit_file(
            dir,
            "openspec/changes/change-a/proposal.md",
            "trunk\n",
            "Trunk edit",
        )
        .await;
        git(dir, &["merge", "--no-ff", "--no-commit", "ws-change-a"]).await;

        let conflicts = index_conflict_entries(dir).await.unwrap();
        assert!(
            conflicts.iter().any(
                |entry| entry.path == "openspec/changes/change-a/proposal.md"
                    && (1..=3).contains(&entry.stage)
            ),
            "unmerged entries must be reported with their conflict stage, got {:?}",
            conflicts
        );
        assert!(
            !index_stage0_paths(dir, "openspec/changes")
                .await
                .unwrap()
                .contains(&"openspec/changes/change-a/proposal.md".to_string()),
            "a conflicted path must not appear as merged stage-0 evidence"
        );
        assert_eq!(
            merge_head(dir).await.unwrap(),
            Some(git(dir, &["rev-parse", "ws-change-a"]).await)
        );
    }

    #[tokio::test]
    async fn cleanliness_detects_head_index_worktree_disagreement_and_untracked_dirt() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        init_test_repo(dir).await;
        assert!(is_clean_including_untracked(dir).await.unwrap());

        // Staged deletion: index disagrees with HEAD.
        git(dir, &["rm", "--cached", "README.md"]).await;
        assert!(
            !is_clean_including_untracked(dir).await.unwrap(),
            "a staged-only change must not read as clean"
        );
        git(dir, &["reset", "--", "README.md"]).await;
        assert!(is_clean_including_untracked(dir).await.unwrap());

        // Unstaged edit: worktree disagrees with index.
        std::fs::write(dir.join("README.md"), "dirty\n").unwrap();
        assert!(!is_clean_including_untracked(dir).await.unwrap());
        std::fs::write(dir.join("README.md"), "initial\n").unwrap();
        assert!(is_clean_including_untracked(dir).await.unwrap());

        // Untracked dirt.
        std::fs::write(dir.join("stray.txt"), "stray\n").unwrap();
        assert!(
            !is_clean_including_untracked(dir).await.unwrap(),
            "untracked files must count against terminal cleanliness"
        );
    }

    #[tokio::test]
    async fn committed_tree_and_diff_views_report_paths_and_deletions() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();
        init_test_repo(dir).await;
        std::fs::create_dir_all(dir.join("openspec/changes/change-a")).unwrap();
        commit_file(
            dir,
            "openspec/changes/change-a/proposal.md",
            "live\n",
            "Add change-a",
        )
        .await;
        let with_live = git(dir, &["rev-parse", "HEAD"]).await;

        assert_eq!(
            committed_tree_paths(dir, &with_live, "openspec/changes")
                .await
                .unwrap(),
            vec!["openspec/changes/change-a/proposal.md"]
        );

        git(dir, &["rm", "-r", "-f", "openspec/changes/change-a"]).await;
        git(
            dir,
            &["commit", "-m", "Cleanup resurrected change: change-a"],
        )
        .await;
        let cleanup = git(dir, &["rev-parse", "HEAD"]).await;

        let entries = commit_diff_entries(dir, &cleanup).await.unwrap();
        assert_eq!(
            entries,
            vec![CommitDiffEntry {
                status: 'D',
                path: "openspec/changes/change-a/proposal.md".to_string(),
            }]
        );
        assert!(committed_tree_paths(dir, &cleanup, "openspec/changes")
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            rev_parse_commit(dir, "HEAD").await.unwrap(),
            Some(cleanup.clone())
        );
        assert_eq!(rev_parse_commit(dir, "does-not-exist").await.unwrap(), None);
        assert_eq!(merge_head(dir).await.unwrap(), None);
    }
}
