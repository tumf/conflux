//! Repository-level tests for the transient WIP snapshot lock retry.
//!
//! These exercise real Git processes against a real temporary repository, so
//! they are integration-scoped evidence rather than unit evidence: they prove
//! the classifier recognises the lock message and lock identity Git actually
//! produces, which no in-memory double can establish.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::{
    run_wip_snapshot_with_retry, GitWipSnapshotEnvironment, WipSnapshotEnvironment,
    WIP_SNAPSHOT_MAX_ATTEMPTS, WIP_SNAPSHOT_RETRY_DELAY,
};
use crate::config::OrchestratorConfig;
use crate::execution::apply::create_progress_commit;
use crate::task_parser::TaskProgress;
use crate::vcs::git::commands::run_git;
use crate::vcs::git::GitWorkspaceManager;
use crate::vcs::{VcsBackend, VcsError};

const CHANGE_ID: &str = "demo-change";
const ITERATION: u32 = 2;

fn progress() -> TaskProgress {
    TaskProgress::with_counts(1, 3)
}

fn expected_wip_message() -> String {
    format!("WIP: {} (1/3 tasks, apply#{})", CHANGE_ID, ITERATION)
}

async fn git_ok(repo: &Path, args: &[&str]) {
    run_git(args, repo)
        .await
        .unwrap_or_else(|error| panic!("git {:?} failed: {}", args, error));
}

/// Initialize a repository holding one base commit plus uncommitted apply output.
async fn init_repo_with_apply_output(repo: &Path) {
    git_ok(repo, &["init", "-q", "-b", "main"]).await;
    git_ok(repo, &["config", "user.email", "test@example.com"]).await;
    git_ok(repo, &["config", "user.name", "Test User"]).await;
    git_ok(repo, &["config", "commit.gpgsign", "false"]).await;
    fs::write(repo.join("base.txt"), "base\n").unwrap();
    git_ok(repo, &["add", "-A"]).await;
    git_ok(repo, &["commit", "-q", "-m", "base"]).await;
    fs::write(repo.join("applied.txt"), "apply output\n").unwrap();
}

fn workspace_manager(repo: &Path) -> GitWorkspaceManager {
    GitWorkspaceManager::new(
        repo.join(".openspec-worktrees"),
        repo.to_path_buf(),
        1,
        OrchestratorConfig::default(),
    )
}

fn hold_index_lock(repo: &Path) -> std::path::PathBuf {
    let lock = repo.join(".git").join("index.lock");
    fs::write(&lock, "held by another git process\n").unwrap();
    lock
}

async fn commit_subjects(repo: &Path) -> Vec<String> {
    run_git(&["log", "--format=%s"], repo)
        .await
        .expect("git log")
        .lines()
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn transient_wip_commit_lock_recovers_when_contention_clears() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    let lock = hold_index_lock(repo.path());

    // The competing process releases the lock inside the retry budget.
    let released = lock.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        fs::remove_file(&released).expect("release index.lock");
    });

    create_progress_commit(
        &workspace_manager(repo.path()),
        repo.path(),
        CHANGE_ID,
        &progress(),
        ITERATION,
        None,
    )
    .await
    .expect("snapshot must recover once contention clears");

    let subjects = commit_subjects(repo.path()).await;
    assert_eq!(
        subjects
            .iter()
            .filter(|subject| *subject == &expected_wip_message())
            .count(),
        1,
        "exactly one WIP commit expected, got {:?}",
        subjects
    );

    // The apply output survived the contention and landed in the snapshot.
    let committed = run_git(&["show", "--name-only", "--format=", "HEAD"], repo.path())
        .await
        .expect("git show");
    assert!(
        committed.contains("applied.txt"),
        "apply output missing from snapshot: {}",
        committed
    );
    assert!(!lock.exists(), "Conflux must never delete a live lock");
}

#[tokio::test]
async fn transient_wip_commit_lock_exhaustion_preserves_workspace() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    let lock = hold_index_lock(repo.path());

    let started = Instant::now();
    let error = create_progress_commit(
        &workspace_manager(repo.path()),
        repo.path(),
        CHANGE_ID,
        &progress(),
        ITERATION,
        None,
    )
    .await
    .expect_err("a lock held past the retry budget must fail");
    assert!(
        started.elapsed() >= WIP_SNAPSHOT_RETRY_DELAY * 2,
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
            WIP_SNAPSHOT_MAX_ATTEMPTS, WIP_SNAPSHOT_MAX_ATTEMPTS
        )),
        "message must report the retry outcome: {}",
        message
    );
    assert_eq!(command.as_deref(), Some("git add -A"));
    assert_eq!(working_dir.as_deref(), Some(repo.path()));
    assert!(stderr.as_deref().unwrap_or_default().contains("index.lock"));

    // The lock is untouched and the apply output is still verifiable.
    assert!(lock.exists(), "Conflux must never delete a live lock");
    assert_eq!(
        fs::read_to_string(repo.path().join("applied.txt")).unwrap(),
        "apply output\n"
    );
    assert_eq!(commit_subjects(repo.path()).await, vec!["base".to_string()]);
}

