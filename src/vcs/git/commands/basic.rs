//! Basic Git command operations.
//!
//! This module provides fundamental Git operations such as running commands,
//! checking repository status, and managing branches.

use crate::vcs::commands::{check_vcs_available, run_vcs_command, run_vcs_command_ignore_error};
use crate::vcs::git::commands::status_policy::{
    read_only_status_argv, DIRTY_STATE_STATUS_ARGS, HUMAN_READABLE_STATUS_ARGS,
    PORCELAIN_STATUS_ARGS,
};
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
///
/// The untracked and ignored modes are passed explicitly rather than inherited:
/// `status.showUntrackedFiles=no` in a repository or a user's global config would
/// otherwise make a worktree full of untracked work report as clean, and that
/// answer is what a destructive deletion is allowed to act on. Ignored entries
/// stay excluded on purpose — generated content is not operator work, and
/// enumerating it is not what this observation is for.
///
/// The observation is read-only, so it runs under the shared optional-lock
/// policy: see [`crate::vcs::git::commands::status_policy`].
pub async fn has_uncommitted_changes<P: AsRef<Path>>(cwd: P) -> VcsResult<(bool, String)> {
    let output = run_git(&read_only_status_argv(DIRTY_STATE_STATUS_ARGS), cwd).await?;
    let has_changes = !output.is_empty();
    Ok((has_changes, output))
}

/// `git status --porcelain` with its two status columns intact.
///
/// [`has_uncommitted_changes`] trims its output, which removes the leading
/// space that is the *only* difference between an unstaged worktree change
/// (` M path`) and a staged one (`M  path`) on the first line. Anything that
/// reads the worktree column — most importantly the Apply finalization stage
/// gate — must read the untrimmed bytes instead.
///
/// The untracked and ignored modes are passed explicitly for the same reason
/// they are on [`has_uncommitted_changes`]: repository or user configuration
/// must not be able to make unselected work report as clean.
///
/// Reading the stage gate must not cost the final Apply commit its index, so
/// this observation also runs under the shared optional-lock policy: see
/// [`crate::vcs::git::commands::status_policy`].
pub async fn porcelain_status<P: AsRef<Path>>(cwd: P) -> VcsResult<String> {
    let cwd = cwd.as_ref();
    let output = crate::vcs::commands::run_vcs_command_captured(
        "git",
        &read_only_status_argv(DIRTY_STATE_STATUS_ARGS),
        cwd,
        VcsBackend::Git,
    )
    .await?;

    if !output.success {
        return Err(VcsError::Command {
            backend: VcsBackend::Git,
            message: format!("{} failed: {}", output.command, output.stderr),
            command: Some(output.command),
            working_dir: Some(cwd.to_path_buf()),
            stderr: Some(output.stderr),
            stdout: Some(output.stdout),
        });
    }

    Ok(output.stdout)
}

