//! Git commit operations.
//!
//! This module provides functions for creating, managing, and querying Git commits.

use super::basic::run_git;
use crate::vcs::commands::{run_vcs_command_captured, VcsCommandOutput};
use crate::vcs::{CommitRejection, VcsBackend, VcsError, VcsResult, VerifiedCommitOutcome};
use std::path::Path;
use tracing::debug;

/// Exit status `git commit` reports when the repository itself rejects the
/// commit, most importantly a non-zero `pre-commit` or `commit-msg` hook.
///
/// Fatal Git failures (bad repository, unreadable objects, usage errors) exit
/// with 128, and a signal-killed process reports no code at all, so matching
/// this exact status keeps recovery eligibility structural instead of relying
/// on the rendered error text.
pub const GIT_REPOSITORY_REJECTION_EXIT_CODE: i32 = 1;

/// Which finalization path a verified commit takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifiedCommitMode {
    /// The worktree had changes, so they were staged and committed normally.
    AddAndCommit,
    /// A WIP snapshot already left the worktree clean, so HEAD is amended.
    Amend,
}

/// Build the argument list for the verified (hook-enabled) commit.
///
/// Kept separate from execution so the "final commits never bypass hooks" rule
/// is checkable without spawning Git.
///
/// The amend path adds `--allow-empty` because the WIP snapshot it rewrites may
/// itself be empty (snapshots are created with `--allow-empty`), and the final
/// commit's job is to name the completed apply rather than to introduce
/// content. Without it Git exits 1 for "would make it empty", which is
/// indistinguishable from a hook rejection by exit code and would spend the
/// whole repair budget on a failure no agent can fix.
pub fn verified_commit_args(mode: VerifiedCommitMode, message: &str) -> Vec<String> {
    let mut args = vec!["commit".to_string()];
    if mode == VerifiedCommitMode::Amend {
        args.push("--amend".to_string());
        args.push("--allow-empty".to_string());
    }
    args.push("-m".to_string());
    args.push(message.to_string());
    args
}

/// Classify the result of a verified commit process that actually ran.
///
/// Only an exact [`GIT_REPOSITORY_REJECTION_EXIT_CODE`] becomes
/// [`VerifiedCommitOutcome::RepositoryRejected`]. Every other failure — fatal
/// Git status, signal death — stays a terminal [`VcsError`]. Classification
/// reads the preserved exit code only; it never inspects rendered error text.
pub fn classify_verified_commit_output(
    output: VcsCommandOutput,
    working_dir: &Path,
) -> VcsResult<VerifiedCommitOutcome> {
    if output.success {
        return Ok(VerifiedCommitOutcome::Committed);
    }

    if output.exit_code == Some(GIT_REPOSITORY_REJECTION_EXIT_CODE) {
        debug!(
            "Verified commit rejected by repository verification: {}",
            output.command
        );
        return Ok(VerifiedCommitOutcome::RepositoryRejected(CommitRejection {
            command: output.command,
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        }));
    }

    Err(VcsError::Command {
        backend: VcsBackend::Git,
        message: format!("{} failed: {}", output.command, output.stderr),
        command: Some(output.command),
        working_dir: Some(working_dir.to_path_buf()),
        stderr: Some(output.stderr),
        stdout: Some(output.stdout),
    })
}

/// Run a verified commit and classify its result structurally.
///
/// A spawn failure propagates as a terminal [`VcsError`] before any
/// classification happens.
async fn run_verified_commit<P: AsRef<Path>>(
    cwd: P,
    args: &[String],
) -> VcsResult<VerifiedCommitOutcome> {
    debug_assert!(
        !args.iter().any(|arg| arg == "--no-verify"),
        "verified commits must run repository hooks"
    );

    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_vcs_command_captured("git", &arg_refs, cwd.as_ref(), VcsBackend::Git).await?;

    classify_verified_commit_output(output, cwd.as_ref())
}

/// Create the verified (hook-enabled) commit for a workspace.
///
/// Setup steps (`status`, `add`, staged-snapshot validation) propagate as
/// terminal errors; only the commit itself can produce a repository rejection.
pub async fn create_verified_commit<P: AsRef<Path>>(
    cwd: P,
    message: &str,
) -> VcsResult<VerifiedCommitOutcome> {
    let mode = if has_changes_to_commit(&cwd).await? {
        run_git(&["add", "-A"], &cwd).await?;
        validate_staged_snapshot(&cwd).await?;
        VerifiedCommitMode::AddAndCommit
    } else {
        VerifiedCommitMode::Amend
    };

    run_verified_commit(&cwd, &verified_commit_args(mode, message)).await
}

