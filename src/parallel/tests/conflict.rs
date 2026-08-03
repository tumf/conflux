//! Tests for conflict detection and resolution functionality.

use super::super::conflict::*;
use crate::vcs::{VcsBackend, VcsResult, VcsWarning, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// How the mock reports conflicts across successive `detect_conflicts` calls.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConflictProbe {
    /// Always report the configured conflicts.
    Static,
    /// Report conflicts on the initial detection, then report them cleared.
    ClearedAfterFirstCall,
    /// Fail detection, exercising the early-return path of resolve.
    AlwaysFails,
}

/// Mock WorkspaceManager for testing conflict detection.
struct MockWorkspaceManager {
    conflicts: Vec<String>,
    status_output: String,
    log_output: String,
    repo_root: PathBuf,
    probe: ConflictProbe,
    detect_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl MockWorkspaceManager {
    fn new(conflicts: Vec<String>) -> Self {
        Self {
            conflicts,
            status_output: "# On branch main\n# Unmerged paths:\n#   both modified:   src/main.rs"
                .to_string(),
            log_output: "commit abc123\nAuthor: Test\nDate: 2024-01-01\n\nTest commit".to_string(),
            repo_root: PathBuf::from("/tmp/test-repo"),
            probe: ConflictProbe::Static,
            detect_calls: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn with_repo_root(mut self, repo_root: PathBuf) -> Self {
        self.repo_root = repo_root;
        self
    }

    fn with_probe(mut self, probe: ConflictProbe) -> Self {
        self.probe = probe;
        self
    }

    fn with_status(mut self, status: String) -> Self {
        self.status_output = status;
        self
    }

    fn with_log(mut self, log: String) -> Self {
        self.log_output = log;
        self
    }
}

#[async_trait]
impl WorkspaceManager for MockWorkspaceManager {
    fn backend_type(&self) -> VcsBackend {
        VcsBackend::Git
    }

    async fn check_available(&self) -> VcsResult<bool> {
        Ok(true)
    }

    async fn prepare_for_parallel(&self) -> VcsResult<Option<VcsWarning>> {
        Ok(None)
    }

    async fn get_current_revision(&self) -> VcsResult<String> {
        Ok("rev".to_string())
    }

    async fn create_workspace(
        &mut self,
        _change_id: &str,
        _base_revision: Option<&str>,
    ) -> VcsResult<super::super::Workspace> {
        Ok(super::super::Workspace {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test"),
            change_id: "test".to_string(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Created,
        })
    }

    fn update_workspace_status(&mut self, _workspace_name: &str, _status: WorkspaceStatus) {}

    async fn merge_workspaces(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok("merged".to_string())
    }

    async fn cleanup_workspace(&mut self, _workspace_name: &str) -> VcsResult<()> {
        Ok(())
    }

    async fn cleanup_all(&mut self) -> VcsResult<()> {
        Ok(())
    }

    fn max_concurrent(&self) -> usize {
        4
    }

    async fn list_worktree_change_ids(&self) -> VcsResult<HashSet<String>> {
        Ok(HashSet::new())
    }

    async fn snapshot_working_copy(&self, _workspace_path: &Path) -> VcsResult<()> {
        Ok(())
    }

    async fn create_verified_commit(
        &self,
        _workspace_path: &Path,
        _message: &str,
    ) -> VcsResult<crate::vcs::VerifiedCommitOutcome> {
        Ok(crate::vcs::VerifiedCommitOutcome::Committed)
    }

    async fn create_iteration_snapshot(
        &self,
        _workspace_path: &Path,
        _change_id: &str,
        _iteration: u32,
        _completed: u32,
        _total: u32,
    ) -> VcsResult<()> {
        Ok(())
    }

    async fn squash_wip_commits(
        &self,
        _workspace_path: &Path,
        _change_id: &str,
        _final_iteration: u32,
    ) -> VcsResult<()> {
        Ok(())
    }

    async fn get_revision_in_workspace(&self, _workspace_path: &Path) -> VcsResult<String> {
        Ok("test-rev".to_string())
    }

    fn forget_workspace_sync(&self, _workspace_name: &str) {}

    async fn find_existing_workspace(
        &mut self,
        _change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        Ok(None)
    }

    async fn reuse_workspace(
        &mut self,
        _workspace_info: &WorkspaceInfo,
    ) -> VcsResult<super::super::Workspace> {
        Ok(super::super::Workspace {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/test"),
            change_id: "test".to_string(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Created,
        })
    }

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    fn workspaces(&self) -> Vec<super::super::Workspace> {
        vec![]
    }

    async fn ensure_original_branch_initialized(&self) -> VcsResult<String> {
        Ok("main".to_string())
    }

    fn original_branch(&self) -> Option<String> {
        Some("main".to_string())
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        let call = self
            .detect_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match self.probe {
            ConflictProbe::Static => Ok(self.conflicts.clone()),
            ConflictProbe::ClearedAfterFirstCall if call == 0 => Ok(self.conflicts.clone()),
            ConflictProbe::ClearedAfterFirstCall => Ok(Vec::new()),
            ConflictProbe::AlwaysFails => Err(crate::vcs::VcsError::Conflict {
                backend: VcsBackend::Git,
                details: "conflict detection unavailable".to_string(),
            }),
        }
    }

    async fn get_status(&self) -> VcsResult<String> {
        Ok(self.status_output.clone())
    }

    async fn get_log_for_revisions(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok(self.log_output.clone())
    }

    fn conflict_resolution_prompt(&self) -> &'static str {
        "Git conflict resolution:"
    }
}

#[tokio::test]
async fn test_detect_conflicts_no_conflicts() {
    let manager = MockWorkspaceManager::new(vec![]);
    let conflicts = detect_conflicts(&manager).await.unwrap();
    assert!(conflicts.is_empty());
}

#[tokio::test]
async fn test_detect_conflicts_with_conflicts() {
    let manager =
        MockWorkspaceManager::new(vec!["src/main.rs".to_string(), "src/lib.rs".to_string()]);
    let conflicts = detect_conflicts(&manager).await.unwrap();
    assert_eq!(conflicts.len(), 2);
    assert_eq!(conflicts[0], "src/main.rs");
    assert_eq!(conflicts[1], "src/lib.rs");
}

#[tokio::test]
async fn test_get_vcs_status() {
    let expected_status =
        "# On branch test\n# Changes not staged for commit:\n#   modified:   src/main.rs";
    let manager = MockWorkspaceManager::new(vec![]).with_status(expected_status.to_string());

    let status = get_vcs_status(&manager).await.unwrap();
    assert_eq!(status, expected_status);
}

#[tokio::test]
async fn test_get_vcs_log_for_revisions() {
    let expected_log = "commit def456\nAuthor: Developer\nDate: 2024-01-02\n\nUpdate feature";
    let manager = MockWorkspaceManager::new(vec![]).with_log(expected_log.to_string());

    let log = get_vcs_log_for_revisions(&manager, &["rev1".to_string(), "rev2".to_string()])
        .await
        .unwrap();
    assert_eq!(log, expected_log);
}

fn test_items() -> Vec<crate::parallel::resolve_state::SequentialMergeItem> {
    crate::parallel::resolve_state::SequentialMergeItem::batch(
        &["rev1".to_string()],
        &["change1".to_string()],
        &[PathBuf::from("/tmp/archive/change1")],
    )
    .expect("well-formed batch")
}

#[tokio::test]
async fn test_resolve_merges_with_retry_args_struct() {
    // Test that ResolveMergesWithRetryArgs can be constructed properly
    let manager = MockWorkspaceManager::new(vec![]);
    let config = crate::config::OrchestratorConfig::default();
    let items = test_items();
    let target_branch = "main";
    let base_revision = "base123";
    let max_retries = 3;

    let shared_stagger_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let auto_resolve_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let args = ResolveMergesWithRetryArgs {
        workspace_manager: &manager as &dyn WorkspaceManager,
        config: &config,
        event_tx: &None,
        items: &items,
        target_branch,
        base_revision,
        max_retries,
        shared_stagger_state,
        auto_resolve_count,
        publication_owns_completion: false,
    };

    // Verify fields are accessible
    assert_eq!(args.target_branch, "main");
    assert_eq!(args.base_revision, "base123");
    assert_eq!(args.max_retries, 3);
    assert_eq!(args.items.len(), 1);
    assert_eq!(args.items[0].change_id, "change1");
    assert_eq!(
        args.items[0].archive_path,
        PathBuf::from("/tmp/archive/change1")
    );
}

#[test]
fn test_resolve_merges_with_retry_args_clone() {
    // Test that ResolveMergesWithRetryArgs implements Clone
    let manager = MockWorkspaceManager::new(vec![]);
    let config = crate::config::OrchestratorConfig::default();
    let items = test_items();

    let shared_stagger_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let auto_resolve_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let args1 = ResolveMergesWithRetryArgs {
        workspace_manager: &manager as &dyn WorkspaceManager,
        config: &config,
        event_tx: &None,
        items: &items,
        target_branch: "main",
        base_revision: "base123",
        max_retries: 3,
        shared_stagger_state,
        auto_resolve_count,
        publication_owns_completion: false,
    };

    let args2 = args1.clone(); // Clone instead of Copy
    let _args3 = args1; // Can still use args1 because it's Clone

    assert_eq!(args2.target_branch, "main");
}

/// Capacity-recovery audit coverage for automatic conflict resolution.
///
/// Automatic resolve occupancy is accounted through an RAII guard, so the scheduler's
/// `calculate_available_slots` input must return to its pre-resolve value on every exit
/// path. If any path leaked the counter, queued work would stay permanently capacity
/// gated and would depend on a sticky re-analysis trigger to make progress.
async fn run_auto_resolve_and_report_counter(
    probe: ConflictProbe,
) -> (bool, usize, tempfile::TempDir) {
    let repo_dir = tempfile::TempDir::new().expect("create temp repo dir");
    let manager = MockWorkspaceManager::new(vec!["conflict.txt".to_string()])
        .with_repo_root(repo_dir.path().to_path_buf())
        .with_probe(probe);

    let config = crate::config::OrchestratorConfig {
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    };
    let auto_resolve_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let result = resolve_conflicts_with_retry(
        &manager as &dyn WorkspaceManager,
        &config,
        &None,
        &["rev1".to_string()],
        &["change-a".to_string()],
        "merge conflict",
        1,
        std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        auto_resolve_count.clone(),
    )
    .await;

    let remaining = auto_resolve_count.load(std::sync::atomic::Ordering::SeqCst);
    (result.is_ok(), remaining, repo_dir)
}

#[tokio::test]
async fn auto_resolve_releases_capacity_on_success() {
    let (succeeded, remaining, _repo_dir) =
        run_auto_resolve_and_report_counter(ConflictProbe::ClearedAfterFirstCall).await;

    assert!(succeeded, "cleared conflicts should complete resolution");
    assert_eq!(
        remaining, 0,
        "successful automatic resolve must release its scheduler slot"
    );
}

#[tokio::test]
async fn auto_resolve_releases_capacity_on_failure() {
    let (succeeded, remaining, _repo_dir) =
        run_auto_resolve_and_report_counter(ConflictProbe::Static).await;

    assert!(
        !succeeded,
        "unresolved conflicts should exhaust retries and fail"
    );
    assert_eq!(
        remaining, 0,
        "failed automatic resolve must release its scheduler slot"
    );
}

#[tokio::test]
async fn auto_resolve_releases_capacity_on_early_return() {
    let (succeeded, remaining, _repo_dir) =
        run_auto_resolve_and_report_counter(ConflictProbe::AlwaysFails).await;

    assert!(
        !succeeded,
        "conflict detection failure should return early with an error"
    );
    assert_eq!(
        remaining, 0,
        "early-return automatic resolve must release its scheduler slot"
    );
}

// ---------------------------------------------------------------------------
// Sequential resolve batch classification
// ---------------------------------------------------------------------------

use crate::parallel::resolve_state::{
    self, BatchState, EvidenceResult, ResolveEvidence, SequentialMergeItem,
};
use crate::vcs::git::commands::{CommitDiffEntry, WorktreeIdentity};
use std::collections::HashMap;

/// Minimal in-memory commit graph.
///
/// Real ancestry, first-parent lineage, and subject-range queries are computed
/// from it, so the classifier's decision table is exercised without a process
/// or filesystem boundary.
#[derive(Default, Clone)]
struct FakeRepo {
    parents: HashMap<String, Vec<String>>,
    subjects: HashMap<String, String>,
    trees: HashMap<String, Vec<String>>,
    diffs: HashMap<String, Vec<CommitDiffEntry>>,
}

impl FakeRepo {
    fn commit(&mut self, id: &str, subject: &str, parents: &[&str]) -> &mut Self {
        self.parents.insert(
            id.to_string(),
            parents.iter().map(|p| p.to_string()).collect(),
        );
        self.subjects.insert(id.to_string(), subject.to_string());
        self
    }

    fn tree(&mut self, id: &str, paths: &[&str]) -> &mut Self {
        self.trees.insert(
            id.to_string(),
            paths.iter().map(|p| p.to_string()).collect(),
        );
        self
    }

    fn diff(&mut self, id: &str, entries: &[(char, &str)]) -> &mut Self {
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
    fn ancestors(&self, id: &str) -> Vec<String> {
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

    fn first_parent_lineage(&self, tip: &str) -> Vec<String> {
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

struct FakeEvidence {
    repo: FakeRepo,
    identities: HashMap<String, WorktreeIdentity>,
    worktree_merges: Vec<PathBuf>,
    worktree_conflicts: HashMap<PathBuf, Vec<String>>,
    head: String,
    merge_head: Option<String>,
    conflict_paths: Vec<String>,
    clean: bool,
    index_paths: Vec<String>,
}

impl FakeEvidence {
    fn new(repo: FakeRepo, head: &str) -> Self {
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
        }
    }

    fn worktree(mut self, branch: &str, path: &str, tip: &str) -> Self {
        self.identities.insert(
            branch.to_string(),
            WorktreeIdentity::Supplied {
                path: PathBuf::from(path),
                tip: tip.to_string(),
            },
        );
        self
    }

    fn unsafe_worktree(mut self, branch: &str, reason: &str) -> Self {
        self.identities.insert(
            branch.to_string(),
            WorktreeIdentity::Unsafe {
                reason: reason.to_string(),
            },
        );
        self
    }

    fn merge_head(mut self, tip: &str) -> Self {
        self.merge_head = Some(tip.to_string());
        self
    }

    fn index(mut self, paths: &[&str]) -> Self {
        self.index_paths = paths.iter().map(|p| p.to_string()).collect();
        self
    }

    fn conflicts(mut self, paths: &[&str]) -> Self {
        self.conflict_paths = paths.iter().map(|p| p.to_string()).collect();
        self
    }

    fn dirty(mut self) -> Self {
        self.clean = false;
        self
    }

    fn worktree_merging(mut self, path: &str) -> Self {
        self.worktree_merges.push(PathBuf::from(path));
        self
    }

    fn worktree_conflicted(mut self, path: &str, files: &[&str]) -> Self {
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
}

fn item(revision: &str, change_id: &str, path: &str) -> SequentialMergeItem {
    SequentialMergeItem {
        revision: revision.to_string(),
        change_id: change_id.to_string(),
        archive_path: PathBuf::from(path),
        branch_base: None,
    }
}

fn live(change_id: &str) -> String {
    format!("openspec/changes/{}/proposal.md", change_id)
}

fn archived(change_id: &str) -> String {
    format!(
        "openspec/changes/archive/2026-08-03-{}/proposal.md",
        change_id
    )
}

/// `base -> t` on the target, with branch `ws-a` pre-synced onto `t`.
///
/// `a_tip` is the branch tip; `merge` (when integrated) is the final merge.
/// The branch tip carries the archived change, which is what a resolve batch
/// item always looks like: the change is archived on its own branch before the
/// final merge integrates it.
fn presynced_repo() -> FakeRepo {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a1", "Work on change-a", &["base"])
        .commit("a_tip", "Pre-sync base into change-a", &["a1", "t"])
        .tree("a_tip", &[&archived("change-a")])
        .tree("t", &[])
        .tree("base", &[]);
    repo
}

#[tokio::test]
async fn absent_manager_entry_still_reports_the_supplied_worktree_path() {
    // The mock workspace manager returns an empty workspace list, exactly like
    // the reported failure. The supplied path must still reach the prompt.
    let repo = presynced_repo();
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/change-a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/change-a")];

    let rendered = super::super::conflict::render_worktree_locations(&evidence, &items).await;

    assert!(
        rendered.contains("/wt/change-a"),
        "supplied worktree path must be reported, got {}",
        rendered
    );
    assert!(
        !rendered.contains("(unknown)"),
        "process-local workspace absence must not degrade to '(unknown)', got {}",
        rendered
    );
}

#[tokio::test]
async fn unsafe_worktree_identity_blocks_every_action() {
    let evidence =
        FakeEvidence::new(presynced_repo(), "t").unsafe_worktree("ws-a", "detached HEAD");
    let items = vec![item("ws-a", "change-a", "/wt/change-a")];

    let state = resolve_state::classify_batch(&evidence, &items, "base").await;

    assert!(
        matches!(state, BatchState::UnsafeEvidence { .. }),
        "{:?}",
        state
    );
    assert!(!state.allows_agent_action());
    let rendered = super::super::conflict::render_worktree_locations(&evidence, &items).await;
    assert!(
        rendered.contains("unvalidated: detached HEAD"),
        "{}",
        rendered
    );
}

#[tokio::test]
async fn empty_batch_is_unsafe() {
    let evidence = FakeEvidence::new(presynced_repo(), "t");
    let state = resolve_state::classify_batch(&evidence, &[], "base").await;
    assert!(
        matches!(state, BatchState::UnsafeEvidence { .. }),
        "{:?}",
        state
    );
}

#[test]
fn malformed_batches_are_rejected_before_resolve() {
    assert!(SequentialMergeItem::batch(&[], &[], &[]).is_err());
    assert!(SequentialMergeItem::batch(
        &["rev1".to_string(), "rev2".to_string()],
        &["change1".to_string()],
        &[PathBuf::from("/a"), PathBuf::from("/b")],
    )
    .is_err());
    assert!(
        SequentialMergeItem::batch(&["rev1".to_string()], &["change1".to_string()], &[],).is_err()
    );
    assert!(SequentialMergeItem::batch(
        &["rev1".to_string()],
        &["".to_string()],
        &[PathBuf::from("/a")],
    )
    .is_err());

    let ordered = SequentialMergeItem::batch(
        &["rev1".to_string(), "rev2".to_string()],
        &["change1".to_string(), "change2".to_string()],
        &[PathBuf::from("/a"), PathBuf::from("/b")],
    )
    .expect("well-formed batch");
    assert_eq!(
        SequentialMergeItem::change_ids(&ordered),
        vec!["change1".to_string(), "change2".to_string()],
        "declared order must survive batch construction"
    );
}

#[tokio::test]
async fn target_state_on_first_parent_lineage_needs_no_presync() {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a_tip", "Work on change-a", &["base"])
        .tree("a_tip", &[&archived("change-a")])
        .tree("base", &[]);
    let evidence = FakeEvidence::new(repo, "base").worktree("ws-a", "/wt/a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    let state = resolve_state::classify_batch(&evidence, &items, "base").await;

    assert!(
        matches!(state, BatchState::FinalMergeMissing { .. }),
        "target HEAD is already on the tip lineage, so classification proceeds to the final merge: {:?}",
        state
    );
}

#[tokio::test]
async fn valid_presync_merge_reaches_final_merge_missing() {
    let evidence = FakeEvidence::new(presynced_repo(), "t").worktree("ws-a", "/wt/a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    let state = resolve_state::classify_batch(&evidence, &items, "base").await;

    match state {
        BatchState::FinalMergeMissing {
            change_id,
            required_target_state,
            ..
        } => {
            assert_eq!(change_id, "change-a");
            assert_eq!(required_target_state, "t");
        }
        other => panic!("expected final merge missing, got {:?}", other),
    }
}

#[tokio::test]
async fn missing_duplicate_and_wrong_parent_presync_all_fail_closed() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // Missing: branch never merged the advanced target.
    let mut missing = FakeRepo::default();
    missing
        .commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a_tip", "Work on change-a", &["base"]);
    let state = resolve_state::classify_batch(
        &FakeEvidence::new(missing, "t").worktree("ws-a", "/wt/a", "a_tip"),
        &items,
        "base",
    )
    .await;
    assert!(
        matches!(state, BatchState::PreSyncInvalid { .. }),
        "{:?}",
        state
    );

    // Duplicate exact pre-sync subjects.
    let mut duplicated = FakeRepo::default();
    duplicated
        .commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a1", "Work", &["base"])
        .commit("p1", "Pre-sync base into change-a", &["a1", "t"])
        .commit("a_tip", "Pre-sync base into change-a", &["p1", "t"]);
    let state = resolve_state::classify_batch(
        &FakeEvidence::new(duplicated, "t").worktree("ws-a", "/wt/a", "a_tip"),
        &items,
        "base",
    )
    .await;
    assert!(
        matches!(state, BatchState::PreSyncInvalid { .. }),
        "{:?}",
        state
    );

    // Wrong non-first parent: pre-synced an older target state.
    let mut wrong = FakeRepo::default();
    wrong
        .commit("base", "Base", &[])
        .commit("t_old", "Older target", &["base"])
        .commit("t", "Target advance", &["t_old"])
        .commit("a1", "Work", &["base"])
        .commit("a_tip", "Pre-sync base into change-a", &["a1", "t_old"]);
    let state = resolve_state::classify_batch(
        &FakeEvidence::new(wrong, "t").worktree("ws-a", "/wt/a", "a_tip"),
        &items,
        "base",
    )
    .await;
    match state {
        BatchState::PreSyncInvalid { reason, .. } => {
            assert!(reason.contains("non-first parent"), "{}", reason)
        }
        other => panic!("expected invalid pre-sync, got {:?}", other),
    }

    // Wrong parent count: a non-merge commit carrying the pre-sync subject.
    let mut single_parent = FakeRepo::default();
    single_parent
        .commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a1", "Work", &["base"])
        .commit("a_tip", "Pre-sync base into change-a", &["a1"]);
    let state = resolve_state::classify_batch(
        &FakeEvidence::new(single_parent, "t").worktree("ws-a", "/wt/a", "a_tip"),
        &items,
        "base",
    )
    .await;
    match state {
        BatchState::PreSyncInvalid { reason, .. } => {
            assert!(reason.contains("parent(s)"), "{}", reason)
        }
        other => panic!("expected invalid pre-sync, got {:?}", other),
    }
}

#[tokio::test]
async fn unfinished_worktree_merge_and_conflicts_report_presync_unfinished() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    let evidence = FakeEvidence::new(presynced_repo(), "t")
        .worktree("ws-a", "/wt/a", "a_tip")
        .worktree_merging("/wt/a");
    assert!(
        matches!(
            resolve_state::classify_batch(&evidence, &items, "base").await,
            BatchState::PreSyncUnfinished { .. }
        ),
        "an unfinished worktree merge must be reported before any target guidance"
    );

    let evidence = FakeEvidence::new(presynced_repo(), "t")
        .worktree("ws-a", "/wt/a", "a_tip")
        .worktree_conflicted("/wt/a", &["src/lib.rs"]);
    assert!(matches!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::PreSyncUnfinished { .. }
    ));
}

#[tokio::test]
async fn historical_ancestry_integration_is_exempt_from_presync_reconstruction() {
    // No exact final-subject candidate exists and the tip is already ancestral:
    // pre-sync topology cannot be reconstructed and is not required.
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a_tip", "Work on change-a", &["base"])
        .commit("head", "Unrelated follow-up", &["a_tip"])
        .tree("head", &[]);
    let evidence = FakeEvidence::new(repo, "head").worktree("ws-a", "/wt/a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete
    );
}

#[tokio::test]
async fn exact_final_merge_topology_is_required() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // Valid: two parents, first parent `t`, non-first parent the branch tip.
    let mut valid = presynced_repo();
    valid
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .tree("merge", &[]);
    let evidence = FakeEvidence::new(valid, "merge").worktree("ws-a", "/wt/a", "a_tip");
    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete
    );

    // The expected revision is only on the first-parent side.
    let mut false_exact = presynced_repo();
    false_exact
        .commit("other", "Unrelated branch", &["base"])
        .commit("merge", "Merge change: change-a", &["a_tip", "other"])
        .tree("merge", &[]);
    let evidence = FakeEvidence::new(false_exact, "merge").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("non-first parent"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Wrong parent count.
    let mut wrong_count = presynced_repo();
    wrong_count
        .commit("merge", "Merge change: change-a", &["t"])
        .tree("merge", &[]);
    let evidence = FakeEvidence::new(wrong_count, "merge").worktree("ws-a", "/wt/a", "a_tip");
    assert!(matches!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::UnsafeEvidence { .. }
    ));

    // Duplicate exact candidates.
    let mut duplicated = presynced_repo();
    duplicated
        .commit("merge1", "Merge change: change-a", &["t", "a_tip"])
        .commit("merge2", "Merge change: change-a", &["merge1", "a_tip"])
        .tree("merge2", &[]);
    let evidence = FakeEvidence::new(duplicated, "merge2").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("exactly one is required"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }
}

#[tokio::test]
async fn later_item_owns_target_merge_after_prior_completion() {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .commit("merge_a", "Merge change: change-a", &["base", "a1"])
        .commit("b_tip", "Pre-sync base into change-b", &["b1", "merge_a"])
        .tree("merge_a", &[]);
    let evidence = FakeEvidence::new(repo, "merge_a")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b_tip")
        .merge_head("b_tip")
        .index(&[&live("change-b"), &archived("change-b")]);
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];

    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::TargetMergeUnfinished {
            change_id,
            requires_live_removal,
            ..
        } => {
            assert_eq!(change_id, "change-b");
            assert!(
                requires_live_removal,
                "stage-0 index coexistence must predict resurrection before the final commit"
            );
        }
        other => panic!(
            "expected target merge unfinished for change-b, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn conflict_stages_withhold_cleanup_guidance() {
    let mut repo = presynced_repo();
    repo.tree("t", &[]);
    let evidence = FakeEvidence::new(repo, "t")
        .worktree("ws-a", "/wt/a", "a_tip")
        .merge_head("a_tip")
        .conflicts(&["openspec/changes/change-a/proposal.md"])
        .index(&[&live("change-a"), &archived("change-a")]);
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::TargetMergeUnfinished {
            requires_live_removal,
            reason,
            ..
        } => {
            assert!(
                !requires_live_removal,
                "cleanup must not be authorized while conflict stages exist"
            );
            assert!(reason.contains("conflict stages"), "{}", reason);
        }
        other => panic!("expected target merge unfinished, got {:?}", other),
    }
}

#[tokio::test]
async fn target_merge_owner_must_be_unique_and_first_incomplete() {
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];

    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .tree("base", &[]);

    // Unknown owner: MERGE_HEAD matches no batch tip.
    let evidence = FakeEvidence::new(repo.clone(), "base")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b1")
        .merge_head("stranger");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("does not match"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Ambiguous owner: two items report the same tip.
    let evidence = FakeEvidence::new(repo.clone(), "base")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "a1")
        .merge_head("a1");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("matches 2"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Owner is not the first incomplete item.
    let evidence = FakeEvidence::new(repo.clone(), "base")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b1")
        .merge_head("b1");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("first incomplete"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }
}

#[tokio::test]
async fn out_of_order_integration_fails_closed() {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .commit("merge_b", "Merge change: change-b", &["base", "b1"])
        .tree("merge_b", &[]);
    let evidence = FakeEvidence::new(repo, "merge_b")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b1");
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];

    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("declared order"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }
}

#[tokio::test]
async fn multi_change_clean_completion() {
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .commit("merge_a", "Merge change: change-a", &["base", "a1"])
        .commit("b_tip", "Pre-sync base into change-b", &["b1", "merge_a"])
        .commit("merge_b", "Merge change: change-b", &["merge_a", "b_tip"])
        .tree("merge_b", &[&archived("change-a"), &archived("change-b")]);
    let evidence = FakeEvidence::new(repo, "merge_b")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b_tip");
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];

    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete
    );
}

#[tokio::test]
async fn committed_live_and_archive_coexistence_requires_forward_cleanup() {
    let mut repo = presynced_repo();
    repo.commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .tree("merge", &[&live("change-a"), &archived("change-a")]);
    let evidence = FakeEvidence::new(repo, "merge").worktree("ws-a", "/wt/a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    let state = resolve_state::classify_batch(&evidence, &items, "base").await;
    match &state {
        BatchState::ResurrectionCleanupRequired { change_id, .. } => {
            assert_eq!(change_id, "change-a")
        }
        other => panic!("expected resurrection cleanup, got {:?}", other),
    }
    assert!(
        state
            .diagnosis()
            .contains("Cleanup resurrected change: change-a"),
        "guidance must name the exact forward cleanup subject: {}",
        state.diagnosis()
    );
}

#[tokio::test]
async fn staged_only_cleanup_remains_incomplete() {
    // The live directory is removed from the index and worktree but the commit
    // still contains it: committed evidence, not staged intent, decides.
    let mut repo = presynced_repo();
    repo.commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .tree("merge", &[&live("change-a"), &archived("change-a")]);
    let evidence = FakeEvidence::new(repo, "merge")
        .worktree("ws-a", "/wt/a", "a_tip")
        .dirty();
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    assert!(matches!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::ResurrectionCleanupRequired { .. }
    ));
}

#[tokio::test]
async fn valid_forward_cleanup_commit_completes_the_batch() {
    let mut repo = presynced_repo();
    repo.commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["merge"],
        )
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("merge", &[&live("change-a"), &archived("change-a")])
        .tree("cleanup", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(repo, "cleanup").worktree("ws-a", "/wt/a", "a_tip");
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete
    );
}

