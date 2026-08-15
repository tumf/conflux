//! Scoped authorization for continuing a change's own unfinished target merge.
//!
//! Bounded sequential resolve can exhaust its agent retries *after* Git has
//! already produced a conflict-free target merge, leaving `MERGE_HEAD` behind.
//! The change correctly returns to manual `merge wait`, but the generic
//! base-dirty preflight in [`crate::parallel::merge`] then sees `MERGE_HEAD` and
//! defers every later attempt — including the operator's explicit `M` retry,
//! whose whole point is to continue that merge.
//!
//! This module holds the narrow exception. An explicit retry carries a
//! [`ManualContinuationAuthorization`]: change-bound, consumed by exactly one
//! dispatch, and never durable workflow state. For that one dispatch, and only
//! while target `MERGE_HEAD` exists, [`classify_manual_continuation`] replaces
//! the generic preflight with a scoped evidence check that proves the merge
//! belongs uniquely to the selected change and that nothing else deviates from
//! `HEAD`.
//!
//! Everything here is side-effect free. The classifier reads evidence through
//! [`ContinuationEvidence`] and returns a decision; it never commits, aborts,
//! stages, resets, or cleans. The worst outcome of a misclassification is a
//! withheld retry, not a wrong mutation.

use async_trait::async_trait;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::vcs::git::commands as git_commands;

use super::resolve_state::EvidenceResult;

/// One-dispatch, change-bound permission to continue an unfinished target merge.
///
/// Created when the scheduler promotes a base-mutating lane waiter that carries
/// operator resolve intent, and consumed by the single dispatch that actually
/// owns the base lane. Occupancy is evaluated *before* consumption, so a retry
/// deferred because another resolve or base mutation holds the lane keeps its
/// authorization for the auto-resumed dispatch.
///
/// Process-local by construction: it lives for one dispatch and is never
/// written anywhere. Workflow routing is still re-derived from the workspace.
#[derive(Debug)]
pub(super) struct ManualContinuationAuthorization {
    change_id: String,
    consumed: AtomicBool,
}

impl ManualContinuationAuthorization {
    /// Authorize exactly one continuation dispatch for `change_id`.
    pub(super) fn new(change_id: impl Into<String>) -> Self {
        Self {
            change_id: change_id.into(),
            consumed: AtomicBool::new(false),
        }
    }

    /// Change this authorization is bound to.
    #[cfg(test)]
    pub(super) fn change_id(&self) -> &str {
        &self.change_id
    }

    /// Consume the authorization for `change_id`.
    ///
    /// Answers `true` exactly once, and only for the bound change: a different
    /// change, or a second dispatch, gets the unchanged generic preflight.
    pub(super) fn consume_for(&self, change_id: &str) -> bool {
        if self.change_id != change_id {
            return false;
        }
        !self.consumed.swap(true, Ordering::SeqCst)
    }

    /// Whether some dispatch already consumed this authorization.
    pub(super) fn is_consumed(&self) -> bool {
        self.consumed.load(Ordering::SeqCst)
    }
}

/// Target working-tree state, split by the porcelain columns the scoped check
/// reasons about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TargetWorkingState {
    /// Paths whose index content differs from `HEAD`.
    pub(super) staged: Vec<String>,
    /// Paths whose worktree content differs from the index.
    pub(super) unstaged: Vec<String>,
    /// Untracked entries; a trailing `/` marks a collapsed untracked directory.
    pub(super) untracked: Vec<String>,
    /// Paths still carrying conflict stages.
    pub(super) unmerged: Vec<String>,
}

/// Porcelain v1 status codes that mean "unmerged", per `git status` docs.
fn is_unmerged_code(index: char, worktree: char) -> bool {
    matches!(
        (index, worktree),
        ('D', 'D') | ('A', 'U') | ('U', 'D') | ('U', 'A') | ('D', 'U') | ('A', 'A') | ('U', 'U')
    )
}

/// Split one porcelain path field, keeping both sides of a `->` rename.
fn porcelain_paths(field: &str) -> Vec<String> {
    let unquote = |value: &str| value.trim().trim_matches('"').to_string();
    match field.split_once(" -> ") {
        Some((from, to)) => vec![unquote(from), unquote(to)],
        None => vec![unquote(field)],
    }
}