#[tokio::test]
async fn transient_wip_commit_lock_cancellation_stops_retrying() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    let lock = hold_index_lock(repo.path());
    let cancel_token = CancellationToken::new();
    cancel_token.cancel();

    let error = create_progress_commit(
        &workspace_manager(repo.path()),
        repo.path(),
        CHANGE_ID,
        &progress(),
        ITERATION,
        Some(&cancel_token),
    )
    .await
    .expect_err("cancelled retry must fail");

    assert!(
        error
            .to_string()
            .contains("cancellation observed before the retry delay"),
        "error must explain the suppressed retry: {}",
        error
    );
    assert!(lock.exists());
    assert_eq!(commit_subjects(repo.path()).await, vec!["base".to_string()]);
}

#[tokio::test]
async fn transient_wip_commit_lock_does_not_retry_identity_failure() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    // A representative non-lock Git failure: the commit stage cannot build an
    // author identity.
    git_ok(repo.path(), &["config", "user.email", ""]).await;
    git_ok(repo.path(), &["config", "user.name", ""]).await;

    let error = create_progress_commit(
        &workspace_manager(repo.path()),
        repo.path(),
        CHANGE_ID,
        &progress(),
        ITERATION,
        None,
    )
    .await
    .expect_err("an identity failure must be terminal");

    // Any retried failure ends in the annotated exhaustion error, so its
    // absence proves the identity failure was never retried.
    assert!(
        !error.to_string().contains("WIP snapshot failed on attempt"),
        "a terminal failure keeps the original error: {}",
        error
    );
    assert_eq!(commit_subjects(repo.path()).await, vec!["base".to_string()]);
}

/// Fabricate the contention error Git reports for this repository, so an
/// already-committed attempt can be made to look ambiguous.
async fn fabricated_lock_error(repo: &Path, wip_message: &str) -> VcsError {
    let lock = run_git(&["rev-parse", "--absolute-git-dir"], repo)
        .await
        .expect("git dir");
    let stderr = format!(
        "fatal: Unable to create '{}/index.lock': File exists.\n\n\
         Another git process seems to be running in this repository.\n",
        lock
    );
    VcsError::Command {
        backend: VcsBackend::Git,
        message: format!("git commit failed: {}", stderr),
        command: Some(format!(
            "git commit --no-verify --allow-empty -m {}",
            wip_message
        )),
        working_dir: Some(repo.to_path_buf()),
        stderr: Some(stderr),
        stdout: Some(String::new()),
    }
}

#[tokio::test]
async fn transient_wip_commit_lock_ambiguous_success_is_not_duplicated() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    let wip_message = expected_wip_message();
    let mut attempts = 0_u32;

    let result = run_wip_snapshot_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path();
            let wip_message = wip_message.clone();
            async move {
                // The snapshot lands, but the command reports contention.
                run_git(&["add", "-A"], repo_path).await?;
                run_git(
                    &["commit", "--no-verify", "--allow-empty", "-m", &wip_message],
                    repo_path,
                )
                .await?;
                Err(fabricated_lock_error(repo_path, &wip_message).await)
            }
        },
        &GitWipSnapshotEnvironment,
        repo.path(),
        &wip_message,
        None,
    )
    .await;

    assert!(result.is_ok(), "unexpected error: {:?}", result.err());
    assert_eq!(attempts, 1, "an ambiguous success must not be retried");
    let subjects = commit_subjects(repo.path()).await;
    assert_eq!(
        subjects,
        vec![wip_message.clone(), "base".to_string()],
        "no duplicate WIP commit"
    );
}

#[tokio::test]
async fn transient_wip_commit_lock_ambiguous_success_rejects_historical_commit() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;
    let wip_message = expected_wip_message();
    // A commit with exactly the expected subject is already at HEAD.
    git_ok(repo.path(), &["add", "-A"]).await;
    git_ok(repo.path(), &["commit", "-q", "-m", &wip_message]).await;
    let mut attempts = 0_u32;

    let error = run_wip_snapshot_with_retry(
        || {
            attempts += 1;
            let repo_path = repo.path();
            let wip_message = wip_message.clone();
            // HEAD never advances, so the same-subject history must not be read
            // as this attempt's snapshot.
            async move { Err(fabricated_lock_error(repo_path, &wip_message).await) }
        },
        &GitWipSnapshotEnvironment,
        repo.path(),
        &wip_message,
        None,
    )
    .await
    .expect_err("an unchanged HEAD is not a recorded snapshot");

    assert_eq!(attempts, WIP_SNAPSHOT_MAX_ATTEMPTS);
    assert!(
        error.to_string().contains("did not clear"),
        "unexpected error: {}",
        error
    );
}

#[tokio::test]
async fn transient_wip_commit_lock_environment_reports_managed_lock_identity() {
    let repo = TempDir::new().unwrap();
    init_repo_with_apply_output(repo.path()).await;

    let lock_paths = GitWipSnapshotEnvironment
        .lock_paths(repo.path())
        .await
        .expect("lock paths");

    assert!(!lock_paths.is_empty());
    assert!(lock_paths
        .iter()
        .all(|path| path.file_name().is_some_and(|name| name == "index.lock")));
    assert!(
        lock_paths.contains(&repo.path().join(".git").join("index.lock")),
        "the workspace-derived lock path must be a candidate: {:?}",
        lock_paths
    );
}
