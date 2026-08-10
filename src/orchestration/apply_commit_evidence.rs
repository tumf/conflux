//! Managed-worktree evidence for the final Apply commit.
//!
//! This answers exactly one question, at exactly one moment: when a
//! stop-and-dequeue settles, had Apply already produced its final commit in the
//! managed worktree? An operator who reads "command succeeded" plus
//! `display_status: applying` and concludes "no commit exists" is wrong often
//! enough to create duplicate manual commits, and the API is where that has to
//! be settled.
//!
//! Three rules make the answer trustworthy rather than merely plausible:
//!
//! * **The OID comes from a typed fact, not from the repository.** Only a
//!   non-empty `ApplyCompleted.revision` observed by *this* process incarnation
//!   is a candidate. A commit subject, a task count, a display status, and a log
//!   line are never inputs.
//! * **The worktree comes from the server's own mapping.** No client supplies a
//!   path, and no path is returned.
//! * **Ambiguity stays unknown.** A missing fact, an absent worktree, a Git
//!   failure, or an OID that is not in the worktree's history all produce
//!   `present: None`. The port never reports `false`, because "I could not prove
//!   it" and "it is definitely absent" are different claims and only the first
//!   one is ever true here.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;

use crate::orchestration::execution_facts::ExecutionFactsStore;

/// What the server could prove about the final managed-worktree Apply commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyCommitEvidence {
    /// `Some(true)` when the retained OID was proven to be in the managed
    /// worktree's history. `None` when nothing could be proven.
    ///
    /// Never `Some(false)`: the port distinguishes proof from absence of proof,
    /// and it can only ever establish the first.
    pub present: Option<bool>,
    /// The proven commit OID; `Some` only alongside `present: Some(true)`.
    pub oid: Option<String>,
}

impl ApplyCommitEvidence {
    /// Nothing could be proven.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// The retained OID is in the managed worktree's history.
    pub fn proven(oid: impl Into<String>) -> Self {
        Self {
            present: Some(true),
            oid: Some(oid.into()),
        }
    }
}

/// The repository observation stop settlement makes while the worktree is quiescent.
#[async_trait]
pub trait ApplyCommitEvidencePort: Send + Sync {
    /// Prove — or fail to prove — that `oid` is in the managed worktree's history.
    ///
    /// Called only after confirmed termination, and only with an OID that came
    /// from a typed Apply-completion fact.
    async fn observe(&self, change_id: &str, oid: &str) -> ApplyCommitEvidence;
}

/// Git-backed evidence over the server-owned change-to-worktree mapping.
pub struct GitApplyCommitEvidence {
    repo_root: PathBuf,
}

impl GitApplyCommitEvidence {
    /// Observe worktrees of the repository rooted at `repo_root`.
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    /// True when `oid` equals `HEAD` or is one of its ancestors.
    ///
    /// `git merge-base --is-ancestor` answers through its exit status, so this
    /// reads the status rather than parsing output: a non-zero exit is "not
    /// proven", which is the same answer a spawn failure gives.
    async fn is_ancestor_of_head(worktree: &Path, oid: &str) -> bool {
        let Ok(output) = tokio::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", oid, "HEAD"])
            .current_dir(worktree)
            .stdin(Stdio::null())
            .output()
            .await
        else {
            return false;
        };
        output.status.success()
    }
}

#[async_trait]
impl ApplyCommitEvidencePort for GitApplyCommitEvidence {
    async fn observe(&self, change_id: &str, oid: &str) -> ApplyCommitEvidence {
        if oid.trim().is_empty() {
            return ApplyCommitEvidence::unknown();
        }
        let worktree =
            match crate::vcs::git::get_worktree_path_for_change(&self.repo_root, change_id).await {
                Ok(Some(path)) => path,
                // An absent worktree or a Git failure are the same answer: the
                // evidence could not be read. Neither is proof of absence.
                Ok(None) | Err(_) => return ApplyCommitEvidence::unknown(),
            };
        if !worktree.is_dir() {
            return ApplyCommitEvidence::unknown();
        }
        if Self::is_ancestor_of_head(&worktree, oid.trim()).await {
            ApplyCommitEvidence::proven(oid.trim())
        } else {
            ApplyCommitEvidence::unknown()
        }
    }
}

/// Read the retained typed Apply-completion OID and prove it against the worktree.
///
/// Returns unknown without touching Git when this incarnation observed no
/// completion fact — which is exactly the state a restarted process is in, and
/// the reason the wire contract makes presence nullable.
pub async fn observe_apply_commit(
    facts: Option<&ExecutionFactsStore>,
    port: Option<&dyn ApplyCommitEvidencePort>,
    change_id: &str,
) -> ApplyCommitEvidence {
    let (Some(facts), Some(port)) = (facts, port) else {
        return ApplyCommitEvidence::unknown();
    };
    let Some(oid) = facts.apply_commit_oid(change_id) else {
        return ApplyCommitEvidence::unknown();
    };
    port.observe(change_id, &oid).await
}

#[cfg(test)]
mod tests;