/// Classify untrimmed `git status --porcelain` output.
///
/// The two status columns are the entire point: ` M path` (worktree only) and
/// `M  path` (index only) differ by one leading space, and the scoped check
/// admits the second while refusing the first.
pub(super) fn parse_porcelain_status(status: &str) -> TargetWorkingState {
    let mut state = TargetWorkingState::default();
    for line in status.lines() {
        if line.len() < 4 {
            continue;
        }
        let mut chars = line.chars();
        let index = chars.next().unwrap_or(' ');
        let worktree = chars.next().unwrap_or(' ');
        let field = &line[3..];
        if index == '?' && worktree == '?' {
            state.untracked.extend(porcelain_paths(field));
            continue;
        }
        if index == '!' && worktree == '!' {
            continue;
        }
        if is_unmerged_code(index, worktree) {
            state.unmerged.extend(porcelain_paths(field));
            continue;
        }
        if index != ' ' {
            state.staged.extend(porcelain_paths(field));
        }
        if worktree != ' ' {
            state.unstaged.extend(porcelain_paths(field));
        }
    }
    for bucket in [
        &mut state.staged,
        &mut state.unstaged,
        &mut state.untracked,
        &mut state.unmerged,
    ] {
        bucket.sort();
        bucket.dedup();
    }
    state
}

/// Read-only target-repository evidence the scoped check needs.
///
/// Split out as a trait for the same reason
/// [`crate::parallel::resolve_state::ResolveEvidence`] is: the policy below is
/// exercised against in-memory doubles, the adapter holds all of the Git access.
#[async_trait]
pub(super) trait ContinuationEvidence: Send + Sync {
    /// Target `MERGE_HEAD`, when a merge is in progress.
    async fn merge_head(&self) -> EvidenceResult<Option<String>>;

    /// Current target `HEAD` commit.
    async fn head(&self) -> EvidenceResult<String>;

    /// Resolve an admitted branch name to a commit, if it exists.
    async fn resolve_revision(&self, revision: &str) -> EvidenceResult<Option<String>>;

    /// Merge base of two revisions, or `None` when they are unrelated.
    async fn merge_base(&self, a: &str, b: &str) -> EvidenceResult<Option<String>>;

    /// Every path that differs between two revisions.
    async fn changed_paths(&self, from: &str, to: &str) -> EvidenceResult<Vec<String>>;

    /// Target index/worktree/untracked state.
    async fn working_state(&self) -> EvidenceResult<TargetWorkingState>;
}

/// What the scoped check decided about one authorized dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ContinuationDecision {
    /// Evidence proves the unfinished merge is this change's own: sequential
    /// classification may start despite `MERGE_HEAD`.
    Admit {
        /// The proven target `MERGE_HEAD`.
        merge_head: String,
    },
    /// No target merge is in progress, so the generic preflight is still
    /// authoritative and nothing was superseded.
    GenericPreflight,
    /// Fail closed with actionable operator evidence; no agent may start.
    Refuse {
        /// Concrete, operator-facing reason.
        reason: String,
    },
}

fn refuse(reason: impl Into<String>) -> ContinuationDecision {
    ContinuationDecision::Refuse {
        reason: reason.into(),
    }
}

/// Whether an untracked entry collides with a merge-attributable path.
///
/// `--untracked-files=normal` collapses an untracked directory into a single
/// `dir/` entry, so a directory the merge deletes has to be matched by prefix
/// rather than by exact path.
fn untracked_collides(entry: &str, attributable: &BTreeSet<String>) -> bool {
    if entry.ends_with('/') {
        attributable.iter().any(|path| path.starts_with(entry))
    } else {
        attributable.contains(entry)
    }
}

