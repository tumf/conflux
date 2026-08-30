//! In-memory doubles for sequential resolve classification tests.
//!
//! The classifier holds all of the policy and [`ResolveEvidence`] holds all of
//! the Git access, so the complete decision table is exercisable without a
//! process, a filesystem, or a repository. One shared double keeps the
//! classifier's own tests and the conflict-layer retry tests honest about the
//! same evidence shape.

use super::{EvidenceResult, ResolveEvidence, SequentialMergeItem};
use crate::vcs::git::commands::{CommitDiffEntry, WorktreeIdentity};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Minimal in-memory commit graph.
///
/// Real ancestry, first-parent lineage, and subject-range queries are computed
/// from it, so the classifier's decision table is exercised without a process
/// or filesystem boundary.
#[derive(Default, Clone)]
pub(crate) struct FakeRepo {
    pub(crate) parents: HashMap<String, Vec<String>>,
    pub(crate) subjects: HashMap<String, String>,
    pub(crate) trees: HashMap<String, Vec<String>>,
    pub(crate) diffs: HashMap<String, Vec<CommitDiffEntry>>,
    /// Committed blob text, keyed by `(revision, repository-relative path)`.
    pub(crate) files: HashMap<(String, String), String>,
}

impl FakeRepo {
    pub(crate) fn commit(&mut self, id: &str, subject: &str, parents: &[&str]) -> &mut Self {
        self.parents.insert(
            id.to_string(),
            parents.iter().map(|p| p.to_string()).collect(),
        );
        self.subjects.insert(id.to_string(), subject.to_string());
        self
    }

    pub(crate) fn tree(&mut self, id: &str, paths: &[&str]) -> &mut Self {
        self.trees.insert(
            id.to_string(),
            paths.iter().map(|p| p.to_string()).collect(),
        );
        self
    }

    /// Record a committed file at `id`, adding its path to that revision's tree.
    ///
    /// Tree membership and blob content are set together because Git cannot
    /// have one without the other, and a fixture that could would let a test
    /// pass against a state the classifier can never see.
    pub(crate) fn file(&mut self, id: &str, path: &str, content: &str) -> &mut Self {
        let tree = self.trees.entry(id.to_string()).or_default();
        if !tree.iter().any(|existing| existing == path) {
            tree.push(path.to_string());
        }
        self.files
            .insert((id.to_string(), path.to_string()), content.to_string());
        self
    }

    /// Record a committed file whose blob exists in no tree, as an unreadable
    /// path does.
    pub(crate) fn tree_only(&mut self, id: &str, path: &str) -> &mut Self {
        let tree = self.trees.entry(id.to_string()).or_default();
        if !tree.iter().any(|existing| existing == path) {
            tree.push(path.to_string());
        }
        self
    }

    pub(crate) fn diff(&mut self, id: &str, entries: &[(char, &str)]) -> &mut Self {
        self.diffs.insert(
            id.to_string(),
            entries
                .iter()
                .map(|(status, path)| CommitDiffEntry {
                    status: *status,
                    path: path.to_string(),
                })
                .collect(),
        );
        self
    }

    /// Reachable commits including `id` itself.
    pub(crate) fn ancestors(&self, id: &str) -> Vec<String> {
        let mut seen = Vec::new();
        let mut stack = vec![id.to_string()];
        while let Some(current) = stack.pop() {
            if seen.contains(&current) {
                continue;
            }
            seen.push(current.clone());
            for parent in self.parents.get(&current).into_iter().flatten() {
                stack.push(parent.clone());
            }
        }
        seen
    }

    pub(crate) fn first_parent_lineage(&self, tip: &str) -> Vec<String> {
        let mut lineage = Vec::new();
        let mut current = tip.to_string();
        loop {
            if lineage.contains(&current) {
                break;
            }
            lineage.push(current.clone());
            match self.parents.get(&current).and_then(|p| p.first()) {
                Some(parent) => current = parent.clone(),
                None => break,
            }
        }
        lineage
    }
}

pub(crate) struct FakeEvidence {
    pub(crate) repo: FakeRepo,
    pub(crate) identities: HashMap<String, WorktreeIdentity>,
    pub(crate) worktree_merges: Vec<PathBuf>,
    pub(crate) worktree_conflicts: HashMap<PathBuf, Vec<String>>,
    pub(crate) head: String,
    pub(crate) merge_head: Option<String>,
    pub(crate) conflict_paths: Vec<String>,
    pub(crate) clean: bool,
    pub(crate) index_paths: Vec<String>,
    /// `(revision, path)` pairs whose blob read fails outright.
    pub(crate) unreadable: Vec<(String, String)>,
}

