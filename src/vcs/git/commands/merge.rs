//! Git merge operations.
//!
//! This module provides functions for merging branches, detecting conflicts,
//! and managing merge-related operations.

use super::basic::{get_conflict_files, is_working_directory_clean, run_git};
use super::status_policy::{read_only_status_argv, PORCELAIN_UNTRACKED_STATUS_ARGS};
use crate::vcs::{VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// How many conflicting paths a diagnostic may name.
pub const MAX_CONFLICT_SAMPLE: usize = 20;

/// How many bytes of each raw output stream a diagnostic may quote.
pub const MAX_OUTPUT_PREFIX_BYTES: usize = 4096;

/// The bounded evidence one `git merge-tree` simulation produced.
///
/// `merge-tree` output size is a function of the conflict surface, and nothing
/// bounds that: one stale branch far behind base can emit megabytes, on every
/// refresh, forever. So the raw streams never leave this module. What leaves is
/// this — the facts an operator or a bug report actually needs (what the command
/// decided, how much it said, which paths it named) plus a deterministic sample
/// small enough to log unconditionally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSimulation {
    /// True when `merge-tree` reported the merge would conflict.
    pub conflicted: bool,
    /// Process exit status, `-1` when the process was signalled.
    pub exit_code: i32,
    /// Total stdout size, including everything not quoted.
    pub stdout_bytes: usize,
    /// Total stderr size, including everything not quoted.
    pub stderr_bytes: usize,
    /// Number of distinct conflicting paths parsed out of the output.
    pub conflict_count: usize,
    /// At most [`MAX_CONFLICT_SAMPLE`] of those paths, deterministically chosen.
    pub conflict_sample: Vec<String>,
    /// At most [`MAX_OUTPUT_PREFIX_BYTES`] of stdout.
    pub stdout_prefix: String,
    /// At most [`MAX_OUTPUT_PREFIX_BYTES`] of stderr.
    pub stderr_prefix: String,
}

impl MergeSimulation {
    /// The conflicting paths to present, or `None` for a clean merge.
    ///
    /// A conflict whose paths could not be parsed still returns `Some`: the exit
    /// status is the authoritative conflict signal, and answering `None` there
    /// would report an unmergeable branch as clean.
    pub fn conflict_files(&self) -> Option<Vec<String>> {
        if !self.conflicted {
            return None;
        }
        if self.conflict_sample.is_empty() {
            return Some(vec!["<unknown>".to_string()]);
        }
        Some(self.conflict_sample.clone())
    }

    /// One-line diagnostic. Bounded by construction, so it is safe to log on
    /// every refresh and safe to hand to an operator as an error message.
    pub fn summary(&self) -> String {
        format!(
            "merge-tree exit={} conflicted={} conflicts={} sample={:?} stdout_bytes={} stderr_bytes={} stdout_prefix={:?} stderr_prefix={:?}",
            self.exit_code,
            self.conflicted,
            self.conflict_count,
            self.conflict_sample,
            self.stdout_bytes,
            self.stderr_bytes,
            self.stdout_prefix,
            self.stderr_prefix,
        )
    }
}

