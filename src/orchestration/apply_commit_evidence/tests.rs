//! Tests for managed-worktree Apply-commit evidence.
//!
//! The fake-port cases are unit-scoped: they exercise the retention rule and the
//! "no fact, no Git" contract over in-memory doubles alone. The Git-backed cases
//! are integration evidence by construction — proving an OID is an ancestor of a
//! worktree HEAD is the one thing that cannot be faked without stopping to test
//! anything real — and are kept to temporary repositories with a handful of
//! commits so the suite stays fast.

use super::*;

use std::sync::Mutex;

use crate::events::ExecutionEvent;

/// Records what the coordinator asked for and answers on cue.
#[derive(Default)]
struct FakePort {
    answer: Mutex<ApplyCommitEvidence>,
    calls: Mutex<Vec<(String, String)>>,
}

impl FakePort {
    fn answering(evidence: ApplyCommitEvidence) -> Self {
        Self {
            answer: Mutex::new(evidence),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl ApplyCommitEvidencePort for FakePort {
    async fn observe(&self, change_id: &str, oid: &str) -> ApplyCommitEvidence {
        self.calls
            .lock()
            .unwrap()
            .push((change_id.to_string(), oid.to_string()));
        self.answer.lock().unwrap().clone()
    }
}

fn store_with_apply(change_id: &str, revision: &str) -> ExecutionFactsStore {
    let store = ExecutionFactsStore::new();
    store.observe(
        1,
        &ExecutionEvent::ApplyCompleted {
            change_id: change_id.to_string(),
            revision: revision.to_string(),
        },
        None,
        chrono::Utc::now(),
    );
    store
}

#[tokio::test]
async fn agent_execution_observability_apply_commit_uses_retained_typed_oid() {
    let store = store_with_apply("alpha", "abc123");
    let port = FakePort::answering(ApplyCommitEvidence::proven("abc123"));

    let evidence = observe_apply_commit(Some(&store), Some(&port), "alpha").await;

    assert_eq!(evidence, ApplyCommitEvidence::proven("abc123"));
    assert_eq!(
        port.calls(),
        vec![("alpha".to_string(), "abc123".to_string())]
    );
}

/// No typed completion fact means no repository observation at all: a restarted
/// process must not go looking for a commit it never saw published.
#[tokio::test]
async fn agent_execution_observability_apply_commit_missing_fact_skips_git() {
    let store = ExecutionFactsStore::new();
    let port = FakePort::answering(ApplyCommitEvidence::proven("abc123"));

    let evidence = observe_apply_commit(Some(&store), Some(&port), "alpha").await;

    assert_eq!(evidence, ApplyCommitEvidence::unknown());
    assert!(
        port.calls().is_empty(),
        "a process with no retained completion fact must not run Git"
    );
}

/// An empty revision is not a fact, so it is not a candidate either.
#[tokio::test]
async fn agent_execution_observability_apply_commit_empty_fact_skips_git() {
    let store = store_with_apply("alpha", "");
    let port = FakePort::answering(ApplyCommitEvidence::proven("abc123"));

    assert_eq!(
        observe_apply_commit(Some(&store), Some(&port), "alpha").await,
        ApplyCommitEvidence::unknown()
    );
    assert!(port.calls().is_empty());
}

/// A process with no observability store or no repository port reports unknown.
#[tokio::test]
async fn agent_execution_observability_apply_commit_unbound_reports_unknown() {
    let store = store_with_apply("alpha", "abc123");
    let port = FakePort::answering(ApplyCommitEvidence::proven("abc123"));

    assert_eq!(
        observe_apply_commit(None, Some(&port), "alpha").await,
        ApplyCommitEvidence::unknown()
    );
    assert_eq!(
        observe_apply_commit(Some(&store), None, "alpha").await,
        ApplyCommitEvidence::unknown()
    );
    assert!(port.calls().is_empty());
}

/// The port answers per change, so one change's fact never explains another's.
#[tokio::test]
async fn agent_execution_observability_apply_commit_is_per_change() {
    let store = store_with_apply("alpha", "abc123");
    let port = FakePort::answering(ApplyCommitEvidence::proven("abc123"));

    assert_eq!(
        observe_apply_commit(Some(&store), Some(&port), "beta").await,
        ApplyCommitEvidence::unknown()
    );
    assert!(port.calls().is_empty());
}

// ── Git-backed evidence ────────────────────────────────────────────────────
//
// These touch a real repository, so they are integration evidence rather than
// unit evidence for the retention rule above.

mod git {
    use super::*;
    use std::path::Path;

    async fn git(args: &[&str], cwd: &Path) {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await
            .expect("git must be runnable");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    async fn commit(cwd: &Path, name: &str) -> String {
        std::fs::write(cwd.join(name), name).expect("write file");
        git(&["add", "."], cwd).await;
        git(&["commit", "-m", name], cwd).await;
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cwd)
            .output()
            .await
            .expect("git rev-parse");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// A repository with one worktree checked out on branch `alpha`.
    async fn repo_with_worktree() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        git(&["init", "-b", "main"], &root).await;
        git(&["config", "user.email", "test@example.com"], &root).await;
        git(&["config", "user.name", "Test User"], &root).await;
        commit(&root, "base.txt").await;
        let worktree = root.join("wt-alpha");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "alpha",
                worktree.to_str().expect("utf-8 path"),
            ],
            &root,
        )
        .await;
        (dir, worktree)
    }

    #[tokio::test]
    #[cfg_attr(not(feature = "heavy-tests"), ignore)]
    async fn agent_execution_observability_apply_commit_proves_head_and_ancestor() {
        let (dir, worktree) = repo_with_worktree().await;
        let port = GitApplyCommitEvidence::new(dir.path().to_path_buf());

        let apply_oid = commit(&worktree, "apply.txt").await;
        assert_eq!(
            port.observe("alpha", &apply_oid).await,
            ApplyCommitEvidence::proven(&apply_oid),
            "the retained OID equal to HEAD is proven"
        );

        // A later commit leaves the Apply commit as an ancestor, which is still
        // proof that Apply produced it.
        commit(&worktree, "later.txt").await;
        assert_eq!(
            port.observe("alpha", &apply_oid).await,
            ApplyCommitEvidence::proven(&apply_oid),
            "an ancestor of HEAD is proven"
        );
    }

    /// A commit that exists in the repository but not in this worktree's history
    /// is not proof, and is reported as unknown rather than as absent.
    #[tokio::test]
    #[cfg_attr(not(feature = "heavy-tests"), ignore)]
    async fn agent_execution_observability_apply_commit_non_ancestor_is_unknown() {
        let (dir, worktree) = repo_with_worktree().await;
        let port = GitApplyCommitEvidence::new(dir.path().to_path_buf());
        commit(&worktree, "apply.txt").await;

        // Committed on `main`, with the same subject a worktree commit could
        // have. Subject text is never an input, so this must not be proven.
        let unrelated = commit(dir.path(), "apply.txt").await;

        assert_eq!(
            port.observe("alpha", &unrelated).await,
            ApplyCommitEvidence::unknown()
        );
    }

    /// No worktree for the change, an unresolvable OID, and a non-repository
    /// root are all "could not read", never "definitely absent".
    #[tokio::test]
    #[cfg_attr(not(feature = "heavy-tests"), ignore)]
    async fn agent_execution_observability_apply_commit_unreadable_is_unknown() {
        let (dir, worktree) = repo_with_worktree().await;
        let port = GitApplyCommitEvidence::new(dir.path().to_path_buf());
        let apply_oid = commit(&worktree, "apply.txt").await;

        assert_eq!(
            port.observe("no-such-change", &apply_oid).await,
            ApplyCommitEvidence::unknown(),
            "an absent worktree is unknown"
        );
        assert_eq!(
            port.observe("alpha", "0000000000000000000000000000000000000000")
                .await,
            ApplyCommitEvidence::unknown(),
            "an unresolvable OID is unknown"
        );

        let outside = tempfile::tempdir().expect("tempdir");
        let detached = GitApplyCommitEvidence::new(outside.path().to_path_buf());
        assert_eq!(
            detached.observe("alpha", &apply_oid).await,
            ApplyCommitEvidence::unknown(),
            "a Git failure is unknown"
        );
    }
}