fn staged_snapshot_anomaly<'a>(paths: impl Iterator<Item = &'a str>) -> Option<String> {
    const MAX_STAGED_FILES: usize = 5_000;

    let mut by_top_level = std::collections::BTreeMap::<&str, usize>::new();
    let mut total = 0;
    for path in paths {
        total += 1;
        *by_top_level
            .entry(path.split('/').next().unwrap_or(path))
            .or_default() += 1;
    }
    if let Some((directory, count)) = by_top_level
        .iter()
        .find(|(directory, _)| directory.starts_with(".agent-target"))
    {
        return Some(format!("forbidden temporary path {directory}: {count}"));
    }
    if total <= MAX_STAGED_FILES {
        return None;
    }

    let summary = by_top_level
        .iter()
        .take(20)
        .map(|(directory, count)| format!("{directory}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("{total} staged files ({summary})"))
}

pub async fn validate_staged_snapshot<P: AsRef<Path>>(cwd: P) -> VcsResult<()> {
    let cwd = cwd.as_ref();
    let output = run_git(
        &[
            "diff",
            "--cached",
            "--name-only",
            "--diff-filter=ACMR",
            "-z",
        ],
        cwd,
    )
    .await?;
    let paths = output
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    let Some(summary) = staged_snapshot_anomaly(paths.iter().copied()) else {
        return Ok(());
    };

    let _ = run_git(&["reset"], cwd).await;
    Err(VcsError::git_command(format!(
        "Refusing suspicious snapshot ({summary}); staged changes were reset"
    )))
}

/// Create a WIP archive commit for a retry attempt.
pub async fn create_archive_wip_commit<P: AsRef<Path>>(
    cwd: P,
    change_id: &str,
    attempt: u32,
) -> VcsResult<()> {
    let message = format!("WIP(archive): {} (attempt#{})", change_id, attempt);
    run_git(&["add", "-A"], &cwd).await?;
    validate_staged_snapshot(&cwd).await?;
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
    validate_staged_snapshot(&cwd).await?;
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

/// Check if there are staged or unstaged changes to commit.
#[allow(dead_code)]
pub async fn has_changes_to_commit<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    Ok(!output.is_empty())
}

/// List change IDs that have uncommitted or untracked files under `openspec/changes/<change_id>/`.
///
/// This function detects changes with:
/// - Staged changes (added, modified, deleted)
/// - Unstaged changes (modified, deleted)
/// - Untracked files
///
/// Returns a sorted vector of change IDs that have uncommitted modifications.
pub async fn list_changes_with_uncommitted_files<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let cwd_ref = cwd.as_ref();

    // Get all files with uncommitted changes or untracked status
    // Use -u to show individual untracked files instead of just directories
    let output = run_git(&["status", "--porcelain", "-u"], cwd_ref).await?;

    let mut change_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        // Parse porcelain format path field.
        // Normal: "XY <path>" (2 status columns + separator)
        // Fallback: "Y <path>" for first line when run_git() trimmed a leading space.
        let bytes = line.as_bytes();
        let path_field = if bytes.len() >= 3 && bytes[2] == b' ' {
            &line[3..]
        } else if bytes.len() >= 2 && bytes[1] == b' ' {
            &line[2..]
        } else {
            continue;
        };

        // Handle rename format: "old/path -> new/path". Use destination path.
        let path = path_field
            .split_once(" -> ")
            .map(|(_, new_path)| new_path)
            .unwrap_or(path_field)
            .trim();

        // Check if path is under openspec/changes/<change_id>/
        if let Some(change_id) = extract_change_id_from_path(path) {
            change_ids.insert(change_id);
        }
    }

    let mut result: Vec<String> = change_ids.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Extract change_id from a file path if it matches `openspec/changes/<change_id>/...`
///
/// Returns None if the path doesn't match the expected pattern or refers to the archive directory.
fn extract_change_id_from_path(path: &str) -> Option<String> {
    // Normalize path separators for cross-platform compatibility
    let normalized_path = path.replace('\\', "/");

    // Expected pattern: openspec/changes/<change_id>/...
    let prefix = "openspec/changes/";
    if !normalized_path.starts_with(prefix) {
        return None;
    }

    // Extract the part after "openspec/changes/"
    let remainder = &normalized_path[prefix.len()..];

    // Find the first path separator to get the change_id
    let change_id = if let Some(pos) = remainder.find('/') {
        &remainder[..pos]
    } else {
        // Path is exactly "openspec/changes/<change_id>" (no trailing slash)
        remainder
    };

    // Filter out special directories
    if change_id.is_empty() || change_id == "archive" || change_id.starts_with('.') {
        return None;
    }

    Some(change_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

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
    async fn test_list_changes_with_uncommitted_files() {
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

        // Create change-a and commit it
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create change-b with uncommitted files
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b").join("proposal.md"), "uncommitted").unwrap();

        // Modify change-a after commit (staged change)
        std::fs::write(base_dir.join("change-a").join("tasks.md"), "new task").unwrap();
        let _ = Command::new("git")
            .args(["add", "openspec/changes/change-a/tasks.md"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        // Should include both change-a (staged) and change-b (untracked)
        assert_eq!(
            uncommitted_changes,
            vec!["change-a".to_string(), "change-b".to_string()]
        );
    }

    #[tokio::test]
    async fn test_list_changes_with_uncommitted_files_filters_archive() {
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

        // Create initial commit to establish git repository
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create archive directory with uncommitted files (should be ignored)
        std::fs::create_dir_all(base_dir.join("archive")).unwrap();
        std::fs::write(base_dir.join("archive").join("old.md"), "archived").unwrap();

        // Create regular change with uncommitted files
        std::fs::create_dir_all(base_dir.join("change-c")).unwrap();
        std::fs::write(base_dir.join("change-c").join("proposal.md"), "test").unwrap();

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        // Should only include change-c, not archive
        assert_eq!(uncommitted_changes, vec!["change-c".to_string()]);
    }

    #[tokio::test]
    async fn test_list_changes_with_uncommitted_files_detects_unstaged_modified_first_line() {
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

        let change_dir = temp_dir.path().join("openspec/changes/change-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "test").unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [ ] task").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create unstaged modification. Status line is typically " M ...",
        // and the leading space can be trimmed by run_git() for the first line.
        std::fs::write(change_dir.join("tasks.md"), "- [x] task").unwrap();

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        assert_eq!(uncommitted_changes, vec!["change-a".to_string()]);
    }

    #[test]
    fn staged_snapshot_anomaly_rejects_thousands_of_files() {
        let paths = (0..5_001)
            .map(|index| format!("generated/artifact-{index}"))
            .collect::<Vec<_>>();

        let anomaly = staged_snapshot_anomaly(paths.iter().map(String::as_str));

        assert!(anomaly.is_some());
        assert!(anomaly.unwrap().contains("5001 staged files"));
    }

    #[test]
    fn staged_snapshot_anomaly_rejects_temporary_paths() {
        assert!(staged_snapshot_anomaly([".agent-target/debug/artifact"].into_iter()).is_some());
    }

    #[test]
    fn staged_snapshot_anomaly_allows_large_legitimate_changes() {
        let paths = (0..5_000)
            .map(|index| format!("src/generated-{index}.rs"))
            .collect::<Vec<_>>();

        assert!(staged_snapshot_anomaly(paths.iter().map(String::as_str)).is_none());
    }

    #[tokio::test]
    async fn validate_staged_snapshot_resets_suspicious_index() {
        let temp_dir = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        let generated = temp_dir.path().join(".agent-target");
        std::fs::create_dir(&generated).unwrap();
        std::fs::write(generated.join("artifact"), "x").unwrap();
        run_git(&["add", "-A"], temp_dir.path()).await.unwrap();

        let error = validate_staged_snapshot(temp_dir.path()).await.unwrap_err();
        let staged = run_git(&["diff", "--cached", "--name-only"], temp_dir.path())
            .await
            .unwrap();

        assert!(error.to_string().contains("forbidden temporary path"));
        assert!(staged.is_empty());
    }

    #[tokio::test]
    async fn validate_staged_snapshot_supports_non_ascii_paths() {
        let temp_dir = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        std::fs::write(temp_dir.path().join("　日本語.rs"), "fn main() {}").unwrap();
        run_git(&["add", "-A"], temp_dir.path()).await.unwrap();

        validate_staged_snapshot(temp_dir.path()).await.unwrap();
    }

    #[tokio::test]
    async fn archive_wip_rejects_suspicious_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        let generated = temp_dir.path().join(".agent-target");
        std::fs::create_dir(&generated).unwrap();
        std::fs::write(generated.join("artifact"), "x").unwrap();

        assert!(create_archive_wip_commit(temp_dir.path(), "change-a", 1)
            .await
            .is_err());
    }

    #[test]
    fn test_extract_change_id_from_path() {
        assert_eq!(
            extract_change_id_from_path("openspec/changes/my-change/proposal.md"),
            Some("my-change".to_string())
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/my-change/specs/auth/spec.md"),
            Some("my-change".to_string())
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/archive/old.md"),
            None
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/.hidden/file.md"),
            None
        );
        assert_eq!(extract_change_id_from_path("src/main.rs"), None);
        assert_eq!(
            extract_change_id_from_path("openspec/specs/auth/spec.md"),
            None
        );
    }
}