/// Return at most `limit` bytes of `text`, never splitting a character.
///
/// The inputs are `from_utf8_lossy` conversions of child output, so a naive byte
/// slice can land inside a multi-byte character and panic.
pub fn bounded_prefix(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Reduce one raw `merge-tree` result to bounded evidence.
///
/// Pure, so the bounds are provable without spawning Git.
pub fn summarize_merge_tree(exit_code: i32, stdout: &str, stderr: &str) -> MergeSimulation {
    // The structured section of stdout is the primary source. `merge-tree`
    // writes its informational `CONFLICT (...)` messages to the *end of stdout*,
    // not to stderr, so that section is the first fallback and stderr only the
    // last — reading them in the other order is how a real conflict ends up
    // recorded as an unknown file.
    let mut files = parse_conflict_files_from_stdout(stdout);
    if files.is_empty() {
        files = parse_conflict_files_from_stderr(stdout);
    }
    if files.is_empty() {
        files = parse_conflict_files_from_stderr(stderr);
    }

    // Sorted, then truncated: the sample must name the same paths every refresh
    // for the same conflict, and Git's own emission order is not something this
    // module gets to depend on.
    files.sort();
    files.dedup();
    let conflict_count = files.len();
    files.truncate(MAX_CONFLICT_SAMPLE);

    MergeSimulation {
        conflicted: exit_code == 1,
        exit_code,
        stdout_bytes: stdout.len(),
        stderr_bytes: stderr.len(),
        conflict_count,
        conflict_sample: files,
        stdout_prefix: bounded_prefix(stdout, MAX_OUTPUT_PREFIX_BYTES).to_string(),
        stderr_prefix: bounded_prefix(stderr, MAX_OUTPUT_PREFIX_BYTES).to_string(),
    }
}

/// Check for merge conflicts without modifying the working tree.
///
/// Uses `git merge-tree` to simulate a merge and detect conflicts without touching
/// the working directory or index. This is safe to run while agents are working
/// in the worktree.
///
/// Returns the bounded [`MergeSimulation`] for a decided merge; an `Err` only
/// for a command that failed to decide at all.
pub async fn check_merge_conflicts<P: AsRef<Path>>(
    cwd: P,
    branch_name: &str,
) -> VcsResult<MergeSimulation> {
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

    let simulation = summarize_merge_tree(exit_code, &stdout, &stderr);

    // Every branch below logs the same bounded summary. A known conflict is an
    // ordinary observation, not a degraded path, so repeating it on each refresh
    // must cost a fixed number of bytes no matter how large the conflict is.
    match exit_code {
        0 | 1 => {
            debug!(
                worktree = %cwd.display(),
                base = branch_name,
                "merge simulation completed: {}",
                simulation.summary()
            );
            Ok(simulation)
        }
        _ => {
            // merge-tree failed for another reason (not conflict-related).
            debug!(
                worktree = %cwd.display(),
                base = branch_name,
                "merge simulation failed: {}",
                simulation.summary()
            );
            Err(VcsError::git_command(format!(
                "Merge tree simulation failed: {}",
                simulation.summary()
            )))
        }
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
    for line in lines {
        // Empty line or start of informational messages section
        if line.trim().is_empty() {
            break;
        }

        if let Some(filename) = parse_conflicted_file_record(line) {
            // Avoid duplicates
            if !files.iter().any(|existing| existing == filename) {
                files.push(filename.to_string());
            }
        }
    }

    files
}

/// One `<mode> <object> <stage>` + path record from the conflicted-file section.
///
/// Git separates the path with a tab, the same way `ls-files --stage` does, but
/// the section has historically been read here as plain space-separated fields.
/// Both are accepted: a parser that recognises only one of them silently degrades
/// every conflict to "unknown file", which is exactly the evidence a bounded
/// diagnostic is supposed to retain.
fn parse_conflicted_file_record(line: &str) -> Option<&str> {
    let (mode, object, stage, path) = match line.split_once('\t') {
        Some((meta, path)) => {
            let mut fields = meta.split_whitespace();
            let mode = fields.next()?;
            let object = fields.next()?;
            let stage = fields.next()?;
            if fields.next().is_some() {
                return None;
            }
            (mode, object, stage, path)
        }
        None => {
            let mut fields = line.trim_start().splitn(4, ' ');
            (
                fields.next()?,
                fields.next()?,
                fields.next()?,
                fields.next()?,
            )
        }
    };

    if !valid_conflict_stage(mode, object, stage) {
        return None;
    }
    let path = path.trim();
    (!path.is_empty()).then_some(path)
}

/// True when the three metadata fields describe a stage 1/2/3 conflict entry.
fn valid_conflict_stage(mode: &str, object: &str, stage: &str) -> bool {
    !mode.is_empty()
        && !object.is_empty()
        && matches!(stage.parse::<u8>(), Ok(stage) if (1..=3).contains(&stage))
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

/// Return every path that differs between two revisions.
///
/// Rename detection is disabled on purpose: the caller uses this as the
/// *superset* of paths one merge may legitimately touch, so a rename must
/// contribute both its source and its destination rather than the destination
/// alone.
pub async fn diff_paths_between<P: AsRef<Path>>(
    cwd: P,
    from: &str,
    to: &str,
) -> VcsResult<Vec<String>> {
    let output = run_git(&["diff", "--no-renames", "--name-only", from, to], cwd).await?;
    let mut paths: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Check whether the index and working tree exactly match `HEAD`, untracked
/// files included.
///
/// Read-only, so it runs under the shared optional-lock policy: see
/// [`crate::vcs::git::commands::status_policy`].
pub async fn is_clean_including_untracked<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&read_only_status_argv(PORCELAIN_UNTRACKED_STATUS_ARGS), cwd).await?;
    Ok(output.trim().is_empty())
}

#[cfg(test)]
mod bounded_diagnostics_tests {
    use super::*;

    /// A conflicted `merge-tree` stdout naming `count` paths.
    fn conflicted_stdout(count: usize) -> String {
        let mut out = String::from("0123456789abcdef0123456789abcdef01234567\n");
        for index in 0..count {
            out.push_str(&format!(
                "100644 {:040x} 2\tsrc/generated/module_{:05}.rs\n",
                index, index
            ));
        }
        out.push('\n');
        for index in 0..count {
            out.push_str(&format!(
                "CONFLICT (content): Merge conflict in src/generated/module_{:05}.rs\n",
                index
            ));
        }
        out
    }

    #[test]
    fn a_huge_conflict_is_summarized_within_fixed_bounds() {
        let stdout = conflicted_stdout(5_000);
        let stderr = "E".repeat(200_000);

        let simulation = summarize_merge_tree(1, &stdout, &stderr);

        assert!(simulation.conflicted);
        assert_eq!(simulation.exit_code, 1);
        assert_eq!(simulation.conflict_count, 5_000, "the count stays truthful");
        assert_eq!(
            simulation.conflict_sample.len(),
            MAX_CONFLICT_SAMPLE,
            "the sample is capped no matter how many paths conflict"
        );
        assert_eq!(
            simulation.stdout_bytes,
            stdout.len(),
            "the total size is retained even though the content is not"
        );
        assert_eq!(simulation.stderr_bytes, stderr.len());
        assert!(simulation.stdout_prefix.len() <= MAX_OUTPUT_PREFIX_BYTES);
        assert!(simulation.stderr_prefix.len() <= MAX_OUTPUT_PREFIX_BYTES);

        let summary = simulation.summary();
        assert!(
            summary.len() < 16 * 1024,
            "a diagnostic logged on every refresh must stay small, got {} bytes",
            summary.len()
        );
        assert!(
            !summary.contains("module_04999"),
            "the summary must not carry the complete raw output"
        );
        assert!(
            summary.contains("stdout_bytes=") && summary.contains("stderr_bytes="),
            "the summary must still disclose how much output was produced: {summary}"
        );
    }

    #[test]
    fn the_conflict_sample_is_deterministic_across_output_orderings() {
        let stdout = conflicted_stdout(200);
        let reordered: String = {
            let mut lines: Vec<&str> = stdout.lines().skip(1).filter(|l| !l.is_empty()).collect();
            lines.reverse();
            let mut out = String::from("0123456789abcdef0123456789abcdef01234567\n");
            out.push_str(&lines.join("\n"));
            out
        };

        assert_eq!(
            summarize_merge_tree(1, &stdout, "").conflict_sample,
            summarize_merge_tree(1, &reordered, "").conflict_sample,
            "the same conflict must sample the same paths whichever order Git emitted them in"
        );
    }

    #[test]
    fn a_clean_simulation_reports_no_conflict_files() {
        let simulation = summarize_merge_tree(0, "0123456789abcdef\n", "");

        assert!(!simulation.conflicted);
        assert_eq!(simulation.conflict_count, 0);
        assert_eq!(simulation.conflict_files(), None);
    }

    #[test]
    fn an_unparsed_conflict_is_still_reported_as_conflicted() {
        let simulation = summarize_merge_tree(1, "", "something unrecognized\n");

        assert!(simulation.conflicted);
        assert_eq!(simulation.conflict_count, 0);
        assert_eq!(
            simulation.conflict_files(),
            Some(vec!["<unknown>".to_string()]),
            "an unparseable conflict must never read as a clean merge"
        );
    }

    #[test]
    fn output_prefixes_never_split_a_character() {
        let multibyte = "日本語テキスト".repeat(2_000);

        let simulation = summarize_merge_tree(0, &multibyte, &multibyte);

        assert!(simulation.stdout_prefix.len() <= MAX_OUTPUT_PREFIX_BYTES);
        assert!(
            multibyte.starts_with(&simulation.stdout_prefix),
            "the prefix must be a real prefix of the output"
        );
        assert_eq!(bounded_prefix("short", MAX_OUTPUT_PREFIX_BYTES), "short");
    }
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
