//! Repository-level tests for the transient final Apply commit lock retry.
//!
//! These exercise real Git processes, a real `pre-commit` hook, and real
//! `index.lock` files against a temporary repository, so they are
//! integration-scoped evidence rather than unit evidence: they prove the
//! classifier recognises the lock message and lock identity Git actually
//! produces, and that both finalization paths recover while still running
//! repository verification. No in-memory double can establish either.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::test_support::{LockReleasingEnvironment, LOCK_SENTINEL};
use super::{
    run_final_commit_with_retry, FinalCommitEnvironment, GitFinalCommitEnvironment,
    FINAL_COMMIT_MAX_ATTEMPTS, FINAL_COMMIT_RETRY_DELAY,
};
use crate::config::OrchestratorConfig;
use crate::execution::apply::{create_final_commit, create_final_commit_with_environment};
use crate::vcs::git::commands::run_git;
use crate::vcs::git::GitWorkspaceManager;
use crate::vcs::{VcsBackend, VcsError, VerifiedCommitOutcome};

const CHANGE_ID: &str = "demo-change";

fn expected_commit_message() -> String {
    format!("Apply: {}", CHANGE_ID)
}

async fn git_ok(repo: &Path, args: &[&str]) {
    run_git(args, repo)
        .await
        .unwrap_or_else(|error| panic!("git {:?} failed: {}", args, error));
}

/// A repository whose `pre-commit` hook records every invocation, so "the
/// commit still ran repository verification" is observable.
struct LockRepo {
    _temp_dir: TempDir,
    path: PathBuf,
    hook_log: PathBuf,
    worktrees_dir: PathBuf,
}

impl LockRepo {
    /// Initialize a repository with one base commit and a counting hook.
    async fn init() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("repo");
        fs::create_dir_all(&path).unwrap();

        git_ok(&path, &["init", "-q", "-b", "main"]).await;
        git_ok(&path, &["config", "user.email", "test@example.com"]).await;
        git_ok(&path, &["config", "user.name", "Test User"]).await;
        git_ok(&path, &["config", "commit.gpgsign", "false"]).await;

        // The hook lives inside `.git` so `git add -A` never stages its log.
        let hooks_dir = path.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook = hooks_dir.join("pre-commit");
        fs::write(
            &hook,
            "#!/bin/sh\necho ran >> \"$(git rev-parse --git-dir)/hook.log\"\nexit 0\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }

        fs::write(path.join("base.txt"), "base\n").unwrap();
        git_ok(&path, &["add", "-A"]).await;
        git_ok(&path, &["commit", "-q", "--no-verify", "-m", "base"]).await;

        Self {
            hook_log: path.join(".git").join("hook.log"),
            worktrees_dir: temp_dir.path().join("worktrees"),
            path,
            _temp_dir: temp_dir,
        }
    }

    /// Leave uncommitted apply output, selecting the add-and-commit path.
    fn write_apply_output(&self) {
        fs::write(self.path.join("applied.txt"), "apply output\n").unwrap();
    }

    /// Record a WIP snapshot so the worktree is clean, selecting the amend path.
    async fn snapshot_apply_output(&self) {
        self.write_apply_output();
        git_ok(&self.path, &["add", "-A"]).await;
        git_ok(
            &self.path,
            &[
                "commit",
                "-q",
                "--no-verify",
                "--allow-empty",
                "-m",
                "WIP: demo-change (1/1 tasks, apply#1)",
            ],
        )
        .await;
    }

    fn workspace_manager(&self) -> GitWorkspaceManager {
        GitWorkspaceManager::new(
            self.worktrees_dir.clone(),
            self.path.clone(),
            1,
            OrchestratorConfig::default(),
        )
    }

    fn lock_path(&self) -> PathBuf {
        self.path.join(".git").join("index.lock")
    }

    /// Hold the managed worktree's index lock, as a competing Git process does.
    fn hold_index_lock(&self) -> PathBuf {
        let lock = self.lock_path();
        fs::write(&lock, LOCK_SENTINEL).unwrap();
        lock
    }

    fn hook_runs(&self) -> usize {
        fs::read_to_string(&self.hook_log)
            .map(|log| log.lines().count())
            .unwrap_or(0)
    }

    async fn commit_subjects(&self) -> Vec<String> {
        run_git(&["log", "--format=%s"], &self.path)
            .await
            .expect("git log")
            .lines()
            .map(str::to_string)
            .collect()
    }

    async fn final_commit_count(&self) -> usize {
        self.commit_subjects()
            .await
            .iter()
            .filter(|subject| *subject == &expected_commit_message())
            .count()
    }

    async fn head_file_names(&self) -> String {
        run_git(&["show", "--name-only", "--format=", "HEAD"], &self.path)
            .await
            .expect("git show")
    }