/// Decide whether one authorized manual retry may enter sequential
/// classification while target `MERGE_HEAD` exists.
///
/// Every branch that is not [`ContinuationDecision::Admit`] leaves the
/// repository untouched: this function performs no mutation and its refusals are
/// evaluated before any agent is invoked.
pub(super) async fn classify_manual_continuation(
    evidence: &dyn ContinuationEvidence,
    change_id: &str,
    revisions: &[String],
    change_ids: &[String],
) -> ContinuationDecision {
    if revisions.len() != change_ids.len() {
        return refuse(format!(
            "Manual resolve continuation for '{}' received {} revisions for {} change ids",
            change_id,
            revisions.len(),
            change_ids.len()
        ));
    }

    let merge_head = match evidence.merge_head().await {
        Ok(Some(merge_head)) => merge_head,
        // Nothing to continue: the unchanged generic preflight decides.
        Ok(None) => return ContinuationDecision::GenericPreflight,
        Err(error) => {
            return refuse(format!("Failed to read target MERGE_HEAD: {}", error));
        }
    };

    // Ownership first. An in-progress merge that is not provably this change's
    // own is exactly the state that must never be continued.
    let mut owners = Vec::new();
    for (revision, candidate) in revisions.iter().zip(change_ids.iter()) {
        match evidence.resolve_revision(revision).await {
            Ok(Some(tip)) => {
                if tip == merge_head {
                    owners.push(candidate.clone());
                }
            }
            Ok(None) => {
                return refuse(format!(
                    "Admitted branch '{}' for '{}' does not resolve to a commit; target MERGE_HEAD {} ownership is unprovable",
                    revision, candidate, merge_head
                ));
            }
            Err(error) => {
                return refuse(format!(
                    "Failed to resolve admitted branch '{}' for '{}': {}",
                    revision, candidate, error
                ));
            }
        }
    }
    match owners.as_slice() {
        [owner] if owner == change_id => {}
        [owner] => {
            return refuse(format!(
                "Target MERGE_HEAD {} belongs to '{}', not to the retried change '{}'",
                merge_head, owner, change_id
            ));
        }
        [] => {
            return refuse(format!(
                "Target MERGE_HEAD {} matches no admitted branch tip for '{}'",
                merge_head, change_id
            ));
        }
        _ => {
            return refuse(format!(
                "Target MERGE_HEAD {} matches {} admitted branch tips; ownership is ambiguous",
                merge_head,
                owners.len()
            ));
        }
    }

    let state = match evidence.working_state().await {
        Ok(state) => state,
        Err(error) => {
            return refuse(format!("Failed to read target working state: {}", error));
        }
    };

    if !state.unmerged.is_empty() {
        return refuse(format!(
            "Target merge for '{}' still has unresolved conflicts: {}",
            change_id,
            state.unmerged.join(", ")
        ));
    }

    if !state.unstaged.is_empty() {
        return refuse(format!(
            "Target working tree does not match the index; unstaged changes are not attributable to the merge for '{}': {}",
            change_id,
            state.unstaged.join(", ")
        ));
    }

    // Attribution: the staged result may only contain paths the merged branch
    // itself changed since the merge base.
    let head = match evidence.head().await {
        Ok(head) => head,
        Err(error) => return refuse(format!("Failed to read target HEAD: {}", error)),
    };
    let base = match evidence.merge_base(&head, &merge_head).await {
        Ok(Some(base)) => base,
        Ok(None) => {
            return refuse(format!(
                "Target HEAD {} and MERGE_HEAD {} have no merge base; merge topology is invalid",
                head, merge_head
            ));
        }
        Err(error) => {
            return refuse(format!(
                "Failed to read the merge base of target HEAD {} and MERGE_HEAD {}: {}",
                head, merge_head, error
            ));
        }
    };
    let attributable: BTreeSet<String> = match evidence.changed_paths(&base, &merge_head).await {
        Ok(paths) => paths.into_iter().collect(),
        Err(error) => {
            return refuse(format!(
                "Failed to read the paths merged from {} into '{}': {}",
                merge_head, change_id, error
            ));
        }
    };

    let unrelated_staged: Vec<&String> = state
        .staged
        .iter()
        .filter(|path| !attributable.contains(*path))
        .collect();
    if !unrelated_staged.is_empty() {
        return refuse(format!(
            "Target index holds staged content the merge for '{}' did not produce: {}",
            change_id,
            unrelated_staged
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let conflicting_untracked: Vec<&String> = state
        .untracked
        .iter()
        .filter(|entry| untracked_collides(entry, &attributable))
        .collect();
    if !conflicting_untracked.is_empty() {
        return refuse(format!(
            "Untracked paths conflict with the in-progress merge for '{}': {}",
            change_id,
            conflicting_untracked
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    ContinuationDecision::Admit { merge_head }
}

/// Git-backed [`ContinuationEvidence`] over a target repository root.
pub(super) struct GitContinuationEvidence {
    repo_root: PathBuf,
}

impl GitContinuationEvidence {
    /// Build an adapter rooted at the target repository.
    pub(super) fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl ContinuationEvidence for GitContinuationEvidence {
    async fn merge_head(&self) -> EvidenceResult<Option<String>> {
        git_commands::merge_head(&self.repo_root)
            .await
            .map_err(|error| error.to_string())
    }

    async fn head(&self) -> EvidenceResult<String> {
        git_commands::rev_parse_commit(&self.repo_root, "HEAD")
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "target repository has no HEAD commit".to_string())
    }

    async fn resolve_revision(&self, revision: &str) -> EvidenceResult<Option<String>> {
        git_commands::rev_parse_commit(&self.repo_root, revision)
            .await
            .map_err(|error| error.to_string())
    }

    async fn merge_base(&self, a: &str, b: &str) -> EvidenceResult<Option<String>> {
        git_commands::merge_base(&self.repo_root, a, b)
            .await
            .map_err(|error| error.to_string())
    }

    async fn changed_paths(&self, from: &str, to: &str) -> EvidenceResult<Vec<String>> {
        git_commands::diff_paths_between(&self.repo_root, from, to)
            .await
            .map_err(|error| error.to_string())
    }

    async fn working_state(&self) -> EvidenceResult<TargetWorkingState> {
        let status = git_commands::porcelain_status(&self.repo_root)
            .await
            .map_err(|error| error.to_string())?;
        Ok(parse_porcelain_status(&status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory [`ContinuationEvidence`] double: no repository, no process.
    struct FakeEvidence {
        merge_head: Result<Option<String>, String>,
        head: Result<String, String>,
        tips: Vec<(String, Result<Option<String>, String>)>,
        merge_base: Result<Option<String>, String>,
        changed: Result<Vec<String>, String>,
        state: Result<TargetWorkingState, String>,
    }

    impl FakeEvidence {
        /// The healthy case: `alpha`'s branch tip owns a conflict-free merge.
        fn healthy() -> Self {
            Self {
                merge_head: Ok(Some("tip-alpha".to_string())),
                head: Ok("head-1".to_string()),
                tips: vec![("cflx-alpha".to_string(), Ok(Some("tip-alpha".to_string())))],
                merge_base: Ok(Some("base-1".to_string())),
                changed: Ok(vec![
                    "openspec/changes/alpha/proposal.md".to_string(),
                    "src/lib.rs".to_string(),
                ]),
                state: Ok(TargetWorkingState {
                    staged: vec!["src/lib.rs".to_string()],
                    ..Default::default()
                }),
            }
        }
    }

    #[async_trait]
    impl ContinuationEvidence for FakeEvidence {
        async fn merge_head(&self) -> EvidenceResult<Option<String>> {
            self.merge_head.clone()
        }
        async fn head(&self) -> EvidenceResult<String> {
            self.head.clone()
        }
        async fn resolve_revision(&self, revision: &str) -> EvidenceResult<Option<String>> {
            self.tips
                .iter()
                .find(|(name, _)| name == revision)
                .map(|(_, tip)| tip.clone())
                .unwrap_or(Ok(None))
        }
        async fn merge_base(&self, _a: &str, _b: &str) -> EvidenceResult<Option<String>> {
            self.merge_base.clone()
        }
        async fn changed_paths(&self, _from: &str, _to: &str) -> EvidenceResult<Vec<String>> {
            self.changed.clone()
        }
        async fn working_state(&self) -> EvidenceResult<TargetWorkingState> {
            self.state.clone()
        }
    }

    async fn classify(evidence: &FakeEvidence) -> ContinuationDecision {
        classify_manual_continuation(
            evidence,
            "alpha",
            &["cflx-alpha".to_string()],
            &["alpha".to_string()],
        )
        .await
    }

    fn refusal(decision: ContinuationDecision) -> String {
        match decision {
            ContinuationDecision::Refuse { reason } => reason,
            other => panic!("expected a scoped refusal, got {:?}", other),
        }
    }

    #[test]
    fn manual_resolve_authorization_is_change_bound_and_consumed_once() {
        let auth = ManualContinuationAuthorization::new("alpha");
        assert_eq!(auth.change_id(), "alpha");
        assert!(!auth.is_consumed());
        assert!(
            !auth.consume_for("beta"),
            "an authorization must never admit a different change"
        );
        assert!(
            !auth.is_consumed(),
            "a rejected change must not consume the authorization"
        );
        assert!(auth.consume_for("alpha"));
        assert!(auth.is_consumed());
        assert!(
            !auth.consume_for("alpha"),
            "a second dispatch must fall back to the generic preflight"
        );
    }

    #[tokio::test]
    async fn manual_resolve_continuation_admits_its_own_conflict_free_merge() {
        let decision = classify(&FakeEvidence::healthy()).await;
        assert_eq!(
            decision,
            ContinuationDecision::Admit {
                merge_head: "tip-alpha".to_string()
            }
        );
    }

    #[tokio::test]
    async fn manual_resolve_continuation_defers_to_generic_preflight_without_merge_head() {
        let evidence = FakeEvidence {
            merge_head: Ok(None),
            ..FakeEvidence::healthy()
        };
        assert_eq!(
            classify(&evidence).await,
            ContinuationDecision::GenericPreflight
        );
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_foreign_merge_head() {
        let evidence = FakeEvidence {
            merge_head: Ok(Some("tip-beta".to_string())),
            tips: vec![
                ("cflx-alpha".to_string(), Ok(Some("tip-alpha".to_string()))),
                ("cflx-beta".to_string(), Ok(Some("tip-beta".to_string()))),
            ],
            ..FakeEvidence::healthy()
        };
        let decision = classify_manual_continuation(
            &evidence,
            "alpha",
            &["cflx-alpha".to_string(), "cflx-beta".to_string()],
            &["alpha".to_string(), "beta".to_string()],
        )
        .await;
        assert!(
            refusal(decision).contains("belongs to 'beta'"),
            "a foreign MERGE_HEAD must name its real owner"
        );
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unmatched_merge_head() {
        let evidence = FakeEvidence {
            merge_head: Ok(Some("tip-unknown".to_string())),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("matches no admitted branch tip"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_ambiguous_merge_head() {
        let evidence = FakeEvidence {
            merge_head: Ok(Some("tip-shared".to_string())),
            tips: vec![
                ("cflx-alpha".to_string(), Ok(Some("tip-shared".to_string()))),
                ("cflx-beta".to_string(), Ok(Some("tip-shared".to_string()))),
            ],
            ..FakeEvidence::healthy()
        };
        let decision = classify_manual_continuation(
            &evidence,
            "alpha",
            &["cflx-alpha".to_string(), "cflx-beta".to_string()],
            &["alpha".to_string(), "beta".to_string()],
        )
        .await;
        assert!(refusal(decision).contains("ambiguous"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unresolvable_branch_evidence() {
        let evidence = FakeEvidence {
            tips: vec![("cflx-alpha".to_string(), Ok(None))],
            merge_head: Ok(Some("tip-alpha".to_string())),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("does not resolve to a commit"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unreadable_merge_head() {
        let evidence = FakeEvidence {
            merge_head: Err("permission denied".to_string()),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("Failed to read target MERGE_HEAD"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unresolved_conflicts() {
        let evidence = FakeEvidence {
            state: Ok(TargetWorkingState {
                unmerged: vec!["src/lib.rs".to_string()],
                ..Default::default()
            }),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("unresolved conflicts"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unstaged_modification() {
        let evidence = FakeEvidence {
            state: Ok(TargetWorkingState {
                staged: vec!["src/lib.rs".to_string()],
                unstaged: vec!["docs/README.md".to_string()],
                ..Default::default()
            }),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("does not match the index"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unrelated_staged_content() {
        let evidence = FakeEvidence {
            state: Ok(TargetWorkingState {
                staged: vec!["src/lib.rs".to_string(), "unrelated.txt".to_string()],
                ..Default::default()
            }),
            ..FakeEvidence::healthy()
        };
        let reason = refusal(classify(&evidence).await);
        assert!(reason.contains("staged content the merge"));
        assert!(reason.contains("unrelated.txt"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_conflicting_untracked_directory() {
        let evidence = FakeEvidence {
            state: Ok(TargetWorkingState {
                staged: vec!["src/lib.rs".to_string()],
                untracked: vec!["openspec/changes/alpha/".to_string()],
                ..Default::default()
            }),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("Untracked paths conflict"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_tolerates_unrelated_untracked_paths() {
        let evidence = FakeEvidence {
            state: Ok(TargetWorkingState {
                staged: vec!["src/lib.rs".to_string()],
                untracked: vec!["target/debug/".to_string()],
                ..Default::default()
            }),
            ..FakeEvidence::healthy()
        };
        assert!(matches!(
            classify(&evidence).await,
            ContinuationDecision::Admit { .. }
        ));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_invalid_merge_topology() {
        let evidence = FakeEvidence {
            merge_base: Ok(None),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("no merge base"));
    }

    #[tokio::test]
    async fn manual_resolve_continuation_rejects_unreadable_merge_paths() {
        let evidence = FakeEvidence {
            changed: Err("bad object".to_string()),
            ..FakeEvidence::healthy()
        };
        assert!(refusal(classify(&evidence).await).contains("Failed to read the paths merged"));
    }

    #[test]
    fn manual_resolve_porcelain_parse_separates_index_and_worktree_columns() {
        let state = parse_porcelain_status(
            "M  staged.txt\n M unstaged.txt\nMM both.txt\nUU conflicted.txt\n?? new/\n!! ignored.txt\nR  old.txt -> new.txt\n",
        );
        assert_eq!(
            state.staged,
            vec![
                "both.txt".to_string(),
                "new.txt".to_string(),
                "old.txt".to_string(),
                "staged.txt".to_string()
            ]
        );
        assert_eq!(
            state.unstaged,
            vec!["both.txt".to_string(), "unstaged.txt".to_string()]
        );
        assert_eq!(state.unmerged, vec!["conflicted.txt".to_string()]);
        assert_eq!(state.untracked, vec!["new/".to_string()]);
    }
}
