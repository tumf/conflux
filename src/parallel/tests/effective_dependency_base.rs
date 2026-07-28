//! Unit coverage for the effective dependency-base evidence shared by dependency
//! classification and analysis-input signatures.
//!
//! The bug these tests pin down is a silent divergence: dependency classification decides merge
//! evidence from a *named* base ref, while signature probing used to read the checkout `HEAD`
//! commit. When that ref advanced on its own, the signature stayed equal and the only timer
//! evaluation able to observe the newly integrated dependency was suppressed.
//!
//! Branch selection and ref-revision lookup both go through [`WorkspaceManager`], so a
//! recording double is enough to prove which source the scheduler asked for. No VCS
//! subprocess, repository state, or clock is involved.

use crate::config::OrchestratorConfig;
use crate::parallel::ParallelExecutor;
use crate::vcs::{
    VcsBackend, VcsError, VcsResult, VcsWarning, Workspace, WorkspaceInfo, WorkspaceManager,
    WorkspaceStatus,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tempfile::TempDir;

/// Workspace-manager double that records which revision source the scheduler consulted.
struct RecordingWorkspaceManager {
    repo_root: PathBuf,
    original_branch: String,
    current_branch: Option<String>,
    /// Revision the checkout `HEAD` reports. Signature probing must never use this.
    head_revision: String,
    /// Revision each named ref resolves to.
    ref_revisions: StdMutex<std::collections::HashMap<String, String>>,
    ref_lookup_failure: Option<String>,
    head_revision_calls: Arc<AtomicUsize>,
    ref_revision_calls: Arc<StdMutex<Vec<String>>>,
}

impl RecordingWorkspaceManager {
    fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            original_branch: "main".to_string(),
            current_branch: Some("main".to_string()),
            head_revision: "head-commit".to_string(),
            ref_revisions: StdMutex::new(std::collections::HashMap::new()),
            ref_lookup_failure: None,
            head_revision_calls: Arc::new(AtomicUsize::new(0)),
            ref_revision_calls: Arc::new(StdMutex::new(Vec::new())),
        }
    }

    fn on_branch(mut self, current_branch: &str) -> Self {
        self.current_branch = Some(current_branch.to_string());
        self
    }

    fn with_ref_revision(self, reference: &str, revision: &str) -> Self {
        self.ref_revisions
            .lock()
            .expect("ref revisions lock")
            .insert(reference.to_string(), revision.to_string());
        self
    }

    fn failing_ref_lookup(mut self, error: &str) -> Self {
        self.ref_lookup_failure = Some(error.to_string());
        self
    }

    fn head_revision_calls(&self) -> Arc<AtomicUsize> {
        self.head_revision_calls.clone()
    }

    fn ref_revision_calls(&self) -> Arc<StdMutex<Vec<String>>> {
        self.ref_revision_calls.clone()
    }
}

#[async_trait]
impl WorkspaceManager for RecordingWorkspaceManager {
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
        self.head_revision_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.head_revision.clone())
    }

    async fn current_branch(&self) -> VcsResult<Option<String>> {
        Ok(self.current_branch.clone())
    }

    async fn revision_for_ref(&self, reference: &str) -> VcsResult<String> {
        self.ref_revision_calls
            .lock()
            .expect("ref revision calls lock")
            .push(reference.to_string());
        if let Some(error) = &self.ref_lookup_failure {
            return Err(VcsError::git_command(error.clone()));
        }
        Ok(self
            .ref_revisions
            .lock()
            .expect("ref revisions lock")
            .get(reference)
            .cloned()
            .unwrap_or_else(|| format!("revision-of-{reference}")))
    }

    async fn create_workspace(
        &mut self,
        change_id: &str,
        _base_revision: Option<&str>,
    ) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: change_id.to_string(),
            path: self.repo_root.clone(),
            change_id: change_id.to_string(),
            base_revision: self.head_revision.clone(),
            status: WorkspaceStatus::Created,
        })
    }

    fn update_workspace_status(&mut self, _workspace_name: &str, _status: WorkspaceStatus) {}

    async fn merge_workspaces(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok("merge-rev".to_string())
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
        Ok(HashSet::new())
    }

    fn conflict_resolution_prompt(&self) -> &'static str {
        "test prompt"
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
        Ok(self.head_revision.clone())
    }

    async fn get_status(&self) -> VcsResult<String> {
        Ok(String::new())
    }

    async fn get_log_for_revisions(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok(String::new())
    }

    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        Ok(Vec::new())
    }

    fn forget_workspace_sync(&self, _workspace_name: &str) {}

    fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    async fn ensure_original_branch_initialized(&self) -> VcsResult<String> {
        Ok(self.original_branch.clone())
    }

    fn original_branch(&self) -> Option<String> {
        Some(self.original_branch.clone())
    }

    async fn find_existing_workspace(
        &mut self,
        _change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        Ok(None)
    }

    async fn reuse_workspace(&mut self, workspace_info: &WorkspaceInfo) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: workspace_info.workspace_name.clone(),
            path: workspace_info.path.clone(),
            change_id: workspace_info.change_id.clone(),
            base_revision: self.head_revision.clone(),
            status: WorkspaceStatus::Created,
        })
    }
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        ..Default::default()
    }
}