    async fn is_clean(&self) -> bool {
        run_git(&["status", "--porcelain"], &self.path)
            .await
            .expect("git status")
            .is_empty()
    }
}

/// Fabricate the contention error Git reports for this repository, so an
/// already-committed attempt can be made to look ambiguous.
async fn fabricated_lock_error(repo: &Path, command: &str) -> VcsError {
    let git_dir = run_git(&["rev-parse", "--absolute-git-dir"], repo)
        .await
        .expect("git dir");
    let stderr = format!(
        "fatal: Unable to create '{}/index.lock': File exists.\n\n\
         Another git process seems to be running in this repository.\n",
        git_dir
    );
    VcsError::Command {
        backend: VcsBackend::Git,
        message: format!("{} failed: {}", command, stderr),
        command: Some(command.to_string()),
        working_dir: Some(repo.to_path_buf()),
        stderr: Some(stderr),
        stdout: Some(String::new()),
    }
}

// ---- recovery on both finalization paths --------------------------------

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_recovers_add_and_commit_when_contention_clears() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    let environment = LockReleasingEnvironment::holding(repo.lock_path());

    let outcome = create_final_commit_with_environment(
        &repo.workspace_manager(),
        &repo.path,
        CHANGE_ID,
        None,
        &environment,
    )
    .await
    .expect("finalization must recover once contention clears");

    assert_eq!(outcome, VerifiedCommitOutcome::Committed);
    assert_eq!(
        environment.sleeps(),
        vec![FINAL_COMMIT_RETRY_DELAY],
        "the first attempt must have hit real contention and waited once"
    );
    assert!(
        environment.lock_was_untouched(),
        "Conflux must never delete or rewrite a lock it does not own"
    );
    assert!(
        !repo.lock_path().exists(),
        "the competing process released its own lock"
    );
    assert_eq!(
        repo.final_commit_count().await,
        1,
        "exactly one final Apply commit: {:?}",
        repo.commit_subjects().await
    );
    assert!(
        repo.head_file_names().await.contains("applied.txt"),
        "the apply output must land in the final commit"
    );
    assert!(repo.is_clean().await, "no workspace content may be left out");
    assert!(
        repo.hook_runs() >= 1,
        "the recovered final commit must run repository verification"
    );
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_recovers_amend_when_contention_clears() {
    let repo = LockRepo::init().await;
    repo.snapshot_apply_output().await;
    let hooks_before = repo.hook_runs();
    let environment = LockReleasingEnvironment::holding(repo.lock_path());

    let outcome = create_final_commit_with_environment(
        &repo.workspace_manager(),
        &repo.path,
        CHANGE_ID,
        None,
        &environment,
    )
    .await
    .expect("amend finalization must recover once contention clears");

    assert_eq!(outcome, VerifiedCommitOutcome::Committed);
    assert_eq!(
        environment.sleeps(),
        vec![FINAL_COMMIT_RETRY_DELAY],
        "the first attempt must have hit real contention and waited once"
    );
    assert!(
        environment.lock_was_untouched(),
        "Conflux must never delete or rewrite a lock it does not own"
    );
    assert!(!repo.lock_path().exists());
    assert_eq!(
        repo.commit_subjects().await,
        vec![expected_commit_message(), "base".to_string()],
        "the WIP snapshot must be replaced by exactly one final Apply commit"
    );
    assert!(repo.head_file_names().await.contains("applied.txt"));
    assert!(repo.is_clean().await);
    assert!(
        repo.hook_runs() > hooks_before,
        "the recovered amend must run repository verification"
    );
}

