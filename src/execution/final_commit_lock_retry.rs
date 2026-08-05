//! Transient final Apply commit `index.lock` retry policy.
//!
//! The hook-enabled final Apply commit is the last repository operation of an
//! apply run. A concurrent Git process - a repository hook, an editor, an
//! external tool - can hold the managed worktree's `index.lock` for a few
//! milliseconds, which used to terminate the whole apply even though the
//! contention cleared immediately and the workspace stayed valid.
//!
//! This module owns the narrow policy for that case only:
//!
//! - the failure is a structured Git `VcsError::Command`,
//! - it comes from one of the finalization commands (`git add -A`, the
//!   add-and-commit form, or the amend form) carrying *this* change's final
//!   commit message,
//! - stderr states that Git could not create an already existing `index.lock`,
//! - and the reported lock resolves to the current managed worktree.
//!
//! Everything else stays terminal. A repository hook rejection is not a
//! failure here at all: it arrives as
//! [`VerifiedCommitOutcome::RepositoryRejected`] and is returned untouched, so
//! it never consumes the lock retry budget and still enters the existing
//! bounded Apply repair flow. Conflux never deletes or bypasses a lock, and it
//! never adds `--no-verify` to the final commit.
//!
//! The policy is deliberately separate from
//! [`crate::execution::wip_lock_retry`]: a WIP snapshot proves completion from
//! a new child commit, while finalization can also *replace* HEAD through
//! `--amend`, so the two need different idempotency proofs. Only the shared
//! lock evidence primitives in [`crate::execution::index_lock`] are common.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::execution::index_lock::{
    is_managed_lock_path, managed_worktree_lock_paths, parse_existing_index_lock,
};
use crate::vcs::git::commands::commit::{
    has_changes_to_commit, verified_commit_args, VerifiedCommitMode,
};
use crate::vcs::git::commands::{parents_of, rev_parse_commit, run_git};
use crate::vcs::{VcsBackend, VcsError, VcsResult, VerifiedCommitOutcome};

/// Total attempts (initial attempt plus retries) for one final Apply commit.
pub const FINAL_COMMIT_MAX_ATTEMPTS: u32 = 3;

/// Delay between finalization attempts.
///
/// Deliberately flat rather than exponential, for the same reason the WIP
/// snapshot policy is: index-lock contention is short-lived, and backoff only
/// widens the window where finished apply output sits uncommitted.
pub const FINAL_COMMIT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// The staging command the add-and-commit finalization path runs.
const FINALIZATION_STAGE_COMMAND: &str = "git add -A";

/// Identity of the finalization whose failures may be retried.
///
/// Both fields are required for a retry decision: the command must be a
/// finalization command for *this* change's final commit message, and the
/// reported lock must belong to *this* managed worktree.
#[derive(Debug, Clone)]
pub struct FinalCommitIdentity {
    /// Exact final commit subject this finalization is trying to record.
    pub commit_message: String,
    /// Absolute `index.lock` paths that identify the managed worktree.
    pub lock_paths: Vec<PathBuf>,
}

/// Render a verified commit command the way `run_vcs_command_captured` records
/// it, so classification compares structured argv rather than prose.
fn rendered_verified_commit(mode: VerifiedCommitMode, commit_message: &str) -> String {
    format!(
        "git {}",
        verified_commit_args(mode, commit_message).join(" ")
    )
}

/// Whether `command` is one of the finalization commands for this change.
///
/// The staging command is shared with other Conflux Git work, so eligibility
/// also depends on the failure arriving inside the finalization retry boundary
/// - a `git add -A` from anywhere else never reaches this classifier.
fn is_finalization_command(command: &str, commit_message: &str) -> bool {
    command == FINALIZATION_STAGE_COMMAND
        || command == rendered_verified_commit(VerifiedCommitMode::AddAndCommit, commit_message)
        || command == rendered_verified_commit(VerifiedCommitMode::Amend, commit_message)
}

/// Decide whether a finalization failure is transient managed-worktree
/// index-lock contention.
///
/// Everything else - permission errors, configuration errors, merge conflicts,
/// other lock paths, other repositories, other commands, other commit
/// messages, non-Git backends, and every non-command VCS error - is terminal.
pub fn is_transient_final_commit_index_lock_failure(
    error: &VcsError,
    identity: &FinalCommitIdentity,
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

    if !is_finalization_command(command, &identity.commit_message) {
        return false;
    }

    let Some(lock_path) = parse_existing_index_lock(stderr) else {
        return false;
    };

    is_managed_lock_path(&identity.lock_paths, &lock_path)
}

/// Boundary access needed by the retry policy.
///
/// Keeping Git process execution and sleeping behind this trait is what lets
/// the retry policy itself be verified with in-memory doubles.
#[async_trait]
pub trait FinalCommitEnvironment: Send + Sync {
    /// Resolve the `index.lock` paths that identify the managed worktree.
    async fn lock_paths(&self, workspace_path: &Path) -> VcsResult<Vec<PathBuf>>;

    /// Current HEAD commit, or `None` when the workspace has no commits yet.
    async fn head_commit(&self, workspace_path: &Path) -> VcsResult<Option<String>>;

    /// Parent commits of `commit`, in the order Git reports them.
    async fn commit_parents(&self, workspace_path: &Path, commit: &str) -> VcsResult<Vec<String>>;

    /// Subject line of `commit`.
    async fn commit_subject(&self, workspace_path: &Path, commit: &str) -> VcsResult<String>;

    /// Tree object recorded by `commit`.
    async fn commit_tree(&self, workspace_path: &Path, commit: &str) -> VcsResult<String>;