impl FakeEvidence {
    pub(crate) fn new(repo: FakeRepo, head: &str) -> Self {
        Self {
            repo,
            identities: HashMap::new(),
            worktree_merges: Vec::new(),
            worktree_conflicts: HashMap::new(),
            head: head.to_string(),
            merge_head: None,
            conflict_paths: Vec::new(),
            clean: true,
            index_paths: Vec::new(),
            unreadable: Vec::new(),
        }
    }

    /// Make one committed path fail to read, as a damaged object would.
    pub(crate) fn unreadable(mut self, revision: &str, path: &str) -> Self {
        self.unreadable
            .push((revision.to_string(), path.to_string()));
        self
    }

    pub(crate) fn worktree(mut self, branch: &str, path: &str, tip: &str) -> Self {
        self.identities.insert(
            branch.to_string(),
            WorktreeIdentity::Supplied {
                path: PathBuf::from(path),
                tip: tip.to_string(),
            },
        );
        self
    }

    pub(crate) fn unsafe_worktree(mut self, branch: &str, reason: &str) -> Self {
        self.identities.insert(
            branch.to_string(),
            WorktreeIdentity::Unsafe {
                reason: reason.to_string(),
            },
        );
        self
    }

    pub(crate) fn merge_head(mut self, tip: &str) -> Self {
        self.merge_head = Some(tip.to_string());
        self
    }

    pub(crate) fn index(mut self, paths: &[&str]) -> Self {
        self.index_paths = paths.iter().map(|p| p.to_string()).collect();
        self
    }

    pub(crate) fn conflicts(mut self, paths: &[&str]) -> Self {
        self.conflict_paths = paths.iter().map(|p| p.to_string()).collect();
        self
    }

    pub(crate) fn dirty(mut self) -> Self {
        self.clean = false;
        self
    }

    pub(crate) fn worktree_merging(mut self, path: &str) -> Self {
        self.worktree_merges.push(PathBuf::from(path));
        self
    }

    pub(crate) fn worktree_conflicted(mut self, path: &str, files: &[&str]) -> Self {
        self.worktree_conflicts.insert(
            PathBuf::from(path),
            files.iter().map(|f| f.to_string()).collect(),
        );
        self
    }
}

#[async_trait]
impl ResolveEvidence for FakeEvidence {
    async fn validate_worktree(
        &self,
        supplied_path: &Path,
        expected_branch: &str,
    ) -> WorktreeIdentity {
        self.identities
            .get(expected_branch)
            .cloned()
            .unwrap_or(WorktreeIdentity::Unsafe {
                reason: format!(
                    "no worktree metadata for branch '{}' (supplied {})",
                    expected_branch,
                    supplied_path.display()
                ),
            })
    }

    async fn worktree_merge_in_progress(&self, worktree: &Path) -> EvidenceResult<bool> {
        Ok(self.worktree_merges.iter().any(|p| p == worktree))
    }

    async fn worktree_conflicts(&self, worktree: &Path) -> EvidenceResult<Vec<String>> {
        Ok(self
            .worktree_conflicts
            .get(worktree)
            .cloned()
            .unwrap_or_default())
    }

    async fn target_head(&self) -> EvidenceResult<String> {
        Ok(self.head.clone())
    }

    async fn target_merge_head(&self) -> EvidenceResult<Option<String>> {
        Ok(self.merge_head.clone())
    }

    async fn target_conflict_paths(&self) -> EvidenceResult<Vec<String>> {
        Ok(self.conflict_paths.clone())
    }

    async fn target_is_clean(&self) -> EvidenceResult<bool> {
        Ok(self.clean)
    }

    async fn target_index_paths(&self, prefix: &str) -> EvidenceResult<Vec<String>> {
        Ok(self
            .index_paths
            .iter()
            .filter(|path| path.starts_with(prefix))
            .cloned()
            .collect())
    }

    async fn parents_of(&self, commit: &str) -> EvidenceResult<Vec<String>> {
        self.repo
            .parents
            .get(commit)
            .cloned()
            .ok_or_else(|| format!("unknown commit {}", commit))
    }

    async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> EvidenceResult<bool> {
        Ok(self
            .repo
            .ancestors(descendant)
            .iter()
            .any(|c| c == ancestor))
    }

    async fn first_parent_lineage(&self, tip: &str) -> EvidenceResult<Vec<String>> {
        Ok(self.repo.first_parent_lineage(tip))
    }

    async fn merge_base(&self, a: &str, b: &str) -> EvidenceResult<Option<String>> {
        let b_ancestors = self.repo.ancestors(b);
        Ok(self
            .repo
            .ancestors(a)
            .into_iter()
            .find(|c| b_ancestors.contains(c)))
    }