// ---- exhaustion, cancellation, and ineligible failures -------------------

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_exhaustion_preserves_workspace_and_lock() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    let lock = repo.hold_index_lock();

    let started = Instant::now();
    let error = create_final_commit(&repo.workspace_manager(), &repo.path, CHANGE_ID)
        .await
        .expect_err("a lock held past the retry budget must fail");

    assert!(
        started.elapsed() >= FINAL_COMMIT_RETRY_DELAY * 2,
        "three attempts must be separated by two fixed delays"
    );
    let VcsError::Command {
        message,
        command,
        working_dir,
        stderr,
        ..
    } = &error
    else {
        panic!("expected a command error, got {:?}", error);
    };
    assert!(
        message.contains(&format!(
            "attempt {}/{}",
            FINAL_COMMIT_MAX_ATTEMPTS, FINAL_COMMIT_MAX_ATTEMPTS
        )),
        "message must report the retry outcome: {}",
        message
    );
    assert_eq!(command.as_deref(), Some("git add -A"));
    assert_eq!(working_dir.as_deref(), Some(repo.path.as_path()));
    assert!(stderr.as_deref().unwrap_or_default().contains("index.lock"));

    assert_eq!(
        fs::read_to_string(&lock).unwrap(),
        LOCK_SENTINEL,
        "Conflux must never delete or rewrite a live lock"
    );
    assert_eq!(
        fs::read_to_string(repo.path.join("applied.txt")).unwrap(),
        "apply output\n",
        "workspace contents must survive for explicit recovery"
    );
    assert_eq!(repo.final_commit_count().await, 0);
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_cancellation_stops_retrying() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    let lock = repo.hold_index_lock();
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let error = create_final_commit_with_environment(
        &repo.workspace_manager(),
        &repo.path,
        CHANGE_ID,
        Some(&cancel_token),
        &GitFinalCommitEnvironment,
    )
    .await
    .expect_err("a cancelled retry must fail");

    assert!(
        error
            .to_string()
            .contains("cancellation observed before the retry delay"),
        "error must explain the suppressed retry: {}",
        error
    );
    assert!(lock.exists());
    assert_eq!(repo.final_commit_count().await, 0);
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_does_not_retry_another_worktrees_lock() {
    let repo = LockRepo::init().await;
    let other = LockRepo::init().await;
    repo.write_apply_output();
    let mut attempts = 0_u32;

    let error = run_final_commit_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path.clone();
            let other_git_dir = other.path.join(".git");
            async move {
                let stderr = format!(
                    "fatal: Unable to create '{}/index.lock': File exists.\n",
                    other_git_dir.display()
                );
                Err(VcsError::Command {
                    backend: VcsBackend::Git,
                    message: format!("git add -A failed: {}", stderr),
                    command: Some("git add -A".to_string()),
                    working_dir: Some(repo_path),
                    stderr: Some(stderr),
                    stdout: Some(String::new()),
                })
            }
        },
        &GitFinalCommitEnvironment,
        &repo.path,
        &expected_commit_message(),
        None,
    )
    .await
    .expect_err("another worktree's lock is not this finalization's contention");

    assert_eq!(attempts, 1, "an ineligible lock must not be retried");
    assert!(
        !error.to_string().contains("Final Apply commit failed on"),
        "a terminal failure keeps the original error: {}",
        error
    );
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_does_not_retry_malformed_or_permission_lock_output() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    let git_dir = run_git(&["rev-parse", "--absolute-git-dir"], &repo.path)
        .await
        .expect("git dir");

    for stderr in [
        // Permission failure on this repository's own lock path.
        format!("fatal: Unable to create '{git_dir}/index.lock': Permission denied\n"),
        // Truncated/malformed lock prose for the same path.
        format!("fatal: Unable to create '{git_dir}/index.lock\n"),
        format!("fatal: cannot lock '{git_dir}/index.lock': File exists.\n"),
    ] {
        let mut attempts = 0_u32;
        let error = run_final_commit_with_retry(
            || {
                attempts += 1;
                let repo_path = repo.path.clone();
                let stderr = stderr.clone();
                async move {
                    Err(VcsError::Command {
                        backend: VcsBackend::Git,
                        message: format!("git add -A failed: {}", stderr),
                        command: Some("git add -A".to_string()),
                        working_dir: Some(repo_path),
                        stderr: Some(stderr),
                        stdout: Some(String::new()),
                    })
                }
            },
            &GitFinalCommitEnvironment,
            &repo.path,
            &expected_commit_message(),
            None,
        )
        .await
        .expect_err("only an existing-index.lock failure is contention");

        assert_eq!(attempts, 1, "must not retry: {stderr}");
        assert!(
            !error.to_string().contains("Final Apply commit failed on"),
            "a terminal failure keeps the original error: {error}"
        );
    }
    assert_eq!(repo.final_commit_count().await, 0);
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_does_not_retry_configuration_failure() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    // A representative non-lock Git failure: the commit stage cannot build an
    // author identity.
    git_ok(&repo.path, &["config", "user.email", ""]).await;
    git_ok(&repo.path, &["config", "user.name", ""]).await;

    let error = create_final_commit(&repo.workspace_manager(), &repo.path, CHANGE_ID)
        .await
        .expect_err("an identity failure must be terminal");

    // Any retried failure ends in the annotated exhaustion error, so its
    // absence proves the identity failure was never retried.
    assert!(
        !error.to_string().contains("Final Apply commit failed on"),
        "a terminal failure keeps the original error: {}",
        error
    );
    assert_eq!(repo.final_commit_count().await, 0);
}