    /// Tree the worktree's current content would produce once fully staged.
    ///
    /// This is the tree the add-and-commit path must record, so it is the only
    /// thing that distinguishes this finalization's commit from another commit
    /// that happens to share its subject and its parent.
    async fn workspace_tree(&self, workspace_path: &Path) -> VcsResult<String>;

    /// Whether the worktree currently holds content that is not committed.
    ///
    /// This is the same predicate finalization uses to choose between the
    /// add-and-commit and amend paths.
    async fn has_uncommitted_changes(&self, workspace_path: &Path) -> VcsResult<bool>;

    /// Wait between attempts.
    async fn sleep(&self, duration: Duration);
}

/// Production environment backed by real Git commands and a real timer.
pub struct GitFinalCommitEnvironment;

#[async_trait]
impl FinalCommitEnvironment for GitFinalCommitEnvironment {
    async fn lock_paths(&self, workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
        managed_worktree_lock_paths(workspace_path).await
    }

    async fn head_commit(&self, workspace_path: &Path) -> VcsResult<Option<String>> {
        rev_parse_commit(workspace_path, "HEAD").await
    }

    async fn commit_parents(&self, workspace_path: &Path, commit: &str) -> VcsResult<Vec<String>> {
        parents_of(workspace_path, commit).await
    }

    async fn commit_subject(&self, workspace_path: &Path, commit: &str) -> VcsResult<String> {
        run_git(&["log", "-1", "--format=%s", commit], workspace_path).await
    }

    async fn commit_tree(&self, workspace_path: &Path, commit: &str) -> VcsResult<String> {
        run_git(
            &["rev-parse", &format!("{}^{{tree}}", commit)],
            workspace_path,
        )
        .await
    }

    async fn workspace_tree(&self, workspace_path: &Path) -> VcsResult<String> {
        workspace_content_tree(workspace_path).await
    }

