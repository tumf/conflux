//! Transient WIP snapshot `index.lock` retry policy.
//!
//! Conflux records a WIP snapshot after every apply iteration. A concurrent or
//! recently terminated Git process can leave the managed worktree's
//! `index.lock` momentarily unavailable, which used to fail the whole apply
//! even though the contention cleared milliseconds later.
//!
//! This module owns the narrow policy for that case only: the Conflux-owned
//! snapshot commands, an existing-`index.lock` failure, and a lock that
//! resolves to the managed worktree being snapshotted. Every other VCS failure
//! stays terminal, and Conflux never removes a lock file - it waits and
//! re-attempts the normal Git operations.

use std::future::Future;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::vcs::git::commands::run_git;
use crate::vcs::{VcsBackend, VcsError, VcsResult};

/// Total attempts (initial attempt plus retries) for a single WIP snapshot.
pub const WIP_SNAPSHOT_MAX_ATTEMPTS: u32 = 3;

/// Delay between snapshot attempts.
///
/// Deliberately flat rather than exponential: index-lock contention is
/// short-lived, and backoff only widens the window where finished apply output
/// sits uncommitted in the workspace.
pub const WIP_SNAPSHOT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// Identity of the snapshot whose failures may be retried.
///
/// Both fields are required for a retry decision: the command must be the
/// Conflux-owned snapshot for *this* message, and the reported lock must belong
/// to *this* managed worktree.
#[derive(Debug, Clone)]
pub struct WipSnapshotIdentity {
    /// Exact WIP commit subject this snapshot is trying to record.
    pub wip_message: String,
    /// Absolute `index.lock` paths that identify the managed worktree.
    ///
    /// More than one entry can exist because a git directory may be reported
    /// through a symlinked path (`/var` vs `/private/var` on macOS).
    pub lock_paths: Vec<PathBuf>,
}

impl WipSnapshotIdentity {
    fn matches_lock(&self, reported: &Path) -> bool {
        let reported = lexically_normalize(reported);
        self.lock_paths
            .iter()
            .any(|candidate| lexically_normalize(candidate) == reported)
    }
}

/// Boundary access needed by the retry policy.
///
/// Keeping Git process execution and sleeping behind this trait is what lets
/// the retry policy itself be verified with in-memory doubles.
#[async_trait]
pub trait WipSnapshotEnvironment: Send + Sync {
    /// Resolve the `index.lock` paths that identify the managed worktree.
    async fn lock_paths(&self, workspace_path: &Path) -> VcsResult<Vec<PathBuf>>;

    /// Current HEAD commit, or `None` when the workspace has no commits yet.
    async fn head_commit(&self, workspace_path: &Path) -> VcsResult<Option<String>>;

    /// Parent commits of `commit`, in the order Git reports them.
    async fn commit_parents(&self, workspace_path: &Path, commit: &str) -> VcsResult<Vec<String>>;

    /// Subject line of `commit`.
    async fn commit_subject(&self, workspace_path: &Path, commit: &str) -> VcsResult<String>;

    /// Wait between attempts.
    async fn sleep(&self, duration: Duration);
}

/// Production environment backed by real Git commands and a real timer.
pub struct GitWipSnapshotEnvironment;

/// Record a git directory and its canonical form as the same lock identity.
fn push_git_dir_variants(git_dirs: &mut Vec<PathBuf>, git_dir: PathBuf) {
    let canonical = std::fs::canonicalize(&git_dir).ok();
    for candidate in [Some(git_dir), canonical].into_iter().flatten() {
        if !git_dirs.contains(&candidate) {
            git_dirs.push(candidate);
        }
    }
}

#[async_trait]
impl WipSnapshotEnvironment for GitWipSnapshotEnvironment {
    async fn lock_paths(&self, workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
        let mut git_dirs = Vec::new();
        push_git_dir_variants(
            &mut git_dirs,
            PathBuf::from(run_git(&["rev-parse", "--absolute-git-dir"], workspace_path).await?),
        );

        // A failing command names the git directory the way Git resolved the
        // current directory, which can keep a symlinked prefix that
        // `--absolute-git-dir` already resolved (`/tmp` vs `/private/tmp` on
        // macOS). Rebuilding it from the workspace path covers that form too.
        if let Ok(git_dir) = run_git(&["rev-parse", "--git-dir"], workspace_path).await {
            let git_dir = Path::new(&git_dir);
            let git_dir = if git_dir.is_absolute() {
                git_dir.to_path_buf()
            } else {
                workspace_path.join(git_dir)
            };
            push_git_dir_variants(&mut git_dirs, git_dir);
        }

        Ok(git_dirs
            .into_iter()
            .map(|git_dir| git_dir.join("index.lock"))
            .collect())
    }