fn executor_with(
    repo_root: &Path,
    manager: RecordingWorkspaceManager,
) -> (
    ParallelExecutor,
    Arc<AtomicUsize>,
    Arc<StdMutex<Vec<String>>>,
) {
    let head_revision_calls = manager.head_revision_calls();
    let ref_revision_calls = manager.ref_revision_calls();
    let mut executor = ParallelExecutor::new(repo_root.to_path_buf(), test_config(), None);
    executor.set_workspace_manager(Box::new(manager));
    (executor, head_revision_calls, ref_revision_calls)
}

#[tokio::test]
async fn base_evidence_resolves_the_original_branch_ref_rather_than_checkout_head() {
    let temp_dir = TempDir::new().unwrap();
    let manager = RecordingWorkspaceManager::new(temp_dir.path().to_path_buf())
        .with_ref_revision("main", "base-commit");
    let (executor, head_revision_calls, ref_revision_calls) =
        executor_with(temp_dir.path(), manager);

    let evidence = executor
        .effective_dependency_base_evidence()
        .await
        .expect("effective dependency base evidence");

    assert_eq!(evidence.base_ref, "main");
    assert_eq!(
        evidence.revision, "base-commit",
        "the signature revision must be the named base ref's revision, not checkout HEAD"
    );
    assert_eq!(
        head_revision_calls.load(Ordering::SeqCst),
        0,
        "resolving dependency-base evidence must never read the checkout HEAD commit"
    );
    assert_eq!(
        ref_revision_calls
            .lock()
            .expect("ref revision calls lock")
            .as_slice(),
        ["main"],
        "exactly the selected base ref must be resolved"
    );
}

#[tokio::test]
async fn base_evidence_follows_the_current_integration_branch_selection() {
    let temp_dir = TempDir::new().unwrap();
    // A stacked run: dependency merge evidence is read from the integration branch, so signature
    // invalidation has to follow the same selection rule.
    let manager = RecordingWorkspaceManager::new(temp_dir.path().to_path_buf())
        .on_branch("integration-1")
        .with_ref_revision("integration-1", "integration-commit");
    let (executor, _head_revision_calls, ref_revision_calls) =
        executor_with(temp_dir.path(), manager);

    let evidence = executor
        .effective_dependency_base_evidence()
        .await
        .expect("effective dependency base evidence");

    assert_eq!(evidence.base_ref, "integration-1");
    assert_eq!(evidence.revision, "integration-commit");
    assert_eq!(
        ref_revision_calls
            .lock()
            .expect("ref revision calls lock")
            .as_slice(),
        ["integration-1"],
        "branch selection and revision lookup must name the same ref"
    );
}

#[tokio::test]
async fn signature_material_names_the_base_ref_and_its_revision() {
    let temp_dir = TempDir::new().unwrap();
    let manager = RecordingWorkspaceManager::new(temp_dir.path().to_path_buf())
        .on_branch("integration-1")
        .with_ref_revision("integration-1", "integration-commit");
    let (executor, head_revision_calls, _ref_revision_calls) =
        executor_with(temp_dir.path(), manager);

    let (base_revision, digests) = executor
        .probe_analysis_signature_materials(&[], &HashSet::new())
        .await
        .expect("probe analysis signature materials");

    assert_eq!(
        base_revision, "integration-1@integration-commit",
        "the signature must be able to distinguish both a re-pointed base and an advancing one"
    );
    assert!(digests.is_empty(), "no change was queued or in flight");
    assert_eq!(
        head_revision_calls.load(Ordering::SeqCst),
        0,
        "substituting the checkout HEAD commit would suppress the evaluation that observes a \
         dependency ref advancing"
    );
}

#[tokio::test]
async fn base_ref_revision_failure_is_reported_so_probing_can_fail_open() {
    let temp_dir = TempDir::new().unwrap();
    let manager = RecordingWorkspaceManager::new(temp_dir.path().to_path_buf())
        .failing_ref_lookup("ref lookup failed");
    let (executor, _head_revision_calls, _ref_revision_calls) =
        executor_with(temp_dir.path(), manager);

    let error = executor
        .probe_analysis_signature_materials(&[], &HashSet::new())
        .await
        .expect_err("an unresolvable base ref must be reported rather than silently substituted");

    assert!(
        error.contains("ref lookup failed"),
        "the fail-open path needs an operator-visible reason; saw {error}"
    );
}
