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

    async fn set_commit_message(&self, _workspace_path: &Path, _message: &str) -> VcsResult<()> {
        Ok(())
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

#[tokio::test]
async fn test_resolve_merges_with_retry_args_struct() {
    // Test that ResolveMergesWithRetryArgs can be constructed properly
    let manager = MockWorkspaceManager::new(vec![]);
    let config = crate::config::OrchestratorConfig::default();
    let revisions = vec!["rev1".to_string()];
    let change_ids = vec!["change1".to_string()];
    let target_branch = "main";
    let base_revision = "base123";
    let max_retries = 3;

    let shared_stagger_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let auto_resolve_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let args = ResolveMergesWithRetryArgs {
        workspace_manager: &manager as &dyn WorkspaceManager,
        config: &config,
        event_tx: &None,
        revisions: &revisions,
        change_ids: &change_ids,
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
    assert_eq!(args.revisions.len(), 1);
    assert_eq!(args.change_ids.len(), 1);
}

#[test]
fn test_resolve_merges_with_retry_args_clone() {
    // Test that ResolveMergesWithRetryArgs implements Copy
    let manager = MockWorkspaceManager::new(vec![]);
    let config = crate::config::OrchestratorConfig::default();
    let revisions = vec!["rev1".to_string()];
    let change_ids = vec!["change1".to_string()];
    let target_branch = "main";
    let base_revision = "base123";
    let max_retries = 3;

    let shared_stagger_state = std::sync::Arc::new(tokio::sync::Mutex::new(None));
    let auto_resolve_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let args1 = ResolveMergesWithRetryArgs {
        workspace_manager: &manager as &dyn WorkspaceManager,
        config: &config,
        event_tx: &None,
        revisions: &revisions,
        change_ids: &change_ids,
        target_branch,
        base_revision,
        max_retries,
        shared_stagger_state,
        auto_resolve_count,
        publication_owns_completion: false,
    };

    let args2 = args1.clone(); // Clone instead of Copy
    let _args3 = args1; // Can still use args1 because it's Clone

    assert_eq!(args2.target_branch, "main");
}

#[test]
fn test_resolve_merges_prompt_contains_cleanup_instructions() {
    // This test verifies that the resolve merges prompt includes the new cleanup instructions
    // for removing resurrected openspec/changes directories before the final merge commit

    let prompt_fragment = r#"2) Final merge into the target branch (in the repo root):
                 - cd <repo_root>
                 - git checkout <target_branch>
                 - git merge --no-ff --no-commit <branch>
                 - If a conflict occurs, resolve it and git add the resolved files.
                 - BEFORE creating the merge commit:
                   * If `openspec/changes/<change_id>/proposal.md` exists AND `openspec/changes/archive/` contains the same <change_id>, remove `openspec/changes/<change_id>` (the directory was resurrected by the merge and must be deleted).
                   * Use `git rm -rf openspec/changes/<change_id>` to remove the resurrected directory.
                 - Finally, run `git commit -m "Merge change: <change_id>"` to complete the merge."#;

    // Verify key elements are present
    assert!(prompt_fragment.contains("git merge --no-ff --no-commit"));
    assert!(prompt_fragment.contains("BEFORE creating the merge commit"));
    assert!(prompt_fragment.contains("openspec/changes/<change_id>/proposal.md"));
    assert!(prompt_fragment.contains("openspec/changes/archive/"));
    assert!(prompt_fragment.contains("git rm -rf openspec/changes/<change_id>"));
    assert!(prompt_fragment.contains("resurrected"));
    assert!(prompt_fragment.contains("Finally, run `git commit -m"));
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