#[tokio::test]
async fn invalid_cleanup_commits_fail_closed() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // Touches archive content as well as the live subtree.
    let mut unrelated = presynced_repo();
    unrelated
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["merge"],
        )
        .diff(
            "cleanup",
            &[('D', &live("change-a")), ('M', &archived("change-a"))],
        )
        .tree("cleanup", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(unrelated, "cleanup").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("may only delete"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Merge-shaped (two-parent) cleanup commit.
    let mut merged_cleanup = presynced_repo();
    merged_cleanup
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["merge", "a_tip"],
        )
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("cleanup", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(merged_cleanup, "cleanup").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("exactly one"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }
}

#[tokio::test]
async fn cleanup_must_be_a_forward_commit_on_the_target_lineage() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // Reachable but not on the target's own forward history: the cleanup was
    // authored on a side branch and merged back, so the target never moved
    // forward past the resurrection.
    let mut side_branch = presynced_repo();
    side_branch
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["merge"],
        )
        .commit("head", "Merge side cleanup", &["merge", "cleanup"])
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("merge", &[&live("change-a"), &archived("change-a")])
        .tree("cleanup", &[&archived("change-a")])
        .tree("head", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(side_branch, "head").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("first-parent lineage"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Wrong predecessor: the cleanup is on the target lineage but its parent
    // predates the commit that integrated the change, so it cannot be the
    // cleanup of that integration.
    let mut before_integration = FakeRepo::default();
    before_integration
        .commit("base", "Base", &[])
        .commit("t", "Target advance", &["base"])
        .commit("a_tip", "Work on change-a", &["base"])
        .commit("cleanup", "Cleanup resurrected change: change-a", &["t"])
        .commit("head", "Merge branch ws-a", &["cleanup", "a_tip"])
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("a_tip", &[&archived("change-a")])
        .tree("cleanup", &[&archived("change-a")])
        .tree("head", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(before_integration, "head").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => assert!(
            reason.contains("does not contain its committed integration"),
            "{}",
            reason
        ),
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Wrong predecessor: on the lineage and after integration, but the parent
    // state never held the live/archive coexistence the commit claims to remove.
    let mut wrong_predecessor = presynced_repo();
    wrong_predecessor
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit("filler", "Unrelated follow-up", &["merge"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["filler"],
        )
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("merge", &[&live("change-a"), &archived("change-a")])
        .tree("filler", &[&archived("change-a")])
        .tree("cleanup", &[&archived("change-a")]);
    let evidence =
        FakeEvidence::new(wrong_predecessor, "cleanup").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => assert!(
            reason.contains("does not hold the live/archive coexistence"),
            "{}",
            reason
        ),
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // The same shape, cleaned up directly on top of the integration it repairs.
    let mut forward = presynced_repo();
    forward
        .commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .commit(
            "cleanup",
            "Cleanup resurrected change: change-a",
            &["merge"],
        )
        .diff("cleanup", &[('D', &live("change-a"))])
        .tree("merge", &[&live("change-a"), &archived("change-a")])
        .tree("cleanup", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(forward, "cleanup").worktree("ws-a", "/wt/a", "a_tip");
    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete,
        "a direct forward cleanup on the target lineage is the accepted shape"
    );
}

#[tokio::test]
async fn declared_order_is_verified_when_every_item_is_integrated() {
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];

    // Both items have an exact integration commit, so no `NotIntegrated` hole
    // exists — but the target history integrated B before A.
    let mut reversed = FakeRepo::default();
    reversed
        .commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .commit("merge_b", "Merge change: change-b", &["base", "b1"])
        .commit("merge_a", "Merge change: change-a", &["merge_b", "a1"])
        .tree("merge_a", &[&archived("change-a"), &archived("change-b")]);
    let evidence = FakeEvidence::new(reversed, "merge_a")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b1");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("declared order"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // The same two integrations in declared order are complete.
    let mut ordered = FakeRepo::default();
    ordered
        .commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("b1", "Work b", &["base"])
        .commit("merge_a", "Merge change: change-a", &["base", "a1"])
        .commit("b_tip", "Pre-sync base into change-b", &["b1", "merge_a"])
        .commit("merge_b", "Merge change: change-b", &["merge_a", "b_tip"])
        .tree("merge_b", &[&archived("change-a"), &archived("change-b")]);
    let evidence = FakeEvidence::new(ordered, "merge_b")
        .worktree("ws-a", "/wt/a", "a1")
        .worktree("ws-b", "/wt/b", "b_tip");
    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, "base").await,
        BatchState::Complete
    );
}