/// Push a local branch to the same-named branch on the selected remote.
pub async fn push_same_named_branch<P: AsRef<Path>>(
    remote: &str,
    branch: &str,
    cwd: P,
) -> VcsResult<String> {
    if remote.contains(':') || branch.contains(':') {
        return Err(VcsError::git_command(
            "branch selection is not supported for push mode",
        ));
    }
    let refspec = format!("{branch}:{branch}");
    run_git(&["push", remote, &refspec], cwd).await
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

/// Files touched since `revision`, including uncommitted and untracked work.
///
/// Acceptance remediation coverage has to see the whole repair delta: a repair
/// Apply may commit, may leave the edit in the working tree, or may add a brand
/// new test file. `git diff --name-only <revision>` covers committed and
/// uncommitted tracked edits; untracked files are collected separately.
pub async fn get_changed_files_since<P: AsRef<Path>>(
    cwd: P,
    revision: &str,
) -> VcsResult<Vec<String>> {
    let cwd = cwd.as_ref();
    let mut files = run_git(&["diff", "--name-only", revision], cwd)
        .await?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(String::from)
        .collect::<Vec<_>>();
    files.extend(
        run_git(&["ls-files", "--others", "--exclude-standard"], cwd)
            .await?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(String::from),
    );
    files.sort();
    files.dedup();
    Ok(files)
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

/// Resolve the commit a named ref points to.
///
/// Dependency merge evidence is evaluated against a named base ref, which can advance while
/// the checkout commit stays put. Callers that need to notice that advance must resolve the
/// ref itself instead of reading `HEAD`.
pub async fn get_ref_revision<P: AsRef<Path>>(cwd: P, reference: &str) -> VcsResult<String> {
    run_git(
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", reference),
        ],
        cwd,
    )
    .await
}

/// Get the current branch name.
/// Returns None if in detached HEAD state.
pub async fn get_current_branch<P: AsRef<Path>>(cwd: P) -> VcsResult<Option<String>> {
    let cwd = cwd.as_ref();
    match run_git(&["rev-parse", "--abbrev-ref", "HEAD"], cwd).await {
        // Detached HEAD state
        Ok(branch) if branch == "HEAD" => Ok(None),
        Ok(branch) => Ok(Some(branch)),
        // An unborn HEAD (freshly initialized repository with no commits) still has
        // an attached branch, but `rev-parse` cannot resolve it. Fall back to the
        // symbolic ref so callers see the branch name instead of a git failure.
        Err(err) => match run_git(&["symbolic-ref", "--short", "HEAD"], cwd).await {
            Ok(branch) if !branch.is_empty() => Ok(Some(branch)),
            _ => Err(err),
        },
    }
}

/// Get git status output.
///
/// The human-readable text is captured as conflict-resolution prompt context,
/// so it is a read-only observation and runs under the shared optional-lock
/// policy: see [`crate::vcs::git::commands::status_policy`].
pub async fn get_status<P: AsRef<Path>>(cwd: P) -> VcsResult<String> {
    run_git(&read_only_status_argv(HUMAN_READABLE_STATUS_ARGS), cwd).await
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
///
/// Read-only, so it runs under the shared optional-lock policy: see
/// [`crate::vcs::git::commands::status_policy`].
pub async fn is_working_directory_clean<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&read_only_status_argv(PORCELAIN_STATUS_ARGS), cwd).await?;
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

/// Delete a branch only when Git can still reach its commits from elsewhere.
///
/// `git branch -d` is the reachability reconfirmation itself: it refuses when the
/// branch carries commits no other ref reaches, so a cleanup decision made from a
/// now-stale observation cannot orphan work.
pub async fn branch_delete_if_merged<P: AsRef<Path>>(cwd: P, branch_name: &str) -> VcsResult<()> {
    debug!("Deleting merged branch {}", branch_name);
    run_git(&["branch", "-d", branch_name], cwd).await?;
    Ok(())
}

/// Delete a local branch only if its ref still points at `expected_oid`.
///
/// `git update-ref -d <ref> <oldvalue>` compares and deletes inside one ref
/// transaction, so nothing can move the branch between the check and the
/// deletion. That matters here and not in [`branch_delete_if_merged`]: this is
/// the path used for commits Git cannot reach from anywhere else, where losing
/// the race would mean deleting a ref the caller never confirmed.
///
/// A mismatched, missing, or unreadable ref fails and leaves the branch alone.
pub async fn branch_delete_at_oid<P: AsRef<Path>>(
    cwd: P,
    branch_name: &str,
    expected_oid: &str,
) -> VcsResult<()> {
    debug!(
        "Deleting branch {} at expected commit {}",
        branch_name, expected_oid
    );
    run_git(
        &[
            "update-ref",
            "-d",
            &format!("refs/heads/{}", branch_name),
            expected_oid,
        ],
        cwd,
    )
    .await?;
    Ok(())
}

/// Current commit a local branch ref points at.
///
/// `Ok(None)` means the ref does not exist; `Err` means the ref state could not
/// be read at all, which callers must not confuse with "already gone".
pub async fn branch_ref_oid<P: AsRef<Path>>(
    cwd: P,
    branch_name: &str,
) -> VcsResult<Option<String>> {
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
        .map_err(|e| VcsError::git_command(format!("Failed to read branch ref: {}", e)))?;

    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    match stdout.split_whitespace().next() {
        Some(oid) => Ok(Some(oid.to_string())),
        None => Err(VcsError::git_command(format!(
            "git show-ref reported no object for branch '{}'",
            branch_name
        ))),
    }
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
    use std::path::Path;
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

    async fn run_git_ok(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await
            .expect("git command should start");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn push_post_archive_push_same_named_branch_to_bare_remote() {
        let repo = TempDir::new().unwrap();
        let remote = TempDir::new().unwrap();
        run_git_ok(remote.path(), &["init", "--bare"]).await;
        run_git_ok(repo.path(), &["init", "-b", "main"]).await;
        run_git_ok(repo.path(), &["config", "user.email", "test@example.com"]).await;
        run_git_ok(repo.path(), &["config", "user.name", "Test User"]).await;
        std::fs::write(repo.path().join("README.md"), "base\n").unwrap();
        run_git_ok(repo.path(), &["add", "."]).await;
        run_git_ok(repo.path(), &["commit", "-m", "base"]).await;
        run_git_ok(repo.path(), &["checkout", "-b", "change-a"]).await;
        std::fs::write(repo.path().join("feature.txt"), "feature\n").unwrap();
        run_git_ok(repo.path(), &["add", "."]).await;
        run_git_ok(repo.path(), &["commit", "-m", "feature"]).await;
        run_git_ok(
            repo.path(),
            &[
                "remote",
                "add",
                "origin",
                remote.path().to_str().expect("remote path utf8"),
            ],
        )
        .await;

        push_same_named_branch("origin", "change-a", repo.path())
            .await
            .expect("push same named branch");

        let remote_head = run_git(&["rev-parse", "refs/heads/change-a"], remote.path())
            .await
            .expect("remote branch head");
        let local_head = run_git(&["rev-parse", "change-a"], repo.path())
            .await
            .expect("local branch head");
        assert_eq!(remote_head, local_head);
    }

    #[tokio::test]
    async fn push_post_archive_rejects_branch_selection_syntax() {
        let repo = TempDir::new().unwrap();
        let error = push_same_named_branch("origin:main", "change-a", repo.path())
            .await
            .expect_err("remote override must fail");
        assert!(error
            .to_string()
            .contains("branch selection is not supported"));

        let error = push_same_named_branch("origin", "change-a:main", repo.path())
            .await
            .expect_err("branch override must fail");
        assert!(error
            .to_string()
            .contains("branch selection is not supported"));
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