    async fn head_commit(&self, workspace_path: &Path) -> VcsResult<Option<String>> {
        // `--quiet` makes an unborn HEAD exit non-zero with empty stderr, which
        // is how a repository without commits is told apart from a real failure.
        match run_git(
            &["rev-parse", "--verify", "--quiet", "HEAD"],
            workspace_path,
        )
        .await
        {
            Ok(head) if head.trim().is_empty() => Ok(None),
            Ok(head) => Ok(Some(head.trim().to_string())),
            Err(VcsError::Command { ref stderr, .. })
                if stderr.as_deref().unwrap_or_default().trim().is_empty() =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    async fn commit_parents(&self, workspace_path: &Path, commit: &str) -> VcsResult<Vec<String>> {
        let line = run_git(
            &["rev-list", "--parents", "-n", "1", commit],
            workspace_path,
        )
        .await?;
        Ok(line
            .split_whitespace()
            .skip(1)
            .map(str::to_string)
            .collect())
    }

    async fn commit_subject(&self, workspace_path: &Path, commit: &str) -> VcsResult<String> {
        run_git(&["log", "-1", "--format=%s", commit], workspace_path).await
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Decide whether a snapshot failure is transient managed-worktree index-lock
/// contention.
///
/// Everything else - permission errors, hook failures, identity/configuration
/// errors, merge conflicts, other lock paths, other repositories, and every
/// non-command VCS error - is terminal.
pub fn is_transient_wip_index_lock_failure(
    error: &VcsError,
    identity: &WipSnapshotIdentity,
) -> bool {
    let VcsError::Command {
        backend: VcsBackend::Git,
        command: Some(command),
        stderr: Some(stderr),
        ..
    } = error
    else {
        return false;
    };

    if !is_wip_snapshot_command(command, &identity.wip_message) {
        return false;
    }

    let Some(lock_path) = parse_existing_index_lock(stderr) else {
        return false;
    };

    identity.matches_lock(&lock_path)
}

/// Whether `command` is one of the two Conflux-owned snapshot commands.
///
/// The command string is the one `run_vcs_command` records, so the commit form
/// carries the exact WIP message this snapshot is recording.
fn is_wip_snapshot_command(command: &str, wip_message: &str) -> bool {
    command == "git add -A"
        || command == format!("git commit --no-verify --allow-empty -m {}", wip_message)
}

/// Extract the `index.lock` path from an existing-lock Git failure.
///
/// Git reports `fatal: Unable to create '<path>': File exists.` for this case;
/// any other lock prose (or any other lock file) is not this failure.
fn parse_existing_index_lock(stderr: &str) -> Option<PathBuf> {
    for line in stderr.lines() {
        let Some((_, rest)) = line.split_once("Unable to create '") else {
            continue;
        };
        let Some((path, tail)) = rest.split_once('\'') else {
            continue;
        };
        if !tail.trim_start().starts_with(": File exists") {
            continue;
        }
        let path = PathBuf::from(path);
        if path.file_name().is_some_and(|name| name == "index.lock") {
            return Some(path);
        }
    }
    None
}

/// Normalize a path without touching the filesystem, so that classification
/// stays a pure decision.
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Run the WIP snapshot sequence, retrying only transient index-lock contention.
///
/// `attempt_snapshot` must perform the complete snapshot sequence (stage plus
/// commit) so that contention at either stage is covered by the same retry.
pub async fn run_wip_snapshot_with_retry<F, Fut>(
    mut attempt_snapshot: F,
    environment: &dyn WipSnapshotEnvironment,
    workspace_path: &Path,
    wip_message: &str,
    cancel_token: Option<&CancellationToken>,
) -> VcsResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = VcsResult<()>>,
{
    let mut identity: Option<WipSnapshotIdentity> = None;

    for attempt in 1..=WIP_SNAPSHOT_MAX_ATTEMPTS {
        // Captured before the attempt so an ambiguous failure can be checked
        // against the exact commit this attempt would have created.
        let head_before = match environment.head_commit(workspace_path).await {
            Ok(head) => Some(head),
            Err(error) => {
                debug!(
                    "Could not resolve HEAD before WIP snapshot attempt {}: {}",
                    attempt, error
                );
                None
            }
        };

        let error = match attempt_snapshot().await {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };

        let Some(head_before) = head_before else {
            // Without a known starting point a retry could duplicate a snapshot
            // the failed attempt already committed, so stop here.
            return Err(annotate_snapshot_failure(
                error,
                attempt,
                "HEAD was unreadable before the attempt, so retrying could duplicate the snapshot",
            ));
        };

        if wip_snapshot_recorded(
            environment,
            workspace_path,
            head_before.as_deref(),
            wip_message,
        )
        .await
        {
            warn!(
                "WIP snapshot command reported failure on attempt {} but the expected snapshot commit exists; treating as success",
                attempt
            );
            return Ok(());
        }

        if identity.is_none() {
            let lock_paths = environment
                .lock_paths(workspace_path)
                .await
                .unwrap_or_else(|error| {
                    debug!("Could not resolve managed worktree lock path: {}", error);
                    Vec::new()
                });
            identity = Some(WipSnapshotIdentity {
                wip_message: wip_message.to_string(),
                lock_paths,
            });
        }
        let identity = identity.as_ref().expect("identity resolved above");

        if !is_transient_wip_index_lock_failure(&error, identity) {
            return Err(error);
        }

        if attempt == WIP_SNAPSHOT_MAX_ATTEMPTS {
            return Err(annotate_snapshot_failure(
                error,
                attempt,
                "managed worktree index.lock contention did not clear within the retry budget",
            ));
        }

        if is_cancelled(cancel_token) {
            return Err(annotate_snapshot_failure(
                error,
                attempt,
                "cancellation observed before the retry delay",
            ));
        }

        warn!(
            "WIP snapshot attempt {}/{} hit managed worktree index.lock contention; retrying in {:?}",
            attempt, WIP_SNAPSHOT_MAX_ATTEMPTS, WIP_SNAPSHOT_RETRY_DELAY
        );
        environment.sleep(WIP_SNAPSHOT_RETRY_DELAY).await;

        if is_cancelled(cancel_token) {
            return Err(annotate_snapshot_failure(
                error,
                attempt,
                "cancellation observed after the retry delay",
            ));
        }
    }

    unreachable!("retry loop returns on the final attempt")
}

fn is_cancelled(cancel_token: Option<&CancellationToken>) -> bool {
    cancel_token.is_some_and(|token| token.is_cancelled())
}

/// Whether the expected WIP commit was actually recorded by a failed attempt.
///
/// Only a commit whose sole parent is `head_before` and whose subject is
/// exactly the expected message counts; a same-subject commit already in
/// history does not, because HEAD never advanced.
async fn wip_snapshot_recorded(
    environment: &dyn WipSnapshotEnvironment,
    workspace_path: &Path,
    head_before: Option<&str>,
    wip_message: &str,
) -> bool {
    let head_now = match environment.head_commit(workspace_path).await {
        Ok(Some(head)) => head,
        _ => return false,
    };
    if Some(head_now.as_str()) == head_before {
        return false;
    }

    let Ok(parents) = environment.commit_parents(workspace_path, &head_now).await else {
        return false;
    };
    let expected_parents: Vec<&str> = head_before.into_iter().collect();
    if parents.iter().map(String::as_str).collect::<Vec<_>>() != expected_parents {
        return false;
    }

    environment
        .commit_subject(workspace_path, &head_now)
        .await
        .is_ok_and(|subject| subject == wip_message)
}

/// Keep the original command, working directory, stderr and stdout while adding
/// the retry outcome, so exhaustion stays diagnosable.
fn annotate_snapshot_failure(error: VcsError, attempt: u32, reason: &str) -> VcsError {
    match error {
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
                "WIP snapshot failed on attempt {}/{} ({}): {}",
                attempt, WIP_SNAPSHOT_MAX_ATTEMPTS, reason, message
            ),
            command,
            working_dir,
            stderr,
            stdout,
        },
        other => other,
    }
}

#[cfg(test)]
#[path = "wip_lock_retry_git_tests.rs"]
mod git_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const WIP_MESSAGE: &str = "WIP: demo-change (1/3 tasks, apply#2)";

    fn lock_stderr(lock_path: &str) -> String {
        format!(
            "fatal: Unable to create '{}': File exists.\n\n\
             Another git process seems to be running in this repository, e.g.\n\
             an editor opened by this repository. Please make sure all processes\n\
             are terminated then try again.\n",
            lock_path
        )
    }

    fn git_command_error(command: &str, stderr: String) -> VcsError {
        VcsError::Command {
            backend: VcsBackend::Git,
            message: format!("{} failed: {}", command, stderr),
            command: Some(command.to_string()),
            working_dir: Some(PathBuf::from("/repo/.openspec-worktrees/demo-change")),
            stderr: Some(stderr),
            stdout: Some(String::new()),
        }
    }

    fn identity() -> WipSnapshotIdentity {
        WipSnapshotIdentity {
            wip_message: WIP_MESSAGE.to_string(),
            lock_paths: vec![PathBuf::from("/repo/.git/worktrees/demo-change/index.lock")],
        }
    }

    fn add_stage_lock_error() -> VcsError {
        git_command_error(
            "git add -A",
            lock_stderr("/repo/.git/worktrees/demo-change/index.lock"),
        )
    }

    fn commit_stage_lock_error() -> VcsError {
        git_command_error(
            &format!("git commit --no-verify --allow-empty -m {}", WIP_MESSAGE),
            lock_stderr("/repo/.git/worktrees/demo-change/index.lock"),
        )
    }

    // ---- classifier -----------------------------------------------------

    #[test]
    fn transient_wip_commit_lock_classifier_accepts_add_stage_contention() {
        assert!(is_transient_wip_index_lock_failure(
            &add_stage_lock_error(),
            &identity()
        ));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_accepts_commit_stage_contention() {
        assert!(is_transient_wip_index_lock_failure(
            &commit_stage_lock_error(),
            &identity()
        ));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_accepts_unnormalized_lock_path() {
        let error = git_command_error(
            "git add -A",
            lock_stderr("/repo/.git/worktrees/demo-change/./index.lock"),
        );
        assert!(is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_other_commands() {
        let error = git_command_error(
            "git status --porcelain",
            lock_stderr("/repo/.git/worktrees/demo-change/index.lock"),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_commit_for_other_message() {
        let error = git_command_error(
            "git commit --no-verify --allow-empty -m WIP: other-change (1/1 tasks, apply#1)",
            lock_stderr("/repo/.git/worktrees/demo-change/index.lock"),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_other_repository_lock() {
        let error = git_command_error(
            "git add -A",
            lock_stderr("/other-repo/.git/worktrees/demo-change/index.lock"),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_other_lock_files() {
        let error = git_command_error(
            "git add -A",
            lock_stderr("/repo/.git/worktrees/demo-change/refs/heads/demo.lock"),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_other_lock_prose() {
        let error = git_command_error(
            "git add -A",
            "fatal: Unable to create '/repo/.git/worktrees/demo-change/index.lock': Permission denied\n".to_string(),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_unrelated_stderr() {
        let error = git_command_error(
            "git add -A",
            "fatal: could not read Username for 'https://github.com'\n".to_string(),
        );
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_non_command_errors() {
        let error = VcsError::git_conflict("index.lock: File exists");
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
        let error = VcsError::UncommittedChanges("index.lock".to_string());
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_non_git_backend() {
        let error = VcsError::Command {
            backend: VcsBackend::Auto,
            message: "failed".to_string(),
            command: Some("git add -A".to_string()),
            working_dir: None,
            stderr: Some(lock_stderr("/repo/.git/worktrees/demo-change/index.lock")),
            stdout: None,
        };
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    #[test]
    fn transient_wip_commit_lock_classifier_rejects_missing_command_context() {
        let error =
            VcsError::git_command(lock_stderr("/repo/.git/worktrees/demo-change/index.lock"));
        assert!(!is_transient_wip_index_lock_failure(&error, &identity()));
    }

    // ---- in-memory environment ------------------------------------------

    #[derive(Clone, Debug)]
    struct FakeCommit {
        id: String,
        parents: Vec<String>,
        subject: String,
    }

    #[derive(Default)]
    struct FakeRepo {
        commits: Vec<FakeCommit>,
    }

    impl FakeRepo {
        fn commit(&mut self, subject: &str) {
            let parents = self
                .commits
                .last()
                .map(|commit| vec![commit.id.clone()])
                .unwrap_or_default();
            let id = format!("commit{}", self.commits.len() + 1);
            self.commits.push(FakeCommit {
                id,
                parents,
                subject: subject.to_string(),
            });
        }
    }

    struct FakeEnvironment {
        repo: Mutex<FakeRepo>,
        lock_paths: Vec<PathBuf>,
        sleeps: Mutex<Vec<Duration>>,
        head_readable: Mutex<bool>,
    }

    impl FakeEnvironment {
        fn new() -> Self {
            let mut repo = FakeRepo::default();
            repo.commit("base commit");
            Self {
                repo: Mutex::new(repo),
                lock_paths: vec![PathBuf::from("/repo/.git/worktrees/demo-change/index.lock")],
                sleeps: Mutex::new(Vec::new()),
                head_readable: Mutex::new(true),
            }
        }

        fn sleeps(&self) -> Vec<Duration> {
            self.sleeps.lock().unwrap().clone()
        }

        fn subjects(&self) -> Vec<String> {
            self.repo
                .lock()
                .unwrap()
                .commits
                .iter()
                .map(|commit| commit.subject.clone())
                .collect()
        }
    }

    #[async_trait]
    impl WipSnapshotEnvironment for FakeEnvironment {
        async fn lock_paths(&self, _workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
            Ok(self.lock_paths.clone())
        }

        async fn head_commit(&self, _workspace_path: &Path) -> VcsResult<Option<String>> {
            if !*self.head_readable.lock().unwrap() {
                return Err(VcsError::git_command("rev-parse failed"));
            }
            Ok(self
                .repo
                .lock()
                .unwrap()
                .commits
                .last()
                .map(|commit| commit.id.clone()))
        }

        async fn commit_parents(
            &self,
            _workspace_path: &Path,
            commit: &str,
        ) -> VcsResult<Vec<String>> {
            self.repo
                .lock()
                .unwrap()
                .commits
                .iter()
                .find(|candidate| candidate.id == commit)
                .map(|candidate| candidate.parents.clone())
                .ok_or_else(|| VcsError::git_command("unknown commit"))
        }

        async fn commit_subject(&self, _workspace_path: &Path, commit: &str) -> VcsResult<String> {
            self.repo
                .lock()
                .unwrap()
                .commits
                .iter()
                .find(|candidate| candidate.id == commit)
                .map(|candidate| candidate.subject.clone())
                .ok_or_else(|| VcsError::git_command("unknown commit"))
        }

        async fn sleep(&self, duration: Duration) {
            self.sleeps.lock().unwrap().push(duration);
        }
    }

    fn workspace() -> &'static Path {
        Path::new("/repo/.openspec-worktrees/demo-change")
    }

    // ---- retry policy ----------------------------------------------------

    #[tokio::test]
    async fn transient_wip_commit_lock_retry_policy_succeeds_on_second_attempt() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);

        let result = run_wip_snapshot_with_retry(
            || async {
                let mut attempts = attempts.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    return Err(add_stage_lock_error());
                }
                environment.repo.lock().unwrap().commit(WIP_MESSAGE);
                Ok(())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await;

        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
        assert_eq!(*attempts.lock().unwrap(), 2);
        assert_eq!(environment.sleeps(), vec![WIP_SNAPSHOT_RETRY_DELAY]);
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), WIP_MESSAGE.to_string()]
        );
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_retry_policy_stops_after_three_attempts() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                Err(commit_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await
        .expect_err("exhausted contention must fail");

        assert_eq!(*attempts.lock().unwrap(), WIP_SNAPSHOT_MAX_ATTEMPTS);
        assert_eq!(
            environment.sleeps(),
            vec![WIP_SNAPSHOT_RETRY_DELAY, WIP_SNAPSHOT_RETRY_DELAY],
            "fixed delay with no backoff"
        );

        let VcsError::Command {
            message,
            command,
            working_dir,
            stderr,
            ..
        } = &error
        else {
            panic!("expected command error, got {:?}", error);
        };
        assert!(message.contains("attempt 3/3"), "message: {}", message);
        assert!(message.contains("did not clear"), "message: {}", message);
        assert!(command.is_some());
        assert!(working_dir.is_some());
        assert!(stderr.as_deref().unwrap_or_default().contains("index.lock"));
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_retry_policy_does_not_retry_other_failures() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                Err(git_command_error(
                    "git commit --no-verify --allow-empty -m WIP: demo-change (1/3 tasks, apply#2)",
                    "fatal: empty ident name not allowed\n".to_string(),
                ))
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await
        .expect_err("non-lock failure must be terminal");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert!(
            !error.to_string().contains("WIP snapshot failed on attempt"),
            "terminal failure must keep the original error: {}",
            error
        );
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_retry_policy_does_not_retry_unreadable_head() {
        let environment = FakeEnvironment::new();
        *environment.head_readable.lock().unwrap() = false;
        let attempts = Mutex::new(0_u32);

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                Err(add_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await
        .expect_err("unreadable HEAD must not be retried");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert!(error.to_string().contains("HEAD was unreadable"));
    }

    // ---- cancellation ----------------------------------------------------

    #[tokio::test]
    async fn transient_wip_commit_lock_cancellation_suppresses_next_attempt() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                Err(add_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            Some(&cancel_token),
        )
        .await
        .expect_err("cancelled retry must fail");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(
            environment.sleeps().is_empty(),
            "cancellation is observed before the retry delay"
        );
        assert!(error.to_string().contains("cancellation observed"));
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_cancellation_after_delay_stops_retry() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);
        let cancel_token = CancellationToken::new();

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                // Cancellation arrives while the retry delay is in progress.
                cancel_token.cancel();
                Err(add_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            Some(&cancel_token),
        )
        .await
        .expect_err("cancelled retry must fail");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(error.to_string().contains("cancellation observed"));
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_cancellation_does_not_affect_success() {
        let environment = FakeEnvironment::new();
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let result = run_wip_snapshot_with_retry(
            || async {
                environment.repo.lock().unwrap().commit(WIP_MESSAGE);
                Ok(())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            Some(&cancel_token),
        )
        .await;

        assert!(
            result.is_ok(),
            "cancellation must not fail a clean snapshot"
        );
    }

    // ---- ambiguous success ------------------------------------------------

    #[tokio::test]
    async fn transient_wip_commit_lock_ambiguous_success_is_not_duplicated() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);

        let result = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                // The commit landed, but the command still reported contention.
                environment.repo.lock().unwrap().commit(WIP_MESSAGE);
                Err(commit_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await;

        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), WIP_MESSAGE.to_string()],
            "no duplicate WIP commit"
        );
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_ambiguous_success_rejects_same_subject_history() {
        let environment = FakeEnvironment::new();
        // A previous run already recorded a commit with the same subject.
        environment.repo.lock().unwrap().commit(WIP_MESSAGE);
        let attempts = Mutex::new(0_u32);

        let error = run_wip_snapshot_with_retry(
            || async {
                *attempts.lock().unwrap() += 1;
                Err(add_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await
        .expect_err("an unchanged HEAD is not a recorded snapshot");

        assert_eq!(*attempts.lock().unwrap(), WIP_SNAPSHOT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }

    #[tokio::test]
    async fn transient_wip_commit_lock_ambiguous_success_rejects_wrong_subject() {
        let environment = FakeEnvironment::new();
        let attempts = Mutex::new(0_u32);

        let error = run_wip_snapshot_with_retry(
            || async {
                let mut attempts = attempts.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    // A commit landed, but it is not the expected snapshot.
                    environment.repo.lock().unwrap().commit("unrelated commit");
                }
                Err(add_stage_lock_error())
            },
            &environment,
            workspace(),
            WIP_MESSAGE,
            None,
        )
        .await
        .expect_err("a different commit is not the expected snapshot");

        assert_eq!(*attempts.lock().unwrap(), WIP_SNAPSHOT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }
}