#[tokio::test]
async fn prefinal_archive_evidence_comes_from_the_validated_worktree_head() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // Worktree-only archive evidence: the target has never seen the archive, so
    // only the validated branch tip can prove it. The final merge is authorized.
    let mut worktree_only = presynced_repo();
    worktree_only
        .tree("t", &[&live("change-a")])
        .tree("a_tip", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(worktree_only, "t").worktree("ws-a", "/wt/a", "a_tip");
    assert!(
        matches!(
            resolve_state::classify_batch(&evidence, &items, "base").await,
            BatchState::FinalMergeMissing { .. }
        ),
        "an archive committed only on the branch tip must still authorize the final merge"
    );

    // Missing archive identity: the branch tip never archived the change.
    let mut unarchived = presynced_repo();
    unarchived.tree("a_tip", &[&live("change-a")]);
    let evidence = FakeEvidence::new(unarchived, "t").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("no valid archive proposal"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Nested layout: reported distinctly because it is repairable.
    let mut nested = presynced_repo();
    nested.tree(
        "a_tip",
        &["openspec/changes/archive/2026-08-03/change-a/proposal.md"],
    );
    let evidence = FakeEvidence::new(nested, "t").worktree("ws-a", "/wt/a", "a_tip");
    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("invalid nested layout"), "{}", reason)
        }
        other => panic!("expected unsafe evidence, got {:?}", other),
    }

    // Suffix collision and a non-proposal entry are not archive identities.
    for archive_like in [
        "openspec/changes/archive/prefix-change-a/proposal.md",
        "openspec/changes/archive/change-a/design.md",
    ] {
        let mut repo = presynced_repo();
        repo.tree("a_tip", &[archive_like]);
        let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");
        match resolve_state::classify_batch(&evidence, &items, "base").await {
            BatchState::UnsafeEvidence { reason } => assert!(
                reason.contains("no valid archive proposal"),
                "'{}' must not authorize the final merge: {}",
                archive_like,
                reason
            ),
            other => panic!(
                "'{}' must not authorize the final merge, got {:?}",
                archive_like, other
            ),
        }
    }
}

