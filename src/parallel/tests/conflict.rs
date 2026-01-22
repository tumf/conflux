//! Tests for conflict detection and resolution functionality.

use super::super::conflict::*;
use crate::vcs::{VcsBackend, VcsResult, VcsWarning, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Mock WorkspaceManager for testing conflict detection.
struct MockWorkspaceManager {
    conflicts: Vec<String>,
    status_output: String,
    log_output: String,
    repo_root: PathBuf,
}

impl MockWorkspaceManager {
    fn new(conflicts: Vec<String>) -> Self {
        Self {
            conflicts,
            status_output: "# On branch main\n# Unmerged paths:\n#   both modified:   src/main.rs"
                .to_string(),
            log_output: "commit abc123\nAuthor: Test\nDate: 2024-01-01\n\nTest commit".to_string(),
            repo_root: PathBuf::from("/tmp/test-repo"),
        }
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

    fn original_branch(&self) -> Option<String> {
        Some("main".to_string())
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        Ok(self.conflicts.clone())
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
    };

    let args2 = args1.clone(); // Clone instead of Copy
    let _args3 = args1; // Can still use args1 because it's Clone

    assert_eq!(args2.target_branch, "main");
}