    async fn commits_with_exact_subject(
        &self,
        from: Option<&str>,
        to: &str,
        subject: &str,
    ) -> EvidenceResult<Vec<String>> {
        let excluded = from.map(|f| self.repo.ancestors(f)).unwrap_or_default();
        Ok(self
            .repo
            .ancestors(to)
            .into_iter()
            .filter(|c| !excluded.contains(c))
            .filter(|c| self.repo.subjects.get(c).map(String::as_str) == Some(subject))
            .collect())
    }

    async fn commit_diff_entries(&self, commit: &str) -> EvidenceResult<Vec<CommitDiffEntry>> {
        Ok(self.repo.diffs.get(commit).cloned().unwrap_or_default())
    }

    async fn committed_tree_paths(
        &self,
        revision: &str,
        prefix: &str,
    ) -> EvidenceResult<Vec<String>> {
        Ok(self
            .repo
            .trees
            .get(revision)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|path| path.starts_with(prefix))
            .collect())
    }

    async fn committed_file_text(
        &self,
        revision: &str,
        path: &str,
    ) -> EvidenceResult<Option<String>> {
        if self
            .unreadable
            .iter()
            .any(|(rev, file)| rev == revision && file == path)
        {
            return Err(format!("fatal: unable to read {}:{}", revision, path));
        }
        Ok(self
            .repo
            .files
            .get(&(revision.to_string(), path.to_string()))
            .cloned())
    }
}

pub(crate) fn item(revision: &str, change_id: &str, path: &str) -> SequentialMergeItem {
    SequentialMergeItem {
        revision: revision.to_string(),
        change_id: change_id.to_string(),
        archive_path: PathBuf::from(path),
        branch_base: None,
    }
}

pub(crate) fn live(change_id: &str) -> String {
    format!("openspec/changes/{}/proposal.md", change_id)
}

pub(crate) fn archived(change_id: &str) -> String {
    format!(
        "openspec/changes/archive/2026-08-03-{}/proposal.md",
        change_id
    )
}

/// Repository-relative active task list of `change_id`.
pub(crate) fn live_tasks(change_id: &str) -> String {
    format!("openspec/changes/{}/tasks.md", change_id)
}

/// Repository-relative archived task list of `change_id`.
pub(crate) fn archived_tasks(change_id: &str) -> String {
    format!("openspec/changes/archive/2026-08-03-{}/tasks.md", change_id)
}

/// Repository-relative active JSON task artifact of `change_id`.
pub(crate) fn live_tasks_json(change_id: &str) -> String {
    format!("openspec/changes/{}/tasks.json", change_id)
}

/// Repository-relative archived JSON task artifact of `change_id`.
pub(crate) fn archived_tasks_json(change_id: &str) -> String {
    format!(
        "openspec/changes/archive/2026-08-03-{}/tasks.json",
        change_id
    )
}

/// A JSON task document recording `completed` of `total` tasks done.
pub(crate) fn tasks_json(completed: u32, total: u32) -> String {
    let tasks = (0..total)
        .map(|index| {
            let status = if index < completed {
                "completed"
            } else {
                "pending"
            };
            format!(
                r#"{{"id":"task-{index}","title":"Task {index}","status":"{status}","section":"implementation"}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"schema_version":1,"tasks":[{tasks}]}}"#)
}

/// A task list recording `completed` of `total` tasks done.
pub(crate) fn tasks_markdown(completed: u32, total: u32) -> String {
    let mut content = String::from("## Implementation Tasks\n\n");
    for index in 0..total {
        let box_ = if index < completed { "x" } else { " " };
        content.push_str(&format!("- [{}] Task {}\n", box_, index + 1));
    }
    content
}

/// Add a complete archived task list for `change_id` at `revision`.
pub(crate) fn with_complete_tasks(repo: &mut FakeRepo, revision: &str, change_id: &str) {
    repo.file(revision, &archived_tasks(change_id), &tasks_markdown(7, 7));
}

/// `base -> t` on the target, with branch `ws-a` pre-synced onto `t`.
///
/// `a_tip` is the branch tip; `merge` (when integrated) is the final merge.
/// The branch tip carries the archived change, which is what a resolve batch
/// item always looks like: the change is archived on its own branch before the
/// final merge integrates it — with its archived task list complete, which is
/// what makes it mergeable at all.
pub(crate) fn presynced_repo() -> FakeRepo {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a1", "Work on change-a", &["base"])
        .commit("a_tip", "Pre-sync base into change-a", &["a1", "t"])
        .tree("a_tip", &[&archived("change-a")])
        .tree("t", &[])
        .tree("base", &[]);
    with_complete_tasks(&mut repo, "a_tip", "change-a");
    repo
}