#[tokio::test]
async fn invalid_archive_shapes_never_authorize_deletion() {
    // Post-final: nested date layout, a suffix-similar entry, and a non-proposal
    // entry are all invalid archive identities, so committed live/archive
    // coexistence is never claimed and deletion is never authorized.
    for archive_like in [
        "openspec/changes/archive/2026-08-03/change-a/proposal.md",
        "openspec/changes/archive/prefix-change-a/proposal.md",
        "openspec/changes/archive/change-a/design.md",
    ] {
        let mut repo = presynced_repo();
        repo.commit("merge", "Merge change: change-a", &["t", "a_tip"])
            .tree("merge", &[&live("change-a"), archive_like]);
        let evidence = FakeEvidence::new(repo, "merge").worktree("ws-a", "/wt/a", "a_tip");
        let items = vec![item("ws-a", "change-a", "/wt/a")];

        let state = resolve_state::classify_batch(&evidence, &items, "base").await;
        assert!(
            !matches!(state, BatchState::ResurrectionCleanupRequired { .. }),
            "'{}' must not be treated as a valid archive identity, got {:?}",
            archive_like,
            state
        );
    }
}

#[tokio::test]
async fn dirty_target_is_never_complete() {
    let mut repo = presynced_repo();
    repo.commit("merge", "Merge change: change-a", &["t", "a_tip"])
        .tree("merge", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(repo, "merge")
        .worktree("ws-a", "/wt/a", "a_tip")
        .dirty();
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    match resolve_state::classify_batch(&evidence, &items, "base").await {
        BatchState::UnsafeEvidence { reason } => {
            assert!(reason.contains("not clean"), "{}", reason)
        }
        other => panic!("expected incomplete batch, got {:?}", other),
    }
}

#[tokio::test]
async fn final_integration_verifier_matches_retry_classification() {
    let items = vec![item("ws-a", "change-a", "/wt/a")];

    // No candidate and not ancestral: verification must report the change missing.
    let mut missing = presynced_repo();
    missing.tree("t", &[]);
    let evidence = FakeEvidence::new(missing, "t").worktree("ws-a", "/wt/a", "a_tip");
    let error = resolve_state::verify_final_integration(&evidence, &items, "base")
        .await
        .expect_err("unintegrated change must fail verification");
    assert!(error.contains("change-a"), "{}", error);

    // No candidate but already ancestral: historical success.
    let mut historical = FakeRepo::default();
    historical
        .commit("base", "Base", &[])
        .commit("a_tip", "Work", &["base"])
        .commit("head", "Follow-up", &["a_tip"]);
    let evidence = FakeEvidence::new(historical, "head").worktree("ws-a", "/wt/a", "a_tip");
    assert!(
        resolve_state::verify_final_integration(&evidence, &items, "base")
            .await
            .is_ok()
    );

    // An exact candidate with the wrong second parent must fail closed rather
    // than fall back to ancestry.
    let mut false_exact = presynced_repo();
    false_exact.commit("other", "Unrelated", &["base"]).commit(
        "merge",
        "Merge change: change-a",
        &["a_tip", "other"],
    );
    let evidence = FakeEvidence::new(false_exact, "merge").worktree("ws-a", "/wt/a", "a_tip");
    let error = resolve_state::verify_final_integration(&evidence, &items, "base")
        .await
        .expect_err("ancestry fallback must not rescue an invalid exact candidate");
    assert!(error.contains("non-first parent"), "{}", error);
}

// ---------------------------------------------------------------------------
// Full reported regression against a real repository
// ---------------------------------------------------------------------------

use crate::parallel::resolve_state::GitResolveEvidence;

async fn git(dir: &Path, args: &[&str]) -> String {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .unwrap_or_else(|e| panic!("git {:?}: {}", args, e));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

async fn write_and_commit(dir: &Path, path: &str, contents: &str, subject: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, contents).unwrap();
    git(dir, &["add", "-A"]).await;
    git(dir, &["commit", "-m", subject]).await;
}

/// The reported failure end to end, against a real Git repository.
///
/// The archive worktree path is supplied by merge admission and is deliberately
/// never registered with a workspace manager, reproducing the process-local
/// list that omitted it. Each phase is driven forward exactly as the resolve
/// agent would, and classification must track it without a generic
/// "missing merge commits" fallback.
#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn sequential_resolve_tracks_the_full_reported_regression() {
    let repo = tempfile::TempDir::new().expect("repo tempdir");
    let root = repo.path();
    git(root, &["init", "-b", "main"]).await;
    git(root, &["config", "user.email", "test@example.com"]).await;
    git(root, &["config", "user.name", "Test User"]).await;
    git(root, &["config", "commit.gpgsign", "false"]).await;
    write_and_commit(root, "README.md", "base\n", "Base").await;
    let base_revision = git(root, &["rev-parse", "HEAD"]).await;

    // The archived change worktree: the change is archived on its branch.
    let worktree = repo.path().parent().unwrap().join(format!(
        "{}-ws",
        root.file_name().unwrap().to_string_lossy()
    ));
    git(
        root,
        &[
            "worktree",
            "add",
            "-b",
            "ws-change-a",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
    )
    .await;
    write_and_commit(
        &worktree,
        "openspec/changes/archive/2026-08-03-change-a/proposal.md",
        "archived\n",
        "Archive change-a",
    )
    .await;

    // The target branch advances independently, so pre-sync becomes required.
    write_and_commit(root, "main.txt", "advanced\n", "Target advance").await;
    // The live change directory is present on the target and must not survive.
    write_and_commit(
        root,
        "openspec/changes/change-a/proposal.md",
        "live\n",
        "Add live change-a",
    )
    .await;

    let items = vec![item("ws-change-a", "change-a", worktree.to_str().unwrap())];
    let evidence = GitResolveEvidence::new(root);

    // Phase 1: the supplied path is validated and pre-sync is required.
    let state = resolve_state::classify_batch(&evidence, &items, &base_revision).await;
    match &state {
        BatchState::PreSyncInvalid {
            worktree: reported,
            required_target_state,
            ..
        } => {
            assert_eq!(
                reported.canonicalize().unwrap(),
                worktree.canonicalize().unwrap(),
                "the supplied archive worktree path must be validated and reported"
            );
            assert_eq!(
                required_target_state,
                &git(root, &["rev-parse", "HEAD"]).await
            );
        }
        other => panic!("expected pre-sync to be required, got {:?}", other),
    }
    assert!(
        !state.diagnosis().contains("Missing merge commits"),
        "the generic fallback must be gone: {}",
        state.diagnosis()
    );

    // Phase 2: the agent pre-syncs in the validated worktree.
    git(
        &worktree,
        &[
            "merge",
            "--no-ff",
            "-m",
            "Pre-sync base into change-a",
            "main",
        ],
    )
    .await;
    let state = resolve_state::classify_batch(&evidence, &items, &base_revision).await;
    assert!(
        matches!(state, BatchState::FinalMergeMissing { .. }),
        "valid pre-sync must unblock the final merge, got {:?}",
        state
    );

    // Phase 3: the final merge is started but not committed, and it resurrects
    // the live change directory alongside the valid archive.
    let merge = tokio::process::Command::new("git")
        .args(["merge", "--no-ff", "--no-commit", "ws-change-a"])
        .current_dir(root)
        .output()
        .await
        .expect("git merge");
    assert!(
        merge.status.success() || merge.status.code() == Some(1),
        "merge should either succeed or stop for commit"
    );
    let head_before_classification = git(root, &["rev-parse", "HEAD"]).await;
    let state = resolve_state::classify_batch(&evidence, &items, &base_revision).await;
    match &state {
        BatchState::TargetMergeUnfinished {
            change_id,
            requires_live_removal,
            ..
        } => {
            assert_eq!(change_id, "change-a");
            assert!(
                requires_live_removal,
                "stage-0 index coexistence must be reported before the final commit"
            );
        }
        other => panic!("expected an unfinished target merge, got {:?}", other),
    }
    // The old conflict-free shortcut committed here. Classification is
    // side-effect free and hands the work back to the agent instead.
    assert_eq!(
        git(root, &["rev-parse", "HEAD"]).await,
        head_before_classification,
        "a conflict-free MERGE_HEAD must not be committed by classification"
    );
    assert!(
        state.diagnosis().contains("Merge change: change-a")
            && !state.diagnosis().contains("Merge changes:"),
        "guidance must name the exact per-change subject, never a combined one: {}",
        state.diagnosis()
    );

    // Phase 4: committing without removing the live directory is not success.
    git(root, &["commit", "-m", "Merge change: change-a"]).await;
    assert!(
        matches!(
            resolve_state::classify_batch(&evidence, &items, &base_revision).await,
            BatchState::ResurrectionCleanupRequired { .. }
        ),
        "committed live/archive coexistence must require forward cleanup"
    );

    // Phase 5: staged-only removal is still not cleanup.
    git(root, &["rm", "-r", "-f", "openspec/changes/change-a"]).await;
    assert!(
        matches!(
            resolve_state::classify_batch(&evidence, &items, &base_revision).await,
            BatchState::ResurrectionCleanupRequired { .. }
        ),
        "staged-only removal must not be accepted as complete"
    );

    // Phase 6: the forward cleanup commit completes the batch.
    git(
        root,
        &["commit", "-m", "Cleanup resurrected change: change-a"],
    )
    .await;
    assert_eq!(
        resolve_state::classify_batch(&evidence, &items, &base_revision).await,
        BatchState::Complete
    );
    resolve_state::verify_final_integration(&evidence, &items, &base_revision)
        .await
        .expect("terminal verification must agree with the retry classifier");

    // Phase 7: untracked dirt in the target keeps the batch incomplete.
    std::fs::write(root.join("stray.txt"), "stray\n").unwrap();
    assert!(
        matches!(
            resolve_state::classify_batch(&evidence, &items, &base_revision).await,
            BatchState::UnsafeEvidence { .. }
        ),
        "a dirty target must never read as complete"
    );

    git(
        root,
        &["worktree", "remove", worktree.to_str().unwrap(), "--force"],
    )
    .await;
}
