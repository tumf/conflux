//! In-memory doubles for conflict and sequential-resolve retry tests.
//!
//! One `WorkspaceManager` double serves both the conflict layer's own tests and
//! the parallel test suite, so neither can drift into asserting against a
//! workspace shape the other never sees.

use crate::vcs::{VcsBackend, VcsResult, VcsWarning, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// How the mock reports conflicts across successive `detect_conflicts` calls.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConflictProbe {
    /// Always report the configured conflicts.
    Static,
    /// Report conflicts on the initial detection, then report them cleared.
    ClearedAfterFirstCall,
    /// Fail detection, exercising the early-return path of resolve.
    AlwaysFails,
}

/// Mock WorkspaceManager for testing conflict detection.
pub(crate) struct MockWorkspaceManager {
    conflicts: Vec<String>,
    status_output: String,
    log_output: String,
    repo_root: PathBuf,
    probe: ConflictProbe,
    detect_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl MockWorkspaceManager {
    pub(crate) fn new(conflicts: Vec<String>) -> Self {
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

    pub(crate) fn with_repo_root(mut self, repo_root: PathBuf) -> Self {
        self.repo_root = repo_root;
        self
    }

    pub(crate) fn with_probe(mut self, probe: ConflictProbe) -> Self {
        self.probe = probe;
        self
    }

    pub(crate) fn with_status(mut self, status: String) -> Self {
        self.status_output = status;
        self
    }

    pub(crate) fn with_log(mut self, log: String) -> Self {
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
    ) -> VcsResult<crate::vcs::Workspace> {
        Ok(crate::vcs::Workspace {
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
    ) -> VcsResult<crate::vcs::Workspace> {
        Ok(crate::vcs::Workspace {
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

    fn workspaces(&self) -> Vec<crate::vcs::Workspace> {
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
