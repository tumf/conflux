//! Shared test-only fixtures for the parallel test modules.
//!
//! These helpers used to live in `executor.rs`, which made that module the de
//! facto owner of fixtures its siblings imported. Keeping them here gives the
//! parallel tests a single support boundary: workspace doubles, orchestrator
//! config builders, Git setup, and the failure helper the other three rely on.
//!
//! Nothing here asserts behavior; it only constructs it. Test bodies stay in
//! the thematic modules that own them.

// Support items are shared across sibling test modules, and individual modules
// are compiled with different feature sets, so unused-in-this-build helpers are
// expected rather than a defect.
#![allow(dead_code)]

use crate::config::OrchestratorConfig;
use crate::vcs::{VcsBackend, VcsError, VcsResult, VcsWarning, Workspace, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::process::Command;

pub(super) trait TestAssertionExt<T> {
    fn or_fail(self, context: &str) -> T;
}

impl<T, E> TestAssertionExt<T> for Result<T, E>
where
    E: Display,
{
    fn or_fail(self, context: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error}"),
        }
    }
}

impl<T> TestAssertionExt<T> for Option<T> {
    fn or_fail(self, context: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{context}: value was None"),
        }
    }
}

/// Helper function to create a test config with all required commands
pub(super) fn create_test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        ..Default::default()
    }
}

/// Helper to create test config with custom overrides
pub(super) fn create_test_config_with(overrides: OrchestratorConfig) -> OrchestratorConfig {
    let mut base = create_test_config();
    base.merge(overrides);
    base
}

pub(super) struct TestWorkspaceManager {
    merge_calls: Arc<AtomicUsize>,
    conflict_files: Vec<String>,
    repo_root: PathBuf,
    existing_workspaces: HashMap<String, WorkspaceInfo>,
    remove_existing_on_lookup: Arc<AtomicBool>,
    fail_original_branch: Arc<AtomicBool>,
    fail_existing_workspace_lookup: Arc<AtomicBool>,
}

impl TestWorkspaceManager {
    pub(super) fn new(merge_calls: Arc<AtomicUsize>) -> Self {
        Self {
            merge_calls,
            conflict_files: vec!["conflict.txt".to_string()],
            repo_root: PathBuf::from("/tmp/test-repo"),
            existing_workspaces: HashMap::new(),
            remove_existing_on_lookup: Arc::new(AtomicBool::new(false)),
            fail_original_branch: Arc::new(AtomicBool::new(false)),
            fail_existing_workspace_lookup: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn with_existing_workspace(mut self, change_id: &str, path: PathBuf) -> Self {
        self.existing_workspaces.insert(
            change_id.to_string(),
            WorkspaceInfo {
                path,
                change_id: change_id.to_string(),
                workspace_name: format!("ws-{change_id}"),
                last_modified: std::time::SystemTime::now(),
            },
        );
        self
    }

    pub(super) fn with_remove_existing_on_lookup(self) -> Self {
        self.remove_existing_on_lookup.store(true, Ordering::SeqCst);
        self
    }

    /// Make base identity resolution fail, as a detached HEAD or a lost original
    /// branch does in a real repository.
    pub(super) fn with_failing_original_branch(self) -> Self {
        self.fail_original_branch.store(true, Ordering::SeqCst);
        self
    }

    /// Make workspace discovery fail the way a broken or racing worktree list
    /// does: the question is asked and produces no answer at all, which is not
    /// the same as answering "this change has no workspace".
    pub(super) fn with_failing_existing_workspace_lookup(self) -> Self {
        self.fail_existing_workspace_lookup
            .store(true, Ordering::SeqCst);
        self
    }
}

#[async_trait]
impl WorkspaceManager for TestWorkspaceManager {
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
        change_id: &str,
        _base_revision: Option<&str>,
    ) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: change_id.to_string(),
            path: PathBuf::from("/tmp/test-workspace"),
            change_id: change_id.to_string(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Created,
        })
    }

    fn update_workspace_status(&mut self, _workspace_name: &str, _status: WorkspaceStatus) {}

    async fn merge_workspaces(&self, _revisions: &[String]) -> VcsResult<String> {
        let attempt = self.merge_calls.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            Err(VcsError::Conflict {
                backend: VcsBackend::Git,
                details: "conflict".to_string(),
            })
        } else {
            Ok("merge-rev".to_string())
        }
    }

    async fn cleanup_workspace(&mut self, _workspace_name: &str) -> VcsResult<()> {
        Ok(())
    }

    async fn cleanup_all(&mut self) -> VcsResult<()> {
        Ok(())
    }

    fn max_concurrent(&self) -> usize {
        1
    }

    fn workspaces(&self) -> Vec<Workspace> {
        Vec::new()
    }

    async fn list_worktree_change_ids(&self) -> VcsResult<HashSet<String>> {
        self.merge_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.existing_workspaces.keys().cloned().collect())
    }

    fn conflict_resolution_prompt(&self) -> &'static str {
        "test prompt"
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
        Ok("rev".to_string())
    }

    async fn get_status(&self) -> VcsResult<String> {
        Ok(String::new())
    }

    async fn get_log_for_revisions(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok(String::new())
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        Ok(self.conflict_files.clone())
    }

    fn forget_workspace_sync(&self, _workspace_name: &str) {}

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    async fn ensure_original_branch_initialized(&self) -> VcsResult<String> {
        if self.fail_original_branch.load(Ordering::SeqCst) {
            return Err(VcsError::git_command(
                "detached HEAD with no recorded original branch",
            ));
        }
        Ok("main".to_string())
    }

    fn original_branch(&self) -> Option<String> {
        if self.fail_original_branch.load(Ordering::SeqCst) {
            return None;
        }
        Some("main".to_string())
    }

    async fn find_existing_workspace(
        &mut self,
        change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        if self.fail_existing_workspace_lookup.load(Ordering::SeqCst) {
            return Err(VcsError::git_command("worktree list unavailable"));
        }
        if self.remove_existing_on_lookup.load(Ordering::SeqCst) {
            Ok(self.existing_workspaces.remove(change_id))
        } else {
            Ok(self.existing_workspaces.get(change_id).cloned())
        }
    }

    async fn reuse_workspace(&mut self, workspace_info: &WorkspaceInfo) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: workspace_info.workspace_name.clone(),
            path: workspace_info.path.clone(),
            change_id: workspace_info.change_id.clone(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Created,
        })
    }
}

pub(super) async fn init_git_repo(repo_root: &Path) {
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    std::fs::write(repo_root.join("README.md"), "base").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
}