// ---- ambiguous success ---------------------------------------------------

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_ambiguous_success_is_not_duplicated_on_the_add_path() {
    let repo = LockRepo::init().await;
    repo.write_apply_output();
    let message = expected_commit_message();
    let mut attempts = 0_u32;

    let outcome = run_final_commit_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path.clone();
            let message = message.clone();
            async move {
                // The commit lands, but the command reports contention.
                run_git(&["add", "-A"], &repo_path).await?;
                run_git(&["commit", "-q", "-m", &message], &repo_path).await?;
                Err(fabricated_lock_error(&repo_path, "git add -A").await)
            }
        },
        &GitFinalCommitEnvironment,
        &repo.path,
        &message,
        None,
    )
    .await
    .expect("an ambiguous success must be recognised from repository state");

    assert_eq!(outcome, VerifiedCommitOutcome::Committed);
    assert_eq!(attempts, 1, "an ambiguous success must not be retried");
    assert_eq!(
        repo.commit_subjects().await,
        vec![message, "base".to_string()],
        "no duplicate final commit"
    );
    assert!(repo.hook_runs() >= 1, "the landed commit ran the hook");
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_ambiguous_success_is_not_duplicated_on_the_amend_path() {
    let repo = LockRepo::init().await;
    repo.snapshot_apply_output().await;
    let message = expected_commit_message();
    let mut attempts = 0_u32;

    run_final_commit_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path.clone();
            let message = message.clone();
            async move {
                run_git(
                    &["commit", "-q", "--amend", "--allow-empty", "-m", &message],
                    &repo_path,
                )
                .await?;
                Err(fabricated_lock_error(&repo_path, "git add -A").await)
            }
        },
        &GitFinalCommitEnvironment,
        &repo.path,
        &message,
        None,
    )
    .await
    .expect("an ambiguous amend must be recognised from repository state");

    assert_eq!(attempts, 1);
    assert_eq!(
        repo.commit_subjects().await,
        vec![message, "base".to_string()],
        "the amend replaced the WIP snapshot exactly once"
    );
    assert!(repo.head_file_names().await.contains("applied.txt"));
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_ambiguous_success_rejects_historical_commit() {
    let repo = LockRepo::init().await;
    let message = expected_commit_message();
    // A commit with exactly the expected subject is already at HEAD, and the
    // worktree is clean, so HEAD never advances during finalization.
    repo.write_apply_output();
    git_ok(&repo.path, &["add", "-A"]).await;
    git_ok(&repo.path, &["commit", "-q", "--no-verify", "-m", &message]).await;
    let mut attempts = 0_u32;

    let error = run_final_commit_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path.clone();
            async move { Err(fabricated_lock_error(&repo_path, "git add -A").await) }
        },
        &GitFinalCommitEnvironment,
        &repo.path,
        &message,
        None,
    )
    .await
    .expect_err("an unchanged HEAD is not this finalization's commit");

    assert_eq!(attempts, FINAL_COMMIT_MAX_ATTEMPTS);
    assert!(
        error.to_string().contains("did not clear"),
        "unexpected error: {}",
        error
    );
    assert_eq!(repo.final_commit_count().await, 1);
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_ambiguous_success_rejects_mismatched_tree() {
    let repo = LockRepo::init().await;
    repo.snapshot_apply_output().await;
    let message = expected_commit_message();
    let mut attempts = 0_u32;

    let error = run_final_commit_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path.clone();
            let message = message.clone();
            async move {
                if attempts == 1 {
                    // A commit with the expected subject and lineage lands, but
                    // it records content the amend would not have committed.
                    fs::write(repo_path.join("unexpected.txt"), "not apply output\n").unwrap();
                    run_git(&["add", "-A"], &repo_path).await?;
                    run_git(
                        &["commit", "-q", "--amend", "--allow-empty", "-m", &message],
                        &repo_path,
                    )
                    .await?;
                }
                Err(fabricated_lock_error(&repo_path, "git add -A").await)
            }
        },
        &GitFinalCommitEnvironment,
        &repo.path,
        &message,
        None,
    )
    .await
    .expect_err("a mismatched tree is not this finalization's commit");

    assert_eq!(attempts, FINAL_COMMIT_MAX_ATTEMPTS);
    assert!(error.to_string().contains("did not clear"));
}

// ---- managed lock identity ----------------------------------------------

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn final_apply_commit_lock_environment_reports_managed_lock_identity() {
    let repo = LockRepo::init().await;

    let lock_paths = GitFinalCommitEnvironment
        .lock_paths(&repo.path)
        .await
        .expect("lock paths");

    assert!(!lock_paths.is_empty());
    assert!(lock_paths
        .iter()
        .all(|path| path.file_name().is_some_and(|name| name == "index.lock")));
    assert!(
        lock_paths.contains(&repo.lock_path()),
        "the workspace-derived lock path must be a candidate: {:?}",
        lock_paths
    );
}