    async fn has_uncommitted_changes(&self, workspace_path: &Path) -> VcsResult<bool> {
        has_changes_to_commit(workspace_path).await
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Compute the tree `git add -A` followed by a commit would record right now.
///
/// The staging is replayed against a throwaway copy of the index, so this stays
/// a pure observation: the managed worktree's real index is never written and
/// its `index.lock` is never taken, which matters because this runs precisely
/// when another process may be holding that lock. Copying the real index rather
/// than reading `HEAD` keeps the replay faithful to whatever the finalization's
/// own `git add -A` would start from.
///
/// The scratch index is driven with `GIT_INDEX_FILE`, spawned here rather than
/// through the shared Git helpers on purpose: a general environment-injecting
/// Git runner in the shared layer would let any caller redirect Git's index or
/// object store, and only this policy needs it.
async fn workspace_content_tree(workspace_path: &Path) -> VcsResult<String> {
    let scratch = tempfile::TempDir::new().map_err(|error| {
        VcsError::git_command(format!(
            "could not create a scratch index directory: {error}"
        ))
    })?;
    let scratch_index = scratch.path().join("index");

    let index_path =
        workspace_path.join(run_git(&["rev-parse", "--git-path", "index"], workspace_path).await?);
    if index_path.exists() {
        tokio::fs::copy(&index_path, &scratch_index)
            .await
            .map_err(|error| {
                VcsError::git_command(format!(
                    "could not copy {} to a scratch index: {error}",
                    index_path.display()
                ))
            })?;
    }

    run_git_with_scratch_index(&["add", "-A"], workspace_path, &scratch_index).await?;
    run_git_with_scratch_index(&["write-tree"], workspace_path, &scratch_index).await
}

/// Run one Git command whose index is redirected to `scratch_index`.
async fn run_git_with_scratch_index(
    args: &[&str],
    workspace_path: &Path,
    scratch_index: &Path,
) -> VcsResult<String> {
    let command = format!("git {}", args.join(" "));
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(workspace_path)
        .env("GIT_INDEX_FILE", scratch_index)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|error| VcsError::Command {
            backend: VcsBackend::Git,
            message: format!("Failed to execute {command}: {error}"),
            command: Some(command.clone()),
            working_dir: Some(workspace_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(VcsError::Command {
            backend: VcsBackend::Git,
            message: format!("{command} failed: {stderr}"),
            command: Some(command),
            working_dir: Some(workspace_path.to_path_buf()),
            stderr: Some(stderr),
            stdout: Some(stdout),
        });
    }

    Ok(stdout.trim().to_string())
}

/// Repository state captured immediately before a finalization attempt.
///
/// This is what makes an ambiguous command result decidable: the exact commit
/// the attempt would have produced can be reconstructed from it, so a commit
/// that already landed is recognised instead of being made twice.
#[derive(Debug, Clone)]
struct FinalizationState {
    /// HEAD before the attempt, or `None` for an unborn branch.
    head: Option<String>,
    /// Parents of `head`, which an amend inherits unchanged.
    parents: Vec<String>,
    /// Tree of `head`, which an amend of a clean worktree preserves.
    tree: Option<String>,
    /// Whether the worktree was dirty, which selects the finalization path.
    dirty: bool,
    /// Tree the dirty worktree's content would produce, i.e. the exact tree the
    /// add-and-commit path must record. `None` on the clean worktree, where the
    /// amend inherits `tree` instead and there is nothing else to stage.
    workspace_tree: Option<String>,
}

async fn capture_finalization_state(
    environment: &dyn FinalCommitEnvironment,
    workspace_path: &Path,
) -> VcsResult<FinalizationState> {
    let head = environment.head_commit(workspace_path).await?;
    let (parents, tree) = match head.as_deref() {
        Some(head) => (
            environment.commit_parents(workspace_path, head).await?,
            Some(environment.commit_tree(workspace_path, head).await?),
        ),
        None => (Vec::new(), None),
    };
    let dirty = environment.has_uncommitted_changes(workspace_path).await?;
    // Captured only on the add-and-commit path, and captured before the
    // attempt: afterwards the workspace is indistinguishable from one another
    // actor rewrote, so it can no longer say what this attempt should record.
    // An unreadable expected tree is an unreadable state, which is terminal
    // rather than retried, for the same reason the rest of the capture is.
    let workspace_tree = match dirty {
        true => Some(environment.workspace_tree(workspace_path).await?),
        false => None,
    };

    Ok(FinalizationState {
        head,
        parents,
        tree,
        dirty,
        workspace_tree,
    })
}

/// Whether the failed attempt actually recorded the final Apply commit.
///
/// A commit counts only when every piece of expected identity holds:
///
/// - HEAD advanced away from the captured commit,
/// - the new commit's subject is exactly the final Apply message,
/// - its lineage is the exact successor of the captured state for the
///   finalization path that state selected (a new child for add-and-commit,
///   the captured commit's own parents for amend),
/// - the committed tree is the expected one for that path (the workspace
///   content captured before the attempt for add-and-commit, the captured
///   commit's inherited tree for amend), and
/// - no workspace content was left out of it.
///
/// A same-subject commit already in history fails the first check because HEAD
/// never moved, and an unrelated or partial commit fails the tree checks - a
/// foreign commit sharing this subject and this parent is still rejected,
/// because it cannot also carry the captured workspace tree.
async fn final_commit_recorded(
    environment: &dyn FinalCommitEnvironment,
    workspace_path: &Path,
    before: &FinalizationState,
    commit_message: &str,
) -> bool {
    let Ok(Some(head_now)) = environment.head_commit(workspace_path).await else {
        return false;
    };
    if Some(head_now.as_str()) == before.head.as_deref() {
        return false;
    }

    let subject = environment
        .commit_subject(workspace_path, &head_now)
        .await
        .ok();
    if subject.as_deref() != Some(commit_message) {
        return false;
    }

    let Ok(parents_now) = environment.commit_parents(workspace_path, &head_now).await else {
        return false;
    };

    if before.dirty {
        // Add-and-commit: the captured HEAD becomes the sole parent, and the
        // recorded tree is exactly the workspace content captured before the
        // attempt. Subject and parent alone are forgeable by any other actor
        // committing on top of the same HEAD, so the tree is what makes this
        // the *expected* commit rather than merely a plausible one.
        let expected_parents: Vec<&str> = before.head.as_deref().into_iter().collect();
        if parents_now.iter().map(String::as_str).collect::<Vec<_>>() != expected_parents {
            return false;
        }
        let Ok(tree_now) = environment.commit_tree(workspace_path, &head_now).await else {
            return false;
        };
        if Some(&tree_now) != before.workspace_tree.as_ref() {
            return false;
        }
    } else {
        // Amend: the captured HEAD is replaced, so its parents and its tree
        // are inherited unchanged. Without a captured HEAD there is nothing to
        // amend, so no observed commit can be this attempt's result.
        if before.head.is_none() || parents_now != before.parents {
            return false;
        }
        let Ok(tree_now) = environment.commit_tree(workspace_path, &head_now).await else {
            return false;
        };
        if Some(&tree_now) != before.tree.as_ref() {
            return false;
        }
    }

    // The commit must contain the whole expected workspace tree: anything left
    // uncommitted means the observed commit is not the finalization result.
    matches!(
        environment.has_uncommitted_changes(workspace_path).await,
        Ok(false)
    )
}

/// Run the final Apply finalization sequence, retrying only transient
/// managed-worktree index-lock contention.
///
/// `attempt_finalization` must perform the *complete* finalization sequence -
/// re-reading whether the worktree needs add-and-commit or amend, staging and
/// validating when dirty, and running the normal hook-enabled commit - so that
/// contention at any stage is covered by one policy and no stale mode decision
/// is replayed after another actor changed repository state.
///
/// It receives the 1-based attempt number. Contention retries re-run the same
/// hooks and therefore reproduce the same output; labelling each attempt is
/// what keeps two identical transcripts attributable to the attempts that
/// produced them instead of looking like one duplicated block.
///
/// `Ok(VerifiedCommitOutcome::RepositoryRejected(_))` is returned immediately:
/// a hook rejection is repository-fixable apply feedback, not contention, and
/// must not consume this budget.
pub async fn run_final_commit_with_retry<F, Fut>(
    mut attempt_finalization: F,
    environment: &dyn FinalCommitEnvironment,
    workspace_path: &Path,
    commit_message: &str,
    cancel_token: Option<&CancellationToken>,
) -> VcsResult<VerifiedCommitOutcome>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = VcsResult<VerifiedCommitOutcome>>,
{
    let mut identity: Option<FinalCommitIdentity> = None;

    for attempt in 1..=FINAL_COMMIT_MAX_ATTEMPTS {
        // Captured before the attempt so an ambiguous failure can be checked
        // against the exact commit this attempt would have created.
        let before = match capture_finalization_state(environment, workspace_path).await {
            Ok(state) => Some(state),
            Err(error) => {
                debug!(
                    "Could not capture repository state before final Apply commit attempt {}: {}",
                    attempt, error
                );
                None
            }
        };

        let error = match attempt_finalization(attempt).await {
            // A repository rejection is a typed apply-repair outcome, not a
            // VCS failure: it leaves this policy untouched.
            Ok(outcome) => return Ok(outcome),
            Err(error) => error,
        };

        let Some(before) = before else {
            // Without a known starting point a retry could duplicate a commit
            // the failed attempt already created, so stop here.
            return Err(annotate_finalization_failure(
                error,
                attempt,
                "repository state was unreadable before the attempt, so retrying could duplicate the final commit",
            ));
        };

        if final_commit_recorded(environment, workspace_path, &before, commit_message).await {
            warn!(
                "Final Apply commit reported failure on attempt {} but the expected commit exists; treating as success",
                attempt
            );
            return Ok(VerifiedCommitOutcome::Committed);
        }

        if identity.is_none() {
            let lock_paths = environment
                .lock_paths(workspace_path)
                .await
                .unwrap_or_else(|error| {
                    debug!("Could not resolve managed worktree lock path: {}", error);
                    Vec::new()
                });
            identity = Some(FinalCommitIdentity {
                commit_message: commit_message.to_string(),
                lock_paths,
            });
        }
        let identity = identity.as_ref().expect("identity resolved above");

        if !is_transient_final_commit_index_lock_failure(&error, identity) {
            return Err(error);
        }

        if attempt == FINAL_COMMIT_MAX_ATTEMPTS {
            return Err(annotate_finalization_failure(
                error,
                attempt,
                "managed worktree index.lock contention did not clear within the retry budget",
            ));
        }

        if is_cancelled(cancel_token) {
            return Err(annotate_finalization_failure(
                error,
                attempt,
                "cancellation observed before the retry delay",
            ));
        }

        warn!(
            "Final Apply commit attempt {}/{} hit managed worktree index.lock contention; retrying in {:?}",
            attempt, FINAL_COMMIT_MAX_ATTEMPTS, FINAL_COMMIT_RETRY_DELAY
        );
        environment.sleep(FINAL_COMMIT_RETRY_DELAY).await;

        if is_cancelled(cancel_token) {
            return Err(annotate_finalization_failure(
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

/// Keep the original command, working directory, stderr and stdout while
/// adding the retry outcome, so exhaustion stays diagnosable.
fn annotate_finalization_failure(error: VcsError, attempt: u32, reason: &str) -> VcsError {
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
                "Final Apply commit failed on attempt {}/{} ({}): {}",
                attempt, FINAL_COMMIT_MAX_ATTEMPTS, reason, message
            ),
            command,
            working_dir,
            stderr,
            stdout,
        },
        other => other,
    }
}

/// Test-only support for repository-level recovery tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::Mutex;

    /// Contents a held lock carries so the policy can be proven not to rewrite it.
    pub(crate) const LOCK_SENTINEL: &str = "held by another git process\n";

    /// A real-Git environment that releases a real `index.lock` at the exact
    /// moment the policy would wait.
    ///
    /// Racing a wall-clock releaser is flaky: the first Git spawn can outlast
    /// any fixed delay, and the attempt then never sees contention at all.
    /// Releasing from `sleep` instead makes the sequence exact - attempt one
    /// always hits the lock, attempt two always finds it gone - and it records
    /// the delays the policy actually asked for.
    pub(crate) struct LockReleasingEnvironment {
        lock: PathBuf,
        sleeps: Mutex<Vec<Duration>>,
        lock_untouched: Mutex<Option<bool>>,
    }

    impl LockReleasingEnvironment {
        /// Hold `lock` with a sentinel the policy must never touch.
        pub(crate) fn holding(lock: PathBuf) -> Self {
            std::fs::write(&lock, LOCK_SENTINEL).expect("hold index.lock");
            Self {
                lock,
                sleeps: Mutex::new(Vec::new()),
                lock_untouched: Mutex::new(None),
            }
        }

        /// Delays the policy asked for, in order.
        pub(crate) fn sleeps(&self) -> Vec<Duration> {
            self.sleeps.lock().unwrap().clone()
        }

        /// Whether the held lock still carried its sentinel every time the
        /// policy waited, i.e. Conflux never deleted or rewrote it.
        pub(crate) fn lock_was_untouched(&self) -> bool {
            self.lock_untouched.lock().unwrap().unwrap_or(false)
        }
    }

    #[async_trait]
    impl FinalCommitEnvironment for LockReleasingEnvironment {
        async fn lock_paths(&self, workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
            GitFinalCommitEnvironment.lock_paths(workspace_path).await
        }

        async fn head_commit(&self, workspace_path: &Path) -> VcsResult<Option<String>> {
            GitFinalCommitEnvironment.head_commit(workspace_path).await
        }

        async fn commit_parents(
            &self,
            workspace_path: &Path,
            commit: &str,
        ) -> VcsResult<Vec<String>> {
            GitFinalCommitEnvironment
                .commit_parents(workspace_path, commit)
                .await
        }

        async fn commit_subject(&self, workspace_path: &Path, commit: &str) -> VcsResult<String> {
            GitFinalCommitEnvironment
                .commit_subject(workspace_path, commit)
                .await
        }

        async fn commit_tree(&self, workspace_path: &Path, commit: &str) -> VcsResult<String> {
            GitFinalCommitEnvironment
                .commit_tree(workspace_path, commit)
                .await
        }

        async fn workspace_tree(&self, workspace_path: &Path) -> VcsResult<String> {
            GitFinalCommitEnvironment
                .workspace_tree(workspace_path)
                .await
        }

        async fn has_uncommitted_changes(&self, workspace_path: &Path) -> VcsResult<bool> {
            GitFinalCommitEnvironment
                .has_uncommitted_changes(workspace_path)
                .await
        }

        async fn sleep(&self, duration: Duration) {
            self.sleeps.lock().unwrap().push(duration);

            let untouched =
                std::fs::read_to_string(&self.lock).ok().as_deref() == Some(LOCK_SENTINEL);
            let mut observed = self.lock_untouched.lock().unwrap();
            *observed = Some(observed.unwrap_or(true) && untouched);

            // The competing process finishes and releases its own lock.
            let _ = std::fs::remove_file(&self.lock);
        }
    }
}

#[cfg(test)]
#[path = "final_commit_lock_retry_git_tests.rs"]
mod git_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcs::CommitRejection;
    use std::sync::Mutex;

    const CHANGE_ID: &str = "demo-change";
    const COMMIT_MESSAGE: &str = "Apply: demo-change";
    const MANAGED_LOCK: &str = "/repo/.git/worktrees/demo-change/index.lock";

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

    fn identity() -> FinalCommitIdentity {
        FinalCommitIdentity {
            commit_message: COMMIT_MESSAGE.to_string(),
            lock_paths: vec![PathBuf::from(MANAGED_LOCK)],
        }
    }

    fn stage_lock_error() -> VcsError {
        git_command_error(FINALIZATION_STAGE_COMMAND, lock_stderr(MANAGED_LOCK))
    }

    fn add_and_commit_lock_error() -> VcsError {
        git_command_error(
            &rendered_verified_commit(VerifiedCommitMode::AddAndCommit, COMMIT_MESSAGE),
            lock_stderr(MANAGED_LOCK),
        )
    }

    fn amend_lock_error() -> VcsError {
        git_command_error(
            &rendered_verified_commit(VerifiedCommitMode::Amend, COMMIT_MESSAGE),
            lock_stderr(MANAGED_LOCK),
        )
    }

    // ---- classifier -----------------------------------------------------

    #[test]
    fn final_apply_commit_lock_classifier_accepts_stage_contention() {
        assert!(is_transient_final_commit_index_lock_failure(
            &stage_lock_error(),
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_accepts_add_and_commit_contention() {
        assert!(is_transient_final_commit_index_lock_failure(
            &add_and_commit_lock_error(),
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_accepts_amend_contention() {
        assert!(is_transient_final_commit_index_lock_failure(
            &amend_lock_error(),
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_covers_every_finalization_command_form() {
        // The classifier must stay in step with the argv finalization actually
        // runs, so it is derived from the same builder rather than re-spelled.
        assert_eq!(
            rendered_verified_commit(VerifiedCommitMode::AddAndCommit, COMMIT_MESSAGE),
            "git commit -m Apply: demo-change"
        );
        assert_eq!(
            rendered_verified_commit(VerifiedCommitMode::Amend, COMMIT_MESSAGE),
            "git commit --amend --allow-empty -m Apply: demo-change"
        );
        for mode in [VerifiedCommitMode::AddAndCommit, VerifiedCommitMode::Amend] {
            let rendered = rendered_verified_commit(mode, COMMIT_MESSAGE);
            assert!(
                !rendered.contains("--no-verify"),
                "final commit forms must stay hook-enabled: {rendered}"
            );
            assert!(is_finalization_command(&rendered, COMMIT_MESSAGE));
        }
    }

    #[test]
    fn final_apply_commit_lock_classifier_accepts_unnormalized_lock_path() {
        let error = git_command_error(
            FINALIZATION_STAGE_COMMAND,
            lock_stderr("/repo/.git/worktrees/demo-change/./index.lock"),
        );
        assert!(is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_other_commands() {
        for command in [
            "git status --porcelain",
            "git diff --cached --name-only --diff-filter=ACMR -z",
            "git reset",
            "git push origin main",
        ] {
            let error = git_command_error(command, lock_stderr(MANAGED_LOCK));
            assert!(
                !is_transient_final_commit_index_lock_failure(&error, &identity()),
                "{command} is not a finalization command"
            );
        }
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_wip_snapshot_commit() {
        let error = git_command_error(
            "git commit --no-verify --allow-empty -m WIP: demo-change (1/3 tasks, apply#2)",
            lock_stderr(MANAGED_LOCK),
        );
        assert!(!is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_another_changes_final_commit() {
        let error = git_command_error(
            "git commit -m Apply: other-change",
            lock_stderr(MANAGED_LOCK),
        );
        assert!(!is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_other_worktree_lock() {
        for reported in [
            "/repo/.git/worktrees/other-change/index.lock",
            "/other-repo/.git/worktrees/demo-change/index.lock",
        ] {
            let error = git_command_error(FINALIZATION_STAGE_COMMAND, lock_stderr(reported));
            assert!(
                !is_transient_final_commit_index_lock_failure(&error, &identity()),
                "{reported} does not belong to this managed worktree"
            );
        }
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_other_lock_files() {
        let error = git_command_error(
            FINALIZATION_STAGE_COMMAND,
            lock_stderr("/repo/.git/worktrees/demo-change/refs/heads/demo.lock"),
        );
        assert!(!is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_malformed_and_near_match_prose() {
        for stderr in [
            format!("fatal: Unable to create '{MANAGED_LOCK}': Permission denied\n"),
            format!("fatal: Unable to create '{MANAGED_LOCK}\n"),
            format!("fatal: cannot lock {MANAGED_LOCK}: File exists.\n"),
            format!("index.lock: File exists ({MANAGED_LOCK})\n"),
            "fatal: could not read Username for 'https://github.com'\n".to_string(),
            "error: Your local changes would be overwritten by merge.\n".to_string(),
        ] {
            let error = git_command_error(FINALIZATION_STAGE_COMMAND, stderr.clone());
            assert!(
                !is_transient_final_commit_index_lock_failure(&error, &identity()),
                "near-match prose must stay terminal: {stderr}"
            );
        }
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_hook_rejection_output() {
        // A hook rejection normally never reaches the classifier, but even its
        // stderr must not be readable as contention.
        let error = git_command_error(
            &rendered_verified_commit(VerifiedCommitMode::AddAndCommit, COMMIT_MESSAGE),
            "repository verification failed: clippy reported warnings\n".to_string(),
        );
        assert!(!is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_non_command_errors() {
        assert!(!is_transient_final_commit_index_lock_failure(
            &VcsError::git_conflict("index.lock: File exists"),
            &identity()
        ));
        assert!(!is_transient_final_commit_index_lock_failure(
            &VcsError::UncommittedChanges("index.lock".to_string()),
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_non_git_backend() {
        let error = VcsError::Command {
            backend: VcsBackend::Auto,
            message: "failed".to_string(),
            command: Some(FINALIZATION_STAGE_COMMAND.to_string()),
            working_dir: None,
            stderr: Some(lock_stderr(MANAGED_LOCK)),
            stdout: None,
        };
        assert!(!is_transient_final_commit_index_lock_failure(
            &error,
            &identity()
        ));
    }

    #[test]
    fn final_apply_commit_lock_classifier_rejects_missing_command_context() {
        assert!(!is_transient_final_commit_index_lock_failure(
            &VcsError::git_command(lock_stderr(MANAGED_LOCK)),
            &identity()
        ));
    }

    // ---- in-memory environment ------------------------------------------

    #[derive(Clone, Debug)]
    struct FakeCommit {
        id: String,
        parents: Vec<String>,
        subject: String,
        tree: String,
    }

    /// Linear in-memory repository with a worktree that can hold uncommitted
    /// content, which is what selects the finalization path.
    #[derive(Default)]
    struct FakeRepo {
        commits: Vec<FakeCommit>,
        next_id: usize,
        /// Content currently in the worktree but not in HEAD, if any.
        pending_tree: Option<String>,
    }

    impl FakeRepo {
        fn new_id(&mut self) -> String {
            self.next_id += 1;
            format!("commit{}", self.next_id)
        }

        fn head(&self) -> Option<&FakeCommit> {
            self.commits.last()
        }

        fn commit(&mut self, subject: &str, tree: &str) {
            let parents = self.head().map(|c| vec![c.id.clone()]).unwrap_or_default();
            let id = self.new_id();
            self.commits.push(FakeCommit {
                id,
                parents,
                subject: subject.to_string(),
                tree: tree.to_string(),
            });
        }

        /// The add-and-commit finalization path.
        fn add_and_commit(&mut self, subject: &str) {
            let tree = self.pending_tree.take().unwrap_or_else(|| {
                self.head()
                    .map(|c| c.tree.clone())
                    .unwrap_or_else(|| "tree-empty".to_string())
            });
            self.commit(subject, &tree);
        }

        /// The amend finalization path: HEAD is replaced, keeping its lineage
        /// and tree.
        fn amend(&mut self, subject: &str) {
            let id = self.new_id();
            let head = self.commits.last_mut().expect("amend requires a HEAD");
            head.id = id;
            head.subject = subject.to_string();
        }

        /// Run the finalization the real workspace manager would run.
        fn finalize(&mut self, subject: &str) {
            if self.pending_tree.is_some() {
                self.add_and_commit(subject);
            } else {
                self.amend(subject);
            }
        }
    }

    struct FakeEnvironment {
        repo: Mutex<FakeRepo>,
        lock_paths: Vec<PathBuf>,
        sleeps: Mutex<Vec<Duration>>,
        state_readable: Mutex<bool>,
    }

    impl FakeEnvironment {
        /// A repository whose WIP snapshot already left the worktree clean, so
        /// finalization amends: the dominant production path.
        fn clean() -> Self {
            let mut repo = FakeRepo::default();
            repo.commit("base commit", "tree-base");
            repo.commit("WIP: demo-change (1/1 tasks, apply#1)", "tree-applied");
            Self::with_repo(repo)
        }

        /// A repository with uncommitted apply output, so finalization stages
        /// and commits.
        fn dirty() -> Self {
            let mut repo = FakeRepo::default();
            repo.commit("base commit", "tree-base");
            repo.pending_tree = Some("tree-applied".to_string());
            Self::with_repo(repo)
        }

        fn with_repo(repo: FakeRepo) -> Self {
            Self {
                repo: Mutex::new(repo),
                lock_paths: vec![PathBuf::from(MANAGED_LOCK)],
                sleeps: Mutex::new(Vec::new()),
                state_readable: Mutex::new(true),
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

        fn find<T>(&self, commit: &str, read: impl Fn(&FakeCommit) -> T) -> VcsResult<T> {
            self.repo
                .lock()
                .unwrap()
                .commits
                .iter()
                .find(|candidate| candidate.id == commit)
                .map(read)
                .ok_or_else(|| VcsError::git_command("unknown commit"))
        }
    }

    #[async_trait]
    impl FinalCommitEnvironment for FakeEnvironment {
        async fn lock_paths(&self, _workspace_path: &Path) -> VcsResult<Vec<PathBuf>> {
            Ok(self.lock_paths.clone())
        }

        async fn head_commit(&self, _workspace_path: &Path) -> VcsResult<Option<String>> {
            if !*self.state_readable.lock().unwrap() {
                return Err(VcsError::git_command("rev-parse failed"));
            }
            Ok(self
                .repo
                .lock()
                .unwrap()
                .head()
                .map(|commit| commit.id.clone()))
        }

        async fn commit_parents(
            &self,
            _workspace_path: &Path,
            commit: &str,
        ) -> VcsResult<Vec<String>> {
            self.find(commit, |commit| commit.parents.clone())
        }

        async fn commit_subject(&self, _workspace_path: &Path, commit: &str) -> VcsResult<String> {
            self.find(commit, |commit| commit.subject.clone())
        }

        async fn commit_tree(&self, _workspace_path: &Path, commit: &str) -> VcsResult<String> {
            self.find(commit, |commit| commit.tree.clone())
        }

        async fn workspace_tree(&self, _workspace_path: &Path) -> VcsResult<String> {
            if !*self.state_readable.lock().unwrap() {
                return Err(VcsError::git_command("write-tree failed"));
            }
            // Staging everything records the pending content when there is any,
            // and otherwise reproduces HEAD - exactly what `FakeRepo::finalize`
            // commits.
            let repo = self.repo.lock().unwrap();
            Ok(repo.pending_tree.clone().unwrap_or_else(|| {
                repo.head()
                    .map(|commit| commit.tree.clone())
                    .unwrap_or_else(|| "tree-empty".to_string())
            }))
        }

        async fn has_uncommitted_changes(&self, _workspace_path: &Path) -> VcsResult<bool> {
            if !*self.state_readable.lock().unwrap() {
                return Err(VcsError::git_command("status failed"));
            }
            Ok(self.repo.lock().unwrap().pending_tree.is_some())
        }

        async fn sleep(&self, duration: Duration) {
            self.sleeps.lock().unwrap().push(duration);
        }
    }

    fn workspace() -> &'static Path {
        Path::new("/repo/.openspec-worktrees/demo-change")
    }

    fn rejection() -> CommitRejection {
        CommitRejection {
            command: rendered_verified_commit(VerifiedCommitMode::Amend, COMMIT_MESSAGE),
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "repository verification failed\n".to_string(),
        }
    }

    // ---- retry policy ----------------------------------------------------

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_succeeds_on_second_attempt() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let outcome = run_final_commit_with_retry(
            |_attempt| async {
                let mut attempts = attempts.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    return Err(amend_lock_error());
                }
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Ok(VerifiedCommitOutcome::Committed)
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("finalization must recover once contention clears");

        assert_eq!(outcome, VerifiedCommitOutcome::Committed);
        assert_eq!(*attempts.lock().unwrap(), 2);
        assert_eq!(environment.sleeps(), vec![FINAL_COMMIT_RETRY_DELAY]);
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), COMMIT_MESSAGE.to_string()],
            "exactly one final Apply commit"
        );
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_recovers_stage_contention() {
        let environment = FakeEnvironment::dirty();
        let attempts = Mutex::new(0_u32);

        run_final_commit_with_retry(
            |_attempt| async {
                let mut attempts = attempts.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    return Err(stage_lock_error());
                }
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Ok(VerifiedCommitOutcome::Committed)
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("staging contention must be retried too");

        assert_eq!(*attempts.lock().unwrap(), 2);
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), COMMIT_MESSAGE.to_string()]
        );
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_stops_after_three_attempts() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Err(add_and_commit_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("exhausted contention must fail");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert_eq!(
            environment.sleeps(),
            vec![FINAL_COMMIT_RETRY_DELAY, FINAL_COMMIT_RETRY_DELAY],
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
        assert_eq!(
            command.as_deref(),
            Some("git commit -m Apply: demo-change"),
            "the original structured command must survive"
        );
        assert_eq!(
            working_dir.as_deref(),
            Some(workspace()),
            "the workspace must stay diagnosable"
        );
        assert!(stderr.as_deref().unwrap_or_default().contains(MANAGED_LOCK));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_does_not_retry_other_failures() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Err(git_command_error(
                    &rendered_verified_commit(VerifiedCommitMode::Amend, COMMIT_MESSAGE),
                    "fatal: empty ident name not allowed\n".to_string(),
                ))
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("a non-lock failure must be terminal");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert!(
            !error.to_string().contains("Final Apply commit failed on"),
            "a terminal failure keeps the original error: {}",
            error
        );
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_does_not_retry_unreadable_state() {
        let environment = FakeEnvironment::clean();
        *environment.state_readable.lock().unwrap() = false;
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("unreadable repository state must not be retried");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert!(error.to_string().contains("state was unreadable"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_returns_hook_rejection_without_retrying() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let outcome = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Ok(VerifiedCommitOutcome::RepositoryRejected(rejection()))
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("a hook rejection is not a VCS failure");

        let VerifiedCommitOutcome::RepositoryRejected(reported) = outcome else {
            panic!("the typed rejection must survive the retry boundary");
        };
        assert_eq!(reported.exit_code, Some(1));
        assert_eq!(
            *attempts.lock().unwrap(),
            1,
            "a hook rejection must not consume the lock retry budget"
        );
        assert!(environment.sleeps().is_empty());
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_repeats_full_preparation_each_attempt() {
        // The worktree starts clean (amend path) and another actor writes new
        // content before the retry, so the attempt must re-read the mode
        // instead of replaying the stale one.
        let environment = FakeEnvironment::clean();
        let observed_dirty = Mutex::new(Vec::<bool>::new());

        run_final_commit_with_retry(
            |_attempt| async {
                let dirty = environment.repo.lock().unwrap().pending_tree.is_some();
                observed_dirty.lock().unwrap().push(dirty);
                if observed_dirty.lock().unwrap().len() == 1 {
                    // Contention on the first attempt, and a competing writer
                    // dirties the worktree while the retry waits.
                    environment.repo.lock().unwrap().pending_tree = Some("tree-late".to_string());
                    return Err(amend_lock_error());
                }
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Ok(VerifiedCommitOutcome::Committed)
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("the retry must re-prepare finalization");

        assert_eq!(
            *observed_dirty.lock().unwrap(),
            vec![false, true],
            "each attempt must re-read the finalization path"
        );
        assert_eq!(
            environment
                .subjects()
                .iter()
                .filter(|subject| *subject == COMMIT_MESSAGE)
                .count(),
            1,
            "exactly one final Apply commit"
        );
    }

    // ---- cancellation ----------------------------------------------------

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_cancellation_suppresses_next_attempt() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            Some(&cancel_token),
        )
        .await
        .expect_err("a cancelled retry must fail");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(
            environment.sleeps().is_empty(),
            "cancellation is observed before the retry delay"
        );
        assert!(error
            .to_string()
            .contains("cancellation observed before the retry delay"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_cancellation_after_delay_stops_retry() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);
        let cancel_token = CancellationToken::new();

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                // Cancellation arrives while the retry delay is in progress.
                cancel_token.cancel();
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            Some(&cancel_token),
        )
        .await
        .expect_err("a cancelled retry must fail");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(error.to_string().contains("cancellation observed"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_retry_policy_cancellation_does_not_affect_success() {
        let environment = FakeEnvironment::clean();
        let cancel_token = CancellationToken::new();
        cancel_token.cancel();

        let outcome = run_final_commit_with_retry(
            |_attempt| async {
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Ok(VerifiedCommitOutcome::Committed)
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            Some(&cancel_token),
        )
        .await
        .expect("cancellation must not fail a clean finalization");

        assert_eq!(outcome, VerifiedCommitOutcome::Committed);
    }

    // ---- ambiguous success ------------------------------------------------

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_is_not_duplicated_on_the_amend_path() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let outcome = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                // The amend landed, but the command still reported contention.
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("an ambiguous success must be recognised");

        assert_eq!(outcome, VerifiedCommitOutcome::Committed);
        assert_eq!(*attempts.lock().unwrap(), 1);
        assert!(environment.sleeps().is_empty());
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), COMMIT_MESSAGE.to_string()],
            "no duplicate final commit"
        );
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_is_not_duplicated_on_the_add_path() {
        let environment = FakeEnvironment::dirty();
        let attempts = Mutex::new(0_u32);

        run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                environment.repo.lock().unwrap().finalize(COMMIT_MESSAGE);
                Err(add_and_commit_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect("an ambiguous success must be recognised");

        assert_eq!(*attempts.lock().unwrap(), 1);
        assert_eq!(
            environment.subjects(),
            vec!["base commit".to_string(), COMMIT_MESSAGE.to_string()]
        );
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_rejects_same_subject_history() {
        let environment = FakeEnvironment::clean();
        // A previous run already recorded a commit with the same subject, but
        // HEAD never advances during this finalization.
        environment
            .repo
            .lock()
            .unwrap()
            .commit(COMMIT_MESSAGE, "tree-applied");
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("an unchanged HEAD is not a recorded final commit");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_rejects_mismatched_tree() {
        let environment = FakeEnvironment::clean();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                if *attempts.lock().unwrap() == 1 {
                    // A commit with the right subject and lineage lands, but it
                    // records different content than the amend would have.
                    let mut repo = environment.repo.lock().unwrap();
                    repo.amend(COMMIT_MESSAGE);
                    repo.commits.last_mut().unwrap().tree = "tree-other".to_string();
                }
                Err(amend_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("a mismatched tree is not this finalization's commit");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_rejects_mismatched_tree_on_the_add_path() {
        let environment = FakeEnvironment::dirty();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                if *attempts.lock().unwrap() == 1 {
                    // Another actor commits on top of the captured HEAD with
                    // exactly this finalization's subject and leaves the
                    // worktree clean, so subject, sole parent and cleanliness
                    // all match - only the recorded content differs from the
                    // workspace this attempt was supposed to commit.
                    let mut repo = environment.repo.lock().unwrap();
                    repo.pending_tree = None;
                    repo.commit(COMMIT_MESSAGE, "tree-other");
                }
                Err(add_and_commit_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("a same-subject, same-parent commit of other content is not this commit");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_rejects_wrong_lineage() {
        let environment = FakeEnvironment::dirty();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                if *attempts.lock().unwrap() == 1 {
                    // Two commits land, so the new HEAD is a grandchild rather
                    // than the exact expected successor.
                    let mut repo = environment.repo.lock().unwrap();
                    repo.commit("unrelated commit", "tree-applied");
                    repo.pending_tree = None;
                    repo.commit(COMMIT_MESSAGE, "tree-applied");
                }
                Err(add_and_commit_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("only the exact expected successor counts");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }

    #[tokio::test]
    async fn final_apply_commit_lock_ambiguous_success_rejects_uncommitted_workspace_content() {
        let environment = FakeEnvironment::dirty();
        let attempts = Mutex::new(0_u32);

        let error = run_final_commit_with_retry(
            |_attempt| async {
                *attempts.lock().unwrap() += 1;
                if *attempts.lock().unwrap() == 1 {
                    // The commit landed but part of the workspace was left out.
                    let mut repo = environment.repo.lock().unwrap();
                    repo.add_and_commit(COMMIT_MESSAGE);
                    repo.pending_tree = Some("tree-left-behind".to_string());
                }
                Err(add_and_commit_lock_error())
            },
            &environment,
            workspace(),
            COMMIT_MESSAGE,
            None,
        )
        .await
        .expect_err("a partial commit is not the expected finalization");

        assert_eq!(*attempts.lock().unwrap(), FINAL_COMMIT_MAX_ATTEMPTS);
        assert!(error.to_string().contains("did not clear"));
    }
}
