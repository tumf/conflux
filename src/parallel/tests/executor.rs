//! Tests for ParallelExecutor and related functionality.

use super::super::*;
use crate::agent::AgentRunner;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::default_retry_patterns;
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::orchestration::acceptance::MAX_ACCEPTANCE_RETRY_CYCLES;
use crate::orchestration::state::{ExecutionMode, OrchestratorState, ReducerCommand, WaitState};
use crate::parallel::dedup::DiagnosticDeduplicationStore;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::executor::{
    execute_acceptance_in_workspace, execute_archive_finalization_in_workspace,
    execute_archive_in_workspace,
};
use crate::parallel::merge::MergeAttempt;
use crate::parallel::queue_state::ReanalysisDispatchContext;
use crate::vcs::git::commands::get_current_commit;
#[cfg(feature = "heavy-tests")]
use crate::vcs::GitWorkspaceManager;
use crate::vcs::{VcsBackend, VcsError, VcsResult, VcsWarning, Workspace, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex, RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

trait TestAssertionExt<T> {
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

const DEFAULT_STAGGER_DELAY_MS: u64 = 2000;
const DEFAULT_MAX_RETRIES: u32 = 2;
const DEFAULT_RETRY_DELAY_MS: u64 = 5000;
const DEFAULT_RETRY_IF_DURATION_UNDER_SECS: u64 = 5;

/// Helper function to create a test config with all required commands
fn create_test_config() -> OrchestratorConfig {
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
fn create_test_config_with(overrides: OrchestratorConfig) -> OrchestratorConfig {
    let mut base = create_test_config();
    base.merge(overrides);
    base
}

#[test]
#[should_panic(expected = "option missing: value was None")]
fn test_or_fail_option_reports_context() {
    let value: Option<u32> = None;
    let _ = value.or_fail("option missing");
}

#[test]
#[should_panic(expected = "io failed: boom")]
fn test_or_fail_result_reports_context_and_error() {
    let result: Result<(), &str> = Err("boom");
    result.or_fail("io failed");
}

#[test]
fn test_parallel_executor_creation() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let executor = ParallelExecutor::new(repo_root, config, None);

    assert_eq!(executor.max_conflict_retries, 3);
}

#[test]
fn test_parallel_executor_preserves_supplied_shared_stagger_state() {
    let shared_stagger_state = Arc::new(Mutex::new(None));
    let executor = ParallelExecutor::with_backend_and_queue_and_stagger(
        PathBuf::from("/tmp/test-repo"),
        create_test_config(),
        None,
        VcsBackend::Git,
        None,
        Some(shared_stagger_state.clone()),
    );

    assert!(Arc::ptr_eq(
        &executor.shared_stagger_state,
        &shared_stagger_state
    ));
    assert!(Arc::ptr_eq(
        &executor.ai_runner.shared_stagger_state(),
        &shared_stagger_state
    ));
}

#[allow(dead_code)]
pub(super) struct TestWorkspaceManager {
    merge_calls: Arc<AtomicUsize>,
    conflict_files: Vec<String>,
    #[allow(dead_code)]
    repo_root: PathBuf,
    existing_workspaces: HashMap<String, WorkspaceInfo>,
    remove_existing_on_lookup: Arc<AtomicBool>,
}

impl TestWorkspaceManager {
    #[allow(dead_code)]
    pub(super) fn new(merge_calls: Arc<AtomicUsize>) -> Self {
        Self {
            merge_calls,
            conflict_files: vec!["conflict.txt".to_string()],
            repo_root: PathBuf::from("/tmp/test-repo"),
            existing_workspaces: HashMap::new(),
            remove_existing_on_lookup: Arc::new(AtomicBool::new(false)),
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn with_remove_existing_on_lookup(self) -> Self {
        self.remove_existing_on_lookup.store(true, Ordering::SeqCst);
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
        Ok("main".to_string())
    }

    fn original_branch(&self) -> Option<String> {
        Some("main".to_string())
    }

    async fn find_existing_workspace(
        &mut self,
        change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
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

async fn init_git_repo(repo_root: &Path) {
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

async fn commit_workspace_change(
    workspace: &Workspace,
    filename: &str,
    contents: &str,
    message: &str,
) {
    std::fs::write(workspace.path.join(filename), contents).or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("unexpected error");
}

fn write_change_proposal(repo_root: &Path, change_id: &str, dependencies: &[&str]) {
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create change proposal directory");
    let dependencies = dependencies
        .iter()
        .map(|dependency| format!("  - {dependency}\n"))
        .collect::<String>();
    std::fs::write(
        change_dir.join("proposal.md"),
        format!("---\ndependencies:\n{dependencies}---\n# {change_id}\n"),
    )
    .or_fail("write change proposal");
}

async fn commit_archive_to_base(repo_root: &Path, archive_leaf: &str, change_id: &str) {
    let archive_dir = repo_root
        .join("openspec/changes/archive")
        .join(archive_leaf);
    std::fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    std::fs::write(
        archive_dir.join("proposal.md"),
        format!("# Archived {change_id}\n"),
    )
    .or_fail("unexpected error");
    // A real archive moves the change: leaving the active directory behind is
    // contradictory base evidence, not proof of completion.
    let active_dir = repo_root.join("openspec/changes").join(change_id);
    if active_dir.exists() {
        std::fs::remove_dir_all(&active_dir).or_fail("unexpected error");
    }
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", &format!("Archive {change_id}")])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
}
#[tokio::test]
async fn resolving_dependency_blocks_its_dependent_but_not_unrelated_dispatch() {
    let temp = TempDir::new().or_fail("create temp repo");
    init_git_repo(temp.path()).await;
    std::fs::create_dir_all(temp.path().join("openspec/changes/dependent"))
        .or_fail("create dependent change");
    std::fs::write(
        temp.path().join("openspec/changes/dependent/proposal.md"),
        "---\ndependencies:\n  - resolving\n---\n# Dependent\n",
    )
    .or_fail("write dependent proposal metadata");
    std::fs::write(
        temp.path().join("openspec/changes/dependent/tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .or_fail("write dependent tasks");
    std::fs::create_dir_all(temp.path().join("openspec/changes/unrelated"))
        .or_fail("create unrelated change");
    std::fs::write(
        temp.path().join("openspec/changes/unrelated/proposal.md"),
        "# Unrelated\n",
    )
    .or_fail("write unrelated proposal");
    std::fs::write(
        temp.path().join("openspec/changes/unrelated/tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .or_fail("write unrelated tasks");
    Command::new("git")
        .args(["add", "."])
        .current_dir(temp.path())
        .output()
        .await
        .or_fail("stage fixture changes");
    Command::new("git")
        .args(["commit", "-m", "Add dispatch fixtures"])
        .current_dir(temp.path())
        .output()
        .await
        .or_fail("commit fixture changes");

    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config_with(OrchestratorConfig {
            workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
            ..Default::default()
        }),
        Some(tx),
    );
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![
            "resolving".to_string(),
            "dependent".to_string(),
            "unrelated".to_string(),
        ],
        1,
        ExecutionMode::Parallel,
    )));
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: "resolving".to_string(),
            command: "resolve".to_string(),
        });
    executor.set_shared_orchestrator_state(shared.clone());

    let analysis = crate::analyzer::AnalysisResult {
        order: vec!["dependent".to_string(), "unrelated".to_string()],
        dependencies: HashMap::new(),
        groups: None,
    };
    let selected = executor
        .select_changes_for_dispatch(&analysis, 2, &HashSet::new())
        .await;
    assert_eq!(
        selected,
        vec!["unrelated"],
        "resolve blocks only its dependent"
    );

    let mut queued = vec![
        crate::openspec::Change {
            dependencies: vec!["resolving".to_string()],
            ..make_test_change("dependent")
        },
        make_test_change("unrelated"),
    ];
    let mut in_flight = HashSet::new();
    let semaphore = Arc::new(Semaphore::new(2));
    let mut join_set = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        temp.path().to_path_buf(),
    );
    executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 2,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &dependent_ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("resolve-gated dispatch");
    while join_set.join_next().await.is_some() {}

    let mut saw_unrelated_apply = false;
    let mut saw_dependent_apply = false;
    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
    {
        match event {
            ExecutionEvent::ApplyStarted { change_id, .. } if change_id == "unrelated" => {
                saw_unrelated_apply = true
            }
            ExecutionEvent::ApplyStarted { change_id, .. } if change_id == "dependent" => {
                saw_dependent_apply = true
            }
            _ => {}
        }
    }
    assert!(
        saw_unrelated_apply,
        "unrelated change must retain parallel dispatch"
    );
    assert!(
        !saw_dependent_apply,
        "dependent must not emit ApplyStarted while its dependency resolves"
    );

    commit_archive_to_base(temp.path(), "2026-07-21-resolving", "resolving").await;
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "resolving".to_string(),
            revision: "merged".to_string(),
        });
    let selected = executor
        .select_changes_for_dispatch(&analysis, 2, &HashSet::new())
        .await;
    assert_eq!(selected, vec!["dependent", "unrelated"]);

    executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 2,
            iteration: 2,
            reanalysis_reason: ReanalysisReason::ResolveCompletion,
            analyzer: &dependent_ready_analysis_result,
            semaphore: Arc::new(Semaphore::new(2)),
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("merged dependency dispatch");
    while join_set.join_next().await.is_some() {}
    let mut saw_dependent_after_merge = false;
    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
    {
        if matches!(
            event,
            ExecutionEvent::ApplyStarted { change_id, .. } if change_id == "dependent"
        ) {
            saw_dependent_after_merge = true;
        }
    }
    assert!(
        saw_dependent_after_merge,
        "dependent must emit ApplyStarted after merge evidence"
    );
}

#[tokio::test]
async fn resolving_dependency_diagnostic_dedupes_and_reemits_after_signature_change() {
    let temp = TempDir::new().or_fail("unexpected error");
    init_git_repo(temp.path()).await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["resolving-a".to_string(), "dependent".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: "resolving-a".to_string(),
            command: "resolve".to_string(),
        });
    executor.set_shared_orchestrator_state(shared.clone());
    let resolving_analysis = crate::analyzer::AnalysisResult {
        order: vec!["dependent".to_string()],
        dependencies: HashMap::from([("dependent".to_string(), vec!["resolving-a".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    assert!(executor
        .select_changes_for_dispatch(&resolving_analysis, 1, &in_flight)
        .await
        .is_empty());
    assert!(executor
        .select_changes_for_dispatch(&resolving_analysis, 1, &in_flight)
        .await
        .is_empty());
    assert_eq!(
        drain_dependency_events(&mut event_rx, "dependent"),
        vec!["blocked:resolving-a".to_string()],
        "unchanged resolving blocker must emit one diagnostic event"
    );

    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ResolveFailed {
            change_id: "resolving-a".to_string(),
            error: "conflict remains".to_string(),
        });
    assert!(executor
        .select_changes_for_dispatch(&resolving_analysis, 1, &in_flight)
        .await
        .is_empty());
    assert_eq!(
        drain_dependency_events(&mut event_rx, "dependent"),
        vec!["blocked:resolving-a".to_string()],
        "changed blocker signature must re-emit a diagnostic event"
    );
}

#[tokio::test]
async fn metadata_read_failure_blocks_dispatch_even_without_analyzer_dependencies() {
    for (change_id, proposal) in [
        ("missing-proposal", None),
        (
            "invalid-proposal",
            Some("---\ndependencies: [broken\n---\n# Invalid\n"),
        ),
    ] {
        let temp = TempDir::new().or_fail("unexpected error");
        init_git_repo(temp.path()).await;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
        let mut executor = ParallelExecutor::new(
            temp.path().to_path_buf(),
            create_test_config(),
            Some(event_tx),
        );
        let proposal_dir = temp.path().join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&proposal_dir).or_fail("create proposal fixture");
        if let Some(proposal) = proposal {
            std::fs::write(proposal_dir.join("proposal.md"), proposal)
                .or_fail("write invalid proposal fixture");
        }
        let analysis = crate::analyzer::AnalysisResult {
            order: vec![change_id.to_string()],
            dependencies: HashMap::new(),
            groups: None,
        };

        assert!(
            executor
                .select_changes_for_dispatch(&analysis, 1, &HashSet::new())
                .await
                .is_empty(),
            "{change_id} must not dispatch"
        );
        let mut saw_metadata_error = false;
        while let Ok(event) = event_rx.try_recv() {
            if let ExecutionEvent::Error { message } = event {
                saw_metadata_error |= message.contains("dependency metadata could not be read");
            }
        }
        assert!(
            saw_metadata_error,
            "{change_id} metadata failure must block dispatch visibly"
        );
    }
}

#[tokio::test]
async fn test_dependency_blocker_diagnostics_dedupe_and_reemit_on_signature_change() {
    let temp = TempDir::new().or_fail("unexpected error");
    write_change_proposal(temp.path(), "dependent", &["dep-a"]);
    let rejected_dir = temp.path().join("openspec/changes/dep-a");

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    let analysis = crate::analyzer::AnalysisResult {
        order: vec!["dependent".to_string()],
        dependencies: HashMap::from([("dependent".to_string(), vec!["dep-a".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let first = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    let second = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;

    assert!(first.is_empty());
    assert!(second.is_empty());
    let mut missing_errors = 0;
    while let Ok(event) = event_rx.try_recv() {
        if let ParallelEvent::Error { message } = event {
            if message.contains("missing dependency 'dep-a'") {
                missing_errors += 1;
            }
        }
    }
    assert_eq!(
        missing_errors, 1,
        "unchanged missing blocker should emit once"
    );

    std::fs::create_dir_all(&rejected_dir).or_fail("unexpected error");
    std::fs::write(rejected_dir.join("proposal.md"), "# Dep A\n").or_fail("unexpected error");
    std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED\n").or_fail("unexpected error");
    let third = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    assert!(third.is_empty());

    let mut rejected_errors = 0;
    while let Ok(event) = event_rx.try_recv() {
        if let ParallelEvent::Error { message } = event {
            if message.contains("rejected dependency 'dep-a'") {
                rejected_errors += 1;
            }
        }
    }
    assert_eq!(rejected_errors, 1, "changed blocker class should re-emit");
}

#[tokio::test]
async fn test_terminal_error_change_is_not_selected_until_explicit_retry() {
    let temp = TempDir::new().or_fail("unexpected error");
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue("alpha".to_string()));
        guard.apply_execution_event(&crate::events::ExecutionEvent::ProcessingError {
            id: "alpha".to_string(),
            error: "boom".to_string(),
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let analysis = crate::analyzer::AnalysisResult {
        order: vec!["alpha".to_string()],
        dependencies: HashMap::new(),
        groups: None,
    };
    let in_flight = HashSet::new();

    let blocked = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    assert!(blocked.is_empty());

    shared
        .write()
        .await
        .apply_command(ReducerCommand::RetryError("alpha".to_string()));
    let selected = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    assert_eq!(selected, vec!["alpha".to_string()]);
}

#[tokio::test]
async fn test_dependency_on_terminal_error_is_blocked_until_retry_and_success() {
    let temp = TempDir::new().or_fail("unexpected error");
    init_git_repo(temp.path()).await;
    write_change_proposal(temp.path(), "alpha", &[]);
    write_change_proposal(temp.path(), "beta", &["alpha"]);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string(), "beta".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue("alpha".to_string()));
        guard.apply_command(ReducerCommand::AddToQueue("beta".to_string()));
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "alpha".to_string(),
            error: "boom".to_string(),
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let analysis = crate::analyzer::AnalysisResult {
        order: vec!["alpha".to_string(), "beta".to_string()],
        dependencies: HashMap::from([("beta".to_string(), vec!["alpha".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let blocked = executor
        .select_changes_for_dispatch(&analysis, 2, &in_flight)
        .await;
    assert!(blocked.is_empty());

    let mut saw_error_dependency_block = false;
    while let Ok(event) = event_rx.try_recv() {
        if let ParallelEvent::Error { message } = event {
            if message.contains("blocked by errored dependency 'alpha'") {
                saw_error_dependency_block = true;
            }
        }
    }
    assert!(
        saw_error_dependency_block,
        "errored dependency should emit a diagnostic"
    );

    shared
        .write()
        .await
        .apply_command(ReducerCommand::RetryError("alpha".to_string()));
    let retry_selected = executor
        .select_changes_for_dispatch(&analysis, 2, &in_flight)
        .await;
    assert_eq!(retry_selected, vec!["alpha".to_string()]);

    commit_archive_to_base(temp.path(), "2026-05-12-alpha", "alpha").await;
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "alpha".to_string(),
            revision: "merged".to_string(),
        });

    let after_success_analysis = crate::analyzer::AnalysisResult {
        order: vec!["beta".to_string()],
        dependencies: HashMap::from([("beta".to_string(), vec!["alpha".to_string()])]),
        groups: None,
    };
    let after_success = executor
        .select_changes_for_dispatch(&after_success_analysis, 2, &in_flight)
        .await;
    assert_eq!(after_success, vec!["beta".to_string()]);
}

#[tokio::test]
async fn test_dependency_blocker_archived_unblocks_dispatch_after_base_merge() {
    let temp = TempDir::new().or_fail("unexpected error");
    init_git_repo(temp.path()).await;
    write_change_proposal(temp.path(), "dependent", &["dep-a"]);
    let rejected_dir = temp.path().join("openspec/changes/dep-a");
    std::fs::create_dir_all(&rejected_dir).or_fail("unexpected error");
    std::fs::write(rejected_dir.join("proposal.md"), "# Dep A\n").or_fail("unexpected error");
    std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED\n").or_fail("unexpected error");

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        temp.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    let analysis = crate::analyzer::AnalysisResult {
        order: vec!["dependent".to_string()],
        dependencies: HashMap::from([("dependent".to_string(), vec!["dep-a".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let blocked = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    assert!(blocked.is_empty());

    std::fs::remove_file(rejected_dir.join("REJECTED.md")).or_fail("unexpected error");
    commit_archive_to_base(temp.path(), "2026-05-09-dep-a", "dep-a").await;

    let selected = executor
        .select_changes_for_dispatch(&analysis, 1, &in_flight)
        .await;
    assert_eq!(selected, vec!["dependent".to_string()]);
    assert!(executor.force_recreate_worktree.contains("dependent"));
}

#[tokio::test]
async fn test_queue_reconciliation_skips_archived_dirty_candidate_when_post_archive_merge_active() {
    let temp = TempDir::new().or_fail("unexpected error");
    init_git_repo(temp.path()).await;

    let workspace_path = temp.path().join("worktrees/ws-gamma");
    std::fs::create_dir_all(&workspace_path).or_fail("unexpected error");
    let worktree_changes_dir = workspace_path.join("openspec/changes");
    std::fs::create_dir_all(worktree_changes_dir.join("archive/2026-05-10-gamma"))
        .or_fail("unexpected error");
    std::fs::write(
        worktree_changes_dir.join("archive/2026-05-10-gamma/proposal.md"),
        "# Gamma\n",
    )
    .or_fail("unexpected error");

    let merge_calls = Arc::new(AtomicUsize::new(0));
    let manager = TestWorkspaceManager::new(merge_calls)
        .with_existing_workspace("gamma", workspace_path.clone());
    let mut executor = ParallelExecutor::new(temp.path().to_path_buf(), create_test_config(), None);
    executor.workspace_manager = Box::new(manager);
    executor.set_shared_orchestrator_state(Arc::new(tokio::sync::RwLock::new(
        crate::orchestration::state::OrchestratorState::with_mode(
            vec!["gamma".to_string()],
            0,
            crate::orchestration::state::ExecutionMode::Parallel,
        ),
    )));

    let _active_merge_guard =
        crate::parallel::merge::ActivePostArchiveMergeGuard::force_register_for_test("gamma");
    let mut queued = Vec::new();
    let in_flight = HashSet::new();

    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(outcome.total_added(), 0);
    assert!(queued.is_empty());
}

#[test]
fn test_skip_reason_for_merge_deferred_dependency() {
    let merge_calls = Arc::new(AtomicUsize::new(0));
    let manager = TestWorkspaceManager::new(merge_calls);
    let mut change_dependencies = HashMap::new();
    change_dependencies.insert("change-b".to_string(), vec!["change-a".to_string()]);
    let mut resolve_wait_changes = HashSet::new();
    resolve_wait_changes.insert("change-a".to_string());

    // Create test AI runner
    let shared_stagger_state = Arc::new(Mutex::new(None));
    let config = create_test_config();
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 1,
        repo_root: PathBuf::from("/tmp/test-repo"),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies,
        resolve_wait_changes,
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        shared_stagger_state,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    // MergeWait dependencies are NOT skip reasons; they are handled as blocked/queued status
    // via dependency resolution checks (is_dependency_resolved). Only failed dependencies
    // are skip reasons.
    assert!(executor.skip_reason_for_change("change-b").is_none());
    assert!(executor.skip_reason_for_change("change-c").is_none());
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_resolve_merge_aborts_when_base_dirty() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    let repo_dir = tempfile::TempDir::new().or_fail("create temp repo");
    let worktree_dir = tempfile::TempDir::new().or_fail("create worktree base");
    let repo_root = repo_dir.path();
    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(worktree_dir.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let mut manager = GitWorkspaceManager::new(
        worktree_dir.path().to_path_buf(),
        repo_root.to_path_buf(),
        1,
        config.clone(),
    );
    let workspace = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("create workspace");
    commit_workspace_change(&workspace, "change-a.txt", "A", "Apply: change-a").await;

    std::fs::write(repo_root.join("dirty.txt"), "dirty").or_fail("dirty base");

    let result = resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a").await;
    assert!(result.is_err(), "dirty base must abort deferred merge");

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("read merge log");
    assert!(!String::from_utf8_lossy(&merge_log.stdout).contains("Merge change: change-a"));
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_conflictless_path_skips_resolve_started_event() {
    use tokio::sync::mpsc;

    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh -c 'echo should-not-run-resolve'".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    let workspace_a = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("unexpected error");
    commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;

    // Prepare the same state as archived sequential merge handoff:
    // `git merge --no-ff --no-commit <worktree>` succeeded on main,
    // MERGE_HEAD exists, and there are no unresolved conflicts.
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["merge", "--no-ff", "--no-commit", &workspace_a.name])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    let (event_tx, mut event_rx) = mpsc::channel(64);

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: Some(event_tx),
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    let revisions = vec![workspace_a.name.clone()];
    let change_ids = vec!["change-a".to_string()];

    executor
        .merge_and_resolve_with(
            &revisions,
            &change_ids,
            |_revs, _details| async move { Ok(()) },
        )
        .await
        .or_fail("unexpected error");

    let mut saw_resolve_started = false;
    let mut saw_resolve_completed = false;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            ParallelEvent::ResolveStarted { .. } => saw_resolve_started = true,
            ParallelEvent::ResolveCompleted { change_id, .. } if change_id == "change-a" => {
                saw_resolve_completed = true
            }
            _ => {}
        }
    }

    assert!(!saw_resolve_started);
    assert!(saw_resolve_completed);
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_conflict_path_emits_resolve_started_event() {
    use tokio::sync::mpsc;

    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    // main側で先に変更を入れて、ワークツリー側と衝突を作る
    std::fs::write(repo_root.join("conflict.txt"), "main").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Main conflict seed"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    let workspace_a = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("unexpected error");
    std::fs::write(workspace_a.path.join("conflict.txt"), "worktree").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");

    let resolver_script = repo_root.join("merge-resolver.sh");
    let script_contents = format!(
        "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            if ! git merge --no-ff -m 'Pre-sync base into change-a' main; then\n\
              if git rev-parse -q --verify MERGE_HEAD >/dev/null 2>&1; then\n\
                git checkout --ours conflict.txt\n\
                git add -A\n\
                git commit -m 'Pre-sync base into change-a'\n\
              else\n\
                exit 1\n\
              fi\n\
            fi\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n",
        workspace_a.path.to_string_lossy(),
        workspace_a.name,
        workspace_a.name
    );
    std::fs::write(&resolver_script, script_contents).or_fail("unexpected error");

    let (event_tx, mut event_rx) = mpsc::channel(64);

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: Some(event_tx),
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    let revisions = vec![workspace_a.name.clone()];
    let change_ids = vec!["change-a".to_string()];

    executor
        .merge_and_resolve_with(
            &revisions,
            &change_ids,
            |_revs, _details| async move { Ok(()) },
        )
        .await
        .or_fail("unexpected error");

    let mut saw_resolve_started = false;
    let mut saw_resolve_completed = false;
    let mut resolve_started_command: Option<String> = None;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            ParallelEvent::ResolveStarted { change_id, command } if change_id == "change-a" => {
                saw_resolve_started = true;
                resolve_started_command = Some(command);
            }
            ParallelEvent::ResolveCompleted { change_id, .. } if change_id == "change-a" => {
                saw_resolve_completed = true
            }
            _ => {}
        }
    }

    assert!(saw_resolve_started);
    assert!(saw_resolve_completed);
    let command = resolve_started_command.expect("resolve started command must exist");
    assert!(command.contains("merge-resolver.sh"));
    assert!(!command.contains("(unknown)"));
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_retries_when_merge_commit_missing() {
    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

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

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("unexpected error");
    let workspace_b = manager
        .create_workspace("change-b", None)
        .await
        .or_fail("unexpected error");

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");

    std::fs::write(workspace_b.path.join("change-b.txt"), "B").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .or_fail("unexpected error");

    let resolver_script = repo_root.join("merge-resolver.sh");
    let script_contents = format!(
        "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            if [ -f .git/merge-missing-marker ]; then\n\
              cd \"{}\"\n\
              git checkout {}\n\
              git merge --no-ff -m 'Pre-sync base into change-b' main\n\
              cd \"$ROOT\"\n\
              git checkout main\n\
              git merge --no-ff -m 'Merge change: change-b' {}\n\
              exit 0\n\
            fi\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            touch .git/merge-missing-marker\n",
        workspace_b.path.to_string_lossy(),
        workspace_b.name,
        workspace_b.name,
        workspace_a.path.to_string_lossy(),
        workspace_a.name,
        workspace_a.name
    );
    std::fs::write(&resolver_script, script_contents).or_fail("unexpected error");

    // Create test AI runner

    let shared_stagger_state = Arc::new(Mutex::new(None));

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

        max_retries: DEFAULT_MAX_RETRIES,

        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

        retry_error_patterns: default_retry_patterns(),

        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    let revisions = vec![workspace_a.name, workspace_b.name];
    let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

    executor
        .merge_and_resolve_with(
            &revisions,
            &change_ids,
            |_revs, _details| async move { Ok(()) },
        )
        .await
        .or_fail("unexpected error");

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
    assert!(merge_messages.contains("Merge change: change-a"));
    assert!(merge_messages.contains("Merge change: change-b"));
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_resolves_conflict_with_resolve_command() {
    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

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

    std::fs::write(repo_root.join("conflict.txt"), "base").or_fail("unexpected error");

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

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("unexpected error");
    let workspace_b = manager
        .create_workspace("change-b", None)
        .await
        .or_fail("unexpected error");

    std::fs::write(workspace_a.path.join("conflict.txt"), "A").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");

    std::fs::write(workspace_b.path.join("conflict.txt"), "B").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .or_fail("unexpected error");

    let resolver_script = repo_root.join("merge-resolver.sh");
    let script_contents = format!(
        "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            if ! git merge --no-ff -m 'Pre-sync base into change-b' main; then\n\
              if git rev-parse -q --verify MERGE_HEAD >/dev/null 2>&1; then\n\
                git checkout --ours conflict.txt\n\
                git add -A\n\
                git commit -m 'Pre-sync base into change-b'\n\
              else\n\
                exit 1\n\
              fi\n\
            fi\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-b' {}\n",
        workspace_a.path.to_string_lossy(),
        workspace_a.name,
        workspace_a.name,
        workspace_b.path.to_string_lossy(),
        workspace_b.name,
        workspace_b.name
    );
    std::fs::write(&resolver_script, script_contents).or_fail("unexpected error");

    // Create test AI runner

    let shared_stagger_state = Arc::new(Mutex::new(None));

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

        max_retries: DEFAULT_MAX_RETRIES,

        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

        retry_error_patterns: default_retry_patterns(),

        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    let revisions = vec![workspace_a.name, workspace_b.name];
    let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

    executor
        .merge_and_resolve_with(
            &revisions,
            &change_ids,
            |_revs, _details| async move { Ok(()) },
        )
        .await
        .or_fail("unexpected error");

    let merged_contents =
        std::fs::read_to_string(repo_root.join("conflict.txt")).or_fail("unexpected error");
    assert!(merged_contents.contains('B'));
}

#[cfg(feature = "heavy-tests")]
#[cfg(unix)]
#[tokio::test]
async fn test_merge_retries_after_pre_commit_changes() {
    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

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

    std::fs::write(repo_root.join("hooked.txt"), "base").or_fail("unexpected error");

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

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    let workspace_a = manager
        .create_workspace("change-a", None)
        .await
        .or_fail("unexpected error");

    std::fs::write(repo_root.join("main.txt"), "main").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .or_fail("unexpected error");

    let hooks_dir = repo_root.join(".git/hooks");
    let hook_path = hooks_dir.join("pre-commit");
    let hook_contents = "#!/bin/sh\n\
        set -e\n\
        COMMON_DIR=$(git rev-parse --git-common-dir)\n\
        MARKER=\"$COMMON_DIR/hooks/pre-commit-ran\"\n\
        if [ ! -f \"$MARKER\" ]; then\n\
          echo 'hooked' >> hooked.txt\n\
          git add hooked.txt\n\
          touch \"$MARKER\"\n\
          exit 1\n\
        fi\n\
        exit 0\n";
    std::fs::write(&hook_path, hook_contents).or_fail("unexpected error");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)
            .or_fail("unexpected error")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).or_fail("unexpected error");
    }

    let resolver_script = repo_root.join("merge-resolver.sh");
    let script_contents = format!(
        "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff --no-commit main\n\
            if ! git commit -m 'Pre-sync base into change-a'; then\n\
              git add -A\n\
              git commit -m 'Pre-sync base into change-a'\n\
            fi\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff --no-commit {}\n\
            if ! git commit -m 'Merge change: change-a'; then\n\
              git add -A\n\
              git commit -m 'Merge change: change-a'\n\
            fi\n",
        workspace_a.path.to_string_lossy(),
        workspace_a.name,
        workspace_a.name
    );
    std::fs::write(&resolver_script, script_contents).or_fail("unexpected error");

    // Create test AI runner

    let shared_stagger_state = Arc::new(Mutex::new(None));

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

        max_retries: DEFAULT_MAX_RETRIES,

        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

        retry_error_patterns: default_retry_patterns(),

        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

    let executor = ParallelExecutor {
        acceptance_stall_state_root: None,
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        explicit_retry: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        reject_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        dependency_blocker_fingerprints: HashMap::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        last_available_slots: None,
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        scheduler_lifetime: SchedulerLifetime::Finite,
        post_archive_action: super::super::PostArchiveAction::MergeToBase,
        shared_orchestrator_state: None,
        last_dispatched_resolve_wait_changes: HashSet::new(),
        last_dispatched_reject_wait_changes: HashSet::new(),
        resolve_wait_retry_triggered: false,
        last_resolve_wait_base_dirty: None,
        diagnostic_dedup: DiagnosticDeduplicationStore::new(),
        last_completed_analysis_input: None,
        next_analysis_signature_probe_at: None,
        analysis_retry_throttle: None,
        analysis_input_probe: None,
        upstream: None,
        explicit_target_plan: None,
    };

    let revisions = vec![workspace_a.name];
    let change_ids = vec!["change-a".to_string()];

    executor
        .merge_and_resolve_with(
            &revisions,
            &change_ids,
            |_revs, _details| async move { Ok(()) },
        )
        .await
        .or_fail("unexpected error");

    let hook_contents =
        std::fs::read_to_string(repo_root.join("hooked.txt")).or_fail("unexpected error");
    assert!(hook_contents.contains("hooked"));
}

#[tokio::test]
async fn test_execute_acceptance_in_workspace_emits_gate_specific_failure_log_context() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    // Create a workspace commit so acceptance diff context has a real delta target.
    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    let acceptance_output = "ACCEPTANCE: FAIL\n\nFINDINGS:\n- archive-readiness gate failed: cargo clippy -- -D warnings (src/lib.rs:42)\n- secondary finding\n";
    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(format!(
            "printf '{}'",
            acceptance_output.replace('\n', "\\n")
        )),
        archive_command: Some(
            "sh -c 'mkdir -p openspec/changes/archive && mv openspec/changes/change-a openspec/changes/archive/change-a && echo archive-ran > archive-ran.txt'"
                .to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        change_id,
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Fail { findings } => {
            assert_eq!(findings.len(), 2);
            assert!(
                findings
                    .iter()
                    .any(|f| f.contains("archive-readiness gate failed")),
                "expected archive-readiness finding in acceptance output"
            );
        }
        other => panic!("expected acceptance fail result, got {:?}", other),
    }

    let archive_result = execute_archive_in_workspace(
        change_id,
        repo_root.path(),
        acceptance_config
            .get_archive_command()
            .or_fail("unexpected error"),
        &acceptance_config,
        None,
        VcsBackend::Git,
        None,
        None,
        None,
        &ai_runner,
        &Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        &Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        &shared_stagger_state,
    )
    .await;

    let archive_commit = archive_result.or_fail("archive should run even after acceptance failure");
    assert!(
        !archive_commit.trim().is_empty(),
        "archive should return a merge commit hash when workspace-local routing allows archive"
    );

    assert!(
        repo_root.path().join("archive-ran.txt").exists(),
        "archive command should execute based on workspace-local routing"
    );
}

#[tokio::test]
async fn test_acceptance_fail_records_follow_up_tasks() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(
            "sh -c 'echo ACCEPTANCE: FAIL; echo; echo FINDINGS:; echo - missing regression test; echo - add repo coverage; echo - external non-mockable prerequisite unavailable'"
                .to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, iteration) = execute_acceptance_in_workspace(
        change_id,
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Fail { findings } => {
            assert_eq!(findings.len(), 3);
        }
        other => panic!("expected acceptance fail, got {:?}", other),
    }

    assert_eq!(
        agent
            .get_acceptance_follow_up(change_id)
            .or_fail("acceptance follow-up should be recorded")
            .1,
        ["missing regression test", "add repo coverage"]
    );

    crate::task_parser::replace_acceptance_follow_up_from_latest_fail(
        &tasks_dir.join("tasks.md"),
        iteration,
        &[
            "missing regression test".to_string().into(),
            "add repo coverage".to_string().into(),
        ],
    )
    .or_fail("unexpected error");

    let updated_tasks =
        std::fs::read_to_string(tasks_dir.join("tasks.md")).or_fail("unexpected error");
    assert!(updated_tasks.contains("## Current Acceptance Follow-up"));
    assert!(updated_tasks.contains("- attempt: 1"));
    assert!(updated_tasks.contains("- [ ] missing regression test"));
    assert!(updated_tasks.contains("- [ ] add repo coverage"));

    let progress = crate::task_parser::parse_file(&tasks_dir.join("tasks.md"), Some(change_id))
        .or_fail("unexpected error");
    assert_eq!(progress.completed, 1);
    assert_eq!(progress.total, 3);
}

/// Parallel and serial dispatch both route acceptance follow-up persistence
/// and PASS cleanup through the shared task-parser recovery path. This asserts
/// the two call sequences produce byte-identical files, so recovery, warning,
/// and cleanup behavior cannot diverge between execution modes.
#[test]
fn parallel_and_serial_follow_up_recovery_produce_identical_files() {
    const DRIFTED: &str = concat!(
        "## Implementation Tasks\n",
        "- [x] done\n",
        "\n",
        "## Current Acceptance Follow-up\n",
        "- attempt: 1\n",
        "- [x] earlier finding\n",
        "### Reviewer notes\n",
        "Free-form evidence with - [ ] checkbox text.\n",
    );
    let change_id = "parity-change";
    let findings = ["latest finding".to_string()];

    let seed = |root: &Path| {
        let change_dir = root.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
        std::fs::write(change_dir.join("tasks.md"), DRIFTED).or_fail("unexpected error");
    };

    let parallel_root = TempDir::new().or_fail("unexpected error");
    let serial_root = TempDir::new().or_fail("unexpected error");
    seed(parallel_root.path());
    seed(serial_root.path());

    let mut rendered = Vec::new();
    for root in [parallel_root.path(), serial_root.path()] {
        // FAIL persistence: identical resolver + shared recovery entry point.
        let tasks_path =
            crate::task_parser::resolve_acceptance_follow_up_tasks_path(change_id, root)
                .or_fail("follow-up path resolves");
        let recovery = crate::task_parser::replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &crate::acceptance::legacy_findings(findings.clone()),
        )
        .or_fail("recovery succeeds instead of terminating");
        assert_eq!(recovery.recovered_blocks, 1);
        assert!(recovery.warning().is_some());

        let after_fail = std::fs::read_to_string(&tasks_path).or_fail("unexpected error");
        assert!(after_fail.contains("## Recovered Acceptance Notes"));
        assert!(after_fail.contains("- [ ] latest finding"));
        assert_eq!(
            crate::task_parser::parse_content(&after_fail, None),
            crate::task_parser::TaskProgress::with_counts(1, 2)
        );

        // PASS cleanup: recovered notes survive, runtime section does not.
        let cleanup_path = crate::task_parser::resolve_acceptance_follow_up_tasks_path_for_cleanup(
            change_id, root,
        )
        .or_fail("cleanup path resolves")
        .or_fail("cleanup path exists");
        crate::task_parser::clear_acceptance_follow_up(&cleanup_path).or_fail("cleanup succeeds");

        let after_pass = std::fs::read_to_string(&cleanup_path).or_fail("unexpected error");
        assert!(!after_pass.contains("Acceptance Follow-up"));
        assert!(after_pass.contains("## Recovered Acceptance Notes"));
        rendered.push(after_pass);
    }

    assert_eq!(rendered[0], rendered[1]);
}

#[tokio::test]
async fn test_acceptance_history_records_end_revision_when_head_changes() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo acceptance-history-marker >> feature.rs; git add feature.rs; git commit -m acceptance-history-rev >/dev/null 2>&1; echo ACCEPTANCE: PASS'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        change_id,
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Pass => {}
        other => panic!("expected acceptance pass, got {:?}", other),
    }

    let end_revision = get_current_commit(repo_root.path())
        .await
        .or_fail("unexpected error");

    let recorded_revision = acceptance_history
        .lock()
        .await
        .last_commit_hash(change_id)
        .or_fail("acceptance history should store commit hash");
    assert_eq!(
        recorded_revision, end_revision,
        "acceptance history commit_hash should track end-of-acceptance HEAD"
    );
}

#[tokio::test]
async fn test_acceptance_diff_base_uses_last_acceptance_end_revision() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo acceptance-drift-marker >> feature.rs; git add feature.rs; git commit -m acceptance-drift-rev >/dev/null 2>&1; echo ACCEPTANCE: PASS'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        change_id,
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Pass => {}
        other => panic!("expected acceptance pass, got {:?}", other),
    }

    std::fs::write(
        repo_root.path().join("post_acceptance_only.rs"),
        "pub fn only_after() {}\n",
    )
    .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "post_acceptance_only.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Post acceptance fix"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let base_commit = acceptance_history
        .lock()
        .await
        .last_commit_hash(change_id)
        .or_fail("acceptance history should provide last commit hash");
    let current_commit = get_current_commit(repo_root.path())
        .await
        .or_fail("unexpected error");

    let changed_files = crate::vcs::git::commands::get_changed_files(
        repo_root.path(),
        Some(&base_commit),
        &current_commit,
    )
    .await
    .or_fail("expected changed files from last acceptance revision");

    assert!(
        changed_files.iter().any(|f| f == "post_acceptance_only.rs"),
        "diff from last acceptance revision should include files changed after acceptance"
    );
    assert!(
        !changed_files.iter().any(|f| f == "feature.rs"),
        "diff from last acceptance revision should exclude files changed during acceptance itself"
    );
}

#[tokio::test]
async fn test_archive_guard_allows_archive_after_acceptance_head_change_pass() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo post-pass-change >> feature.rs; git add feature.rs; git commit -m acceptance-pass-rev >/dev/null 2>&1; echo ACCEPTANCE: PASS'".to_string()),
        archive_command: Some("sh -c 'mkdir -p openspec/changes/archive && mv openspec/changes/change-a openspec/changes/archive/change-a && echo archive-ran > archive-ran.txt'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        change_id,
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Pass => {}
        other => panic!("expected acceptance pass, got {:?}", other),
    }

    execute_archive_in_workspace(
        change_id,
        repo_root.path(),
        acceptance_config
            .get_archive_command()
            .or_fail("unexpected error"),
        &acceptance_config,
        None,
        VcsBackend::Git,
        None,
        None,
        None,
        &ai_runner,
        &Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        &Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        &shared_stagger_state,
    )
    .await
    .or_fail("archive should pass when acceptance history records the final revision for handoff");

    assert!(
        repo_root.path().join("archive-ran.txt").exists(),
        "archive command should execute after acceptance pass with head change"
    );
}

#[tokio::test]
async fn test_dynamic_queue_injection() {
    use crate::tui::queue::DynamicQueue;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // Create a dynamic queue and add a change ID
    let queue = Arc::new(DynamicQueue::new());
    queue.push("test-change-2".to_string()).await;

    // Verify the queue has one item
    assert_eq!(queue.len().await, 1);

    // Create a simple parallel executor with the queue
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, _rx) = mpsc::channel(10);
    let mut executor = ParallelExecutor::new(repo_root, config, Some(tx));
    executor.set_dynamic_queue(queue.clone());

    // The queue reference should be set
    assert!(executor.dynamic_queue.is_some());

    // After this point, the execute_with_reanalysis method would poll the queue
    // and inject the change into the execution. This is tested via integration tests.
}

#[tokio::test]
async fn test_should_reanalyze_bypasses_debounce_on_slot_recovery() {
    use std::time::Instant;
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root, config, Some(tx));

    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(Instant::now());
    }

    assert!(
        executor.should_reanalyze(true).await,
        "explicit scheduler triggers should bypass debounce"
    );
    assert!(
        !executor.should_reanalyze(false).await,
        "regular queue edits should still respect debounce"
    );
}

#[tokio::test]
async fn test_queue_notification_with_fresh_debounce_starts_analysis_after_initial_iteration() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use std::time::Instant;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(Instant::now());
    }

    let mut queued = vec![make_test_change("fresh-queue-notification")];
    let mut in_flight = HashSet::new();
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 2,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("queue notification reanalysis should not fail");

    assert!(!should_break);
    assert_eq!(iteration, 3, "dispatch should advance after analysis");
    assert!(queued.is_empty());
    assert_eq!(in_flight.len(), 1);

    let mut saw_analysis_started = false;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::AnalysisStarted { attempt_id, .. } = event {
            saw_analysis_started = attempt_id.contains("iteration=2;trigger=queue;");
        }
    }
    assert!(
        saw_analysis_started,
        "fresh queue debounce must not suppress explicit queue notification analysis"
    );

    while join_set.join_next().await.is_some() {}
}

fn make_test_change(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

#[tokio::test]
async fn capacity_zero_dispatch_diagnostic_guard_suppresses_identical_keys_and_emits_changed_keys()
{
    let (tx, mut rx) = mpsc::channel(8);
    let mut executor = ParallelExecutor::new(
        PathBuf::from("/tmp/cflx-capacity-zero-dedup-unit"),
        create_test_config(),
        Some(tx),
    );
    let queued = vec![make_test_change("change-a")];
    let in_flight = HashSet::from(["active-apply".to_string()]);
    let order = vec!["change-a".to_string()];

    executor
        .emit_capacity_zero_dispatch_diagnostic_once(&queued, &in_flight, 1, &order)
        .await;
    executor
        .emit_capacity_zero_dispatch_diagnostic_once(&queued, &in_flight, 1, &order)
        .await;

    let changed_order = vec!["change-b".to_string()];
    executor
        .emit_capacity_zero_dispatch_diagnostic_once(&queued, &in_flight, 1, &changed_order)
        .await;
    let changed_parallelism = 2;
    executor
        .emit_capacity_zero_dispatch_diagnostic_once(
            &queued,
            &in_flight,
            changed_parallelism,
            &order,
        )
        .await;

    drop(executor);

    let mut diagnostics = Vec::new();
    while let Some(event) = rx.recv().await {
        if let ExecutionEvent::Log(entry) = event {
            if entry
                .message
                .contains("dispatch_capacity_zero_after_analysis")
            {
                diagnostics.push(entry.message);
            }
        }
    }

    assert_eq!(
        diagnostics.len(),
        3,
        "initial signature plus two changed signatures should emit exactly three diagnostics; saw {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("max_parallelism=2")),
        "changed max_parallelism should emit a fresh diagnostic; saw {diagnostics:?}"
    );
}

fn dependent_ready_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::from([("dependent".to_string(), vec!["resolving".to_string()])]),
            groups: None,
        }
        .into()
    })
}

fn ready_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::new(),
            groups: None,
        }
        .into()
    })
}

fn blocked_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    let dependencies = changes
        .iter()
        .map(|change| (change.id.clone(), vec!["unresolved-dependency".to_string()]))
        .collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies,
            groups: None,
        }
        .into()
    })
}

fn declared_dependency_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    let dependencies = changes
        .iter()
        .map(|change| (change.id.clone(), change.dependencies.clone()))
        .collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies,
            groups: None,
        }
        .into()
    })
}

fn selective_dependency_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    let dependencies = changes
        .iter()
        .map(|change| {
            if change.id == "change-b" {
                (change.id.clone(), vec!["unresolved-dependency".to_string()])
            } else {
                (change.id.clone(), Vec::new())
            }
        })
        .collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies,
            groups: None,
        }
        .into()
    })
}

#[tokio::test]
async fn test_blocked_only_classifier_distinguishes_scheduler_work_classes() {
    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    let mut dependency_blocked = make_test_change("dependency-blocked");
    dependency_blocked.dependencies = vec!["missing-dependency".to_string()];
    let queued = vec![
        make_test_change("dispatchable"),
        make_test_change("manual-merge"),
        make_test_change("resolve-wait"),
        make_test_change("terminal-error"),
        dependency_blocked,
    ];
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![
            "dispatchable".to_string(),
            "manual-merge".to_string(),
            "resolve-wait".to_string(),
            "terminal-error".to_string(),
            "dependency-blocked".to_string(),
            "candidate-missing".to_string(),
        ],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue("candidate-missing".to_string()));
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "manual-merge".to_string(),
            reason: "manual merge required".to_string(),
            auto_resumable: false,
        });
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "resolve-wait".to_string(),
            reason: "dirty base".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge("resolve-wait".to_string()));
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "terminal-error".to_string(),
            error: "boom".to_string(),
        });
    }

    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.set_shared_orchestrator_state(shared);
    let classification = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;

    assert_eq!(
        classification.class_for("dispatchable"),
        Some(crate::parallel::queue_state::QueuedWorkClass::DispatchableApply)
    );
    assert_eq!(
        classification.class_for("manual-merge"),
        Some(crate::parallel::queue_state::QueuedWorkClass::ManualMergeWait)
    );
    assert_eq!(
        classification.class_for("resolve-wait"),
        Some(crate::parallel::queue_state::QueuedWorkClass::SchedulerLaneWait)
    );
    assert_eq!(
        classification.class_for("terminal-error"),
        Some(crate::parallel::queue_state::QueuedWorkClass::TerminalErrorRetryRequired)
    );
    assert_eq!(
        classification.class_for("dependency-blocked"),
        Some(crate::parallel::queue_state::QueuedWorkClass::DependencyBlocked)
    );
    assert_eq!(
        classification.class_for("candidate-missing"),
        Some(crate::parallel::queue_state::QueuedWorkClass::CandidateUnavailable)
    );
}

/// Queue reconciliation must keep an actively retried missing-verdict change as
/// in-progress acceptance work and only defer it as
/// `terminal_error_retry_required` once the protocol budget is exhausted.
#[tokio::test]
async fn queue_reconciliation_defers_missing_verdict_only_after_retry_exhaustion() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    init_git_repo(repo_dir.path()).await;

    let change_id = "missing-verdict-queue";
    let queued = vec![make_test_change(change_id)];
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: change_id.to_string(),
            command: "apply".to_string(),
        });
    }

    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.set_shared_orchestrator_state(shared.clone());

    // While protocol-retry budget remains, no terminal error is reported, so
    // the change stays ordinary acceptance work.
    let during_retry = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;
    assert_ne!(
        during_retry.class_for(change_id),
        Some(crate::parallel::queue_state::QueuedWorkClass::TerminalErrorRetryRequired),
        "an actively retried missing verdict must not be deferred as a terminal error"
    );
    assert!(during_retry.terminal_error_retry_required.is_empty());

    // Exhaustion is the first point at which the runtime reports a terminal error.
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: change_id.to_string(),
            error: crate::orchestration::acceptance::missing_verdict_exhausted_error(
                3,
                2,
                &["Monitoring verification".to_string()],
            ),
        });
    }

    let after_exhaustion = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;
    assert_eq!(
        after_exhaustion.class_for(change_id),
        Some(crate::parallel::queue_state::QueuedWorkClass::TerminalErrorRetryRequired),
        "an exhausted missing verdict must reach the existing terminal-error deferral"
    );
}

#[tokio::test]
async fn test_blocked_only_resolve_wait_present() {
    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    let active_alpha = repo_dir.path().join("openspec/changes/alpha");
    std::fs::create_dir_all(&active_alpha).or_fail("create active alpha change dir");
    std::fs::write(active_alpha.join("proposal.md"), "# Alpha\n")
        .or_fail("write active alpha proposal");

    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.resolve_wait_changes.insert("alpha".to_string());

    let mut beta = make_test_change("beta");
    beta.dependencies = vec!["alpha".to_string()];
    let queued = vec![beta];
    let in_flight = HashSet::new();

    assert!(
        executor
            .classify_queued_work(&queued, &in_flight)
            .await
            .is_blocked_only(),
        "fixture must still be blocked-only at the queue-classification layer"
    );
    assert!(
        !executor
            .is_blocked_only_scheduler_state(&queued, &in_flight)
            .await,
        "local resolve_wait entries must keep the scheduler alive until resolve dispatch or completion"
    );

    executor.resolve_wait_changes.clear();
    executor.reject_wait_changes.insert("alpha".to_string());
    assert!(
        !executor
            .is_blocked_only_scheduler_state(&queued, &in_flight)
            .await,
        "local reject_wait entries must also prevent blocked-only drain"
    );
}

#[tokio::test]
async fn test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["manual-merge".to_string(), "terminal-error".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "manual-merge".to_string(),
            reason: "manual merge required".to_string(),
            auto_resumable: false,
        });
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "terminal-error".to_string(),
            error: "boom".to_string(),
        });
    }

    let analyzer_calls = Arc::new(AtomicUsize::new(0));
    static BLOCKED_ONLY_ANALYZER_CALLS: std::sync::OnceLock<Arc<AtomicUsize>> =
        std::sync::OnceLock::new();
    let _ = BLOCKED_ONLY_ANALYZER_CALLS.set(analyzer_calls.clone());
    fn should_not_call_analysis_result<'a>(
        changes: &'a [crate::openspec::Change],
        _in_flight: &'a [String],
        _iteration: u32,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
    > {
        if let Some(calls) = BLOCKED_ONLY_ANALYZER_CALLS.get() {
            calls.fetch_add(1, Ordering::SeqCst);
        }
        let order: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        Box::pin(async move {
            crate::analyzer::AnalysisResult {
                order,
                dependencies: HashMap::new(),
                groups: None,
            }
            .into()
        })
    }

    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.set_shared_orchestrator_state(shared);
    let mut queued = vec![
        make_test_change("manual-merge"),
        make_test_change("terminal-error"),
    ];
    let mut in_flight = HashSet::new();
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &should_not_call_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 1);
    assert_eq!(analyzer_calls.load(Ordering::SeqCst), 0);
    assert!(in_flight.is_empty());
    assert_eq!(queued.len(), 2);
    assert!(
        executor
            .is_blocked_only_scheduler_state(&queued, &in_flight)
            .await
    );
}

#[tokio::test]
async fn test_resolve_wait_completion_unblocks_dependents() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::{MergeResult, MergeResultOrigin, MergeTaskOutcome, WorkspaceResult};
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let alpha_dir = repo_dir.path().join("openspec/changes/alpha");
    std::fs::create_dir_all(&alpha_dir).or_fail("unexpected error");
    std::fs::write(alpha_dir.join("proposal.md"), "# Alpha\n").or_fail("unexpected error");
    let beta_dir = repo_dir.path().join("openspec/changes/beta");
    std::fs::create_dir_all(&beta_dir).or_fail("create beta change");
    std::fs::write(
        beta_dir.join("proposal.md"),
        "---\ndependencies:\n  - alpha\n---\n# Beta\n",
    )
    .or_fail("write beta proposal");
    std::fs::write(
        beta_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] apply\n",
    )
    .or_fail("write beta tasks");
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("stage fixture changes");
    Command::new("git")
        .args(["commit", "-m", "Add dependency fixtures"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("commit fixture changes");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor.resolve_wait_changes.insert("alpha".to_string());

    let mut beta = make_test_change("beta");
    beta.dependencies = vec!["alpha".to_string()];
    let mut queued = vec![beta];
    let mut in_flight = HashSet::new();
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );

    assert!(
        !executor
            .should_exit_when_idle(true, &queued, &in_flight)
            .await,
        "finite scheduler must not exit while a dependency blocker is waiting on resolve_wait completion"
    );
    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &beta_depends_on_alpha_analysis_result,
            semaphore: semaphore.clone(),
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("blocked pre-completion reanalysis should not fail");
    assert!(!should_break);
    assert_eq!(
        iteration, 1,
        "blocked-only beta must not dispatch before alpha resolves"
    );
    assert!(in_flight.is_empty());
    assert_eq!(queued.len(), 1);

    std::fs::remove_dir_all(&alpha_dir).or_fail("unexpected error");
    commit_archive_to_base(repo_dir.path(), "2026-05-13-alpha", "alpha").await;
    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);
    assert!(
        executor
            .handle_merge_result(MergeResult {
                change_id: "alpha".to_string(),
                workspace_name: "ws-alpha".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Ok(MergeTaskOutcome::Merged),
            })
            .await,
        "merged resolve_wait result should be treated as a successful base-lane completion"
    );
    assert!(
        executor.resolve_wait_changes.is_empty(),
        "merged resolve_wait result should clear local resolve_wait state"
    );

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration,
            reanalysis_reason: ReanalysisReason::ResolveCompletion,
            analyzer: &beta_depends_on_alpha_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("post-completion reanalysis should dispatch unblocked beta");

    assert!(!should_break);
    assert_eq!(
        iteration, 2,
        "dispatching beta should advance the scheduler iteration"
    );
    assert!(
        queued.is_empty(),
        "unblocked dependent should leave the queue"
    );
    assert!(
        in_flight.contains("beta"),
        "dependent beta should dispatch after alpha resolve_wait completion is merged"
    );

    while join_set.join_next().await.is_some() {}

    let mut saw_apply_started = false;
    while let Ok(Some(event)) =
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await
    {
        if matches!(
            event,
            ExecutionEvent::ApplyStarted { change_id, .. } if change_id == "beta"
        ) {
            saw_apply_started = true;
        }
    }
    assert!(
        saw_apply_started,
        "dependent must emit ApplyStarted only after merged resolve completion"
    );
}

#[tokio::test]
async fn test_analyze_failure_diagnostic_dedupes_by_signature() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16);
    let mut executor = ParallelExecutor::new(
        PathBuf::from("/tmp/test-repo"),
        create_test_config(),
        Some(tx),
    );
    let queued = vec![make_test_change("alpha")];
    let in_flight = HashSet::new();

    executor
        .emit_analysis_failure_diagnostic_once(&queued, &in_flight, "InstanceRef not provided")
        .await;
    executor
        .emit_analysis_failure_diagnostic_once(&queued, &in_flight, "InstanceRef not provided")
        .await;
    executor
        .emit_analysis_failure_diagnostic_once(&queued, &in_flight, "different failure")
        .await;

    let mut error_count = 0;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::Error { message } = event {
            if message.contains("Dependency analysis failed") {
                error_count += 1;
            }
        }
    }
    assert_eq!(
        error_count, 2,
        "stable analysis failure signature should emit once"
    );
}

fn single_queued_route_depends_on_policy_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
            groups: None,
        }
        .into()
    })
}

fn beta_depends_on_alpha_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::from([("beta".to_string(), vec!["alpha".to_string()])]),
            groups: None,
        }
        .into()
    })
}

fn dependency_on_inflight_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = crate::analyzer::AnalysisOutcome> + Send + 'a>,
> {
    let order = changes.iter().map(|change| change.id.clone()).collect();
    let dependency = in_flight
        .first()
        .cloned()
        .unwrap_or_else(|| "policy".to_string());
    let dependencies = changes
        .iter()
        .map(|change| {
            if change.id == "route" {
                (change.id.clone(), vec![dependency.clone()])
            } else {
                (change.id.clone(), Vec::new())
            }
        })
        .collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies,
            groups: None,
        }
        .into()
    })
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_apply_time_rejected_handoff_enters_rejecting_review_and_emits_change_rejected() {
    use crate::events::ExecutionEvent;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let change_id = "change-rejected";
    let change_dir = repo_dir
        .path()
        .join("openspec")
        .join("changes")
        .join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("proposal.md"),
        "---\nchange_type: implementation\n---\n# Change\n",
    )
    .or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] implement rejected flow\n",
    )
    .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Add change files"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some(
            "sh -c 'mkdir -p openspec/changes/{change_id}; printf "
                .to_string()
                + "\"# REJECTED\\n\\n- change_id: {change_id}\\n- reason: regression\\n\" > openspec/changes/{change_id}/REJECTED.md'",
        ),
        acceptance_command: Some("sh -c 'echo REJECTION_REVIEW: CONFIRM'".to_string()),
        ..Default::default()
    });

    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("unexpected error");

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("unexpected error");

    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    assert!(
        result.error.is_none(),
        "unexpected error: {:?}",
        result.error
    );
    assert!(
        result.rejected.is_some(),
        "rejected reason should be set after apply-time rejecting confirm"
    );
    assert!(
        result
            .rejected
            .as_deref()
            .unwrap_or_default()
            .contains("Rejecting review confirmed rejection"),
        "unexpected rejected reason: {:?}",
        result.rejected
    );

    let mut saw_rejecting_status = false;
    let mut saw_change_rejected = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::WorkspaceStatusUpdated {
                change_id: id,
                status,
                ..
            } if id == change_id && status == WorkspaceStatus::Rejecting => {
                saw_rejecting_status = true;
            }
            ExecutionEvent::ChangeRejected { change_id: id, .. } if id == change_id => {
                saw_change_rejected = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_rejecting_status,
        "apply-time rejected handoff must emit WorkspaceStatus::Rejecting"
    );
    assert!(
        saw_change_rejected,
        "confirmed rejecting review must emit ChangeRejected"
    );
}

#[tokio::test]
async fn dynamic_queue_ingestion_skips_final_terminal_merged_change() {
    use crate::tui::queue::DynamicQueue;

    let queue = Arc::new(DynamicQueue::new());
    queue.push("alpha".to_string()).await;

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "alpha".to_string(),
            revision: "rev".to_string(),
        });

    let mut executor =
        ParallelExecutor::new(PathBuf::from("/tmp/test-repo"), create_test_config(), None);
    executor.set_dynamic_queue(queue);
    executor.set_shared_orchestrator_state(shared);

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reason = crate::parallel::dynamic_queue::ReanalysisReason::Initial;

    let changed = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reason)
        .await;

    assert!(!changed);
    assert!(queued.is_empty());
    assert_eq!(
        reason,
        crate::parallel::dynamic_queue::ReanalysisReason::Initial
    );
}

#[tokio::test]
async fn final_terminal_dispatch_preflight_skips_before_workspace_execution() {
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    shared
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "alpha".to_string(),
            revision: "rev".to_string(),
        });

    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.set_shared_orchestrator_state(shared);
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            "alpha".to_string(),
            "base".to_string(),
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch should skip cleanly");

    assert!(in_flight.is_empty());
    assert!(join_set.join_next().await.is_none());
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::ArchiveStarted { ref change_id, .. }
            | ExecutionEvent::ApplyStarted { ref change_id, .. }
            | ExecutionEvent::AcceptanceStarted { ref change_id, .. }
                if change_id == "alpha" =>
            {
                panic!("final terminal dispatch must not emit execution event: {event:?}");
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_dependency_blocked_event_is_emitted_even_when_slots_are_full() {
    use crate::events::ExecutionEvent;
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("change-a"), make_test_change("change-b")];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &selective_dependency_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 2);
    assert_eq!(in_flight.len(), 1, "ready change should consume the slot");

    let mut saw_blocked_event = false;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::DependencyBlocked {
            change_id,
            dependency_ids,
        } = event
        {
            if change_id == "change-b" {
                assert_eq!(dependency_ids, vec!["unresolved-dependency".to_string()]);
                saw_blocked_event = true;
            }
        }
    }

    assert!(
        saw_blocked_event,
        "dependency-blocked event must be emitted even if available slots are already consumed"
    );

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_single_queued_active_not_queued_dependency_blocks_dispatch_selection() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["policy"]);
    let policy_dir = repo_dir.path().join("openspec/changes/policy");
    std::fs::create_dir_all(&policy_dir).or_fail("unexpected error");
    std::fs::write(policy_dir.join("proposal.md"), "# Policy\n").or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert!(
        selected.is_empty(),
        "route must not dispatch while policy is active but not queued"
    );
    let dependency_events = drain_dependency_events(&mut rx, "route");
    assert_eq!(dependency_events, vec!["blocked:policy".to_string()]);
}

#[tokio::test]
async fn test_single_queued_archived_dependency_waits_until_merged() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["policy"]);
    let archived_dir = repo_dir
        .path()
        .join("openspec/changes/archive/2026-05-13-policy");
    std::fs::create_dir_all(&archived_dir).or_fail("unexpected error");
    std::fs::write(archived_dir.join("proposal.md"), "# Policy\n").or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert!(
        selected.is_empty(),
        "archived dependency must wait for base-branch merge evidence"
    );
    assert_eq!(
        drain_dependency_events(&mut rx, "route"),
        vec!["blocked:policy".to_string()],
        "archived-but-not-merged dependency should emit one dependency-blocked event"
    );
}

#[tokio::test]
async fn test_single_queued_archived_dependency_can_dispatch_after_merge() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["policy"]);
    commit_archive_to_base(repo_dir.path(), "2026-05-13-policy", "policy").await;

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert_eq!(selected, vec!["route".to_string()]);
    assert!(
        drain_dependency_events(&mut rx, "route").is_empty(),
        "merged archived dependency should be satisfied without dependency-blocked events"
    );
}

#[tokio::test]
async fn test_archived_dependency_uses_effective_integration_base_after_startup() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["policy"]);

    let archived_dir = repo_dir
        .path()
        .join("openspec/changes/archive/2026-05-13-policy");
    std::fs::create_dir_all(&archived_dir).or_fail("unexpected error");
    std::fs::write(archived_dir.join("proposal.md"), "# Archived policy\n")
        .or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let initially_blocked = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;
    assert!(
        initially_blocked.is_empty(),
        "archived-but-not-merged dependency should capture startup branch and remain blocked"
    );
    assert_eq!(
        drain_dependency_events(&mut rx, "route"),
        vec!["blocked:policy".to_string()]
    );

    let startup_branch_has_archive =
        crate::execution::state::is_merged_to_base("policy", repo_dir.path(), "main")
            .await
            .or_fail("unexpected error");
    assert!(
        !startup_branch_has_archive,
        "test fixture must prove the original startup branch lacks the archive merge before integration advances"
    );

    Command::new("git")
        .args(["checkout", "-b", "integration"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Archive policy on integration"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("unexpected error");

    let integration_has_archive =
        crate::execution::state::is_merged_to_base("policy", repo_dir.path(), "integration")
            .await
            .or_fail("unexpected error");
    assert!(
        integration_has_archive,
        "test fixture must prove the effective integration base contains the archive merge"
    );

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert_eq!(
        selected,
        vec!["route".to_string()],
        "archived dependency merged into the effective integration base should unblock dispatch even when startup branch is unchanged"
    );
    assert_eq!(
        drain_dependency_events(&mut rx, "route"),
        vec!["resolved".to_string()],
        "effective-base merge should resolve the previous archived dependency blocker"
    );

    let main_has_archive =
        crate::execution::state::is_merged_to_base("policy", repo_dir.path(), "main")
            .await
            .or_fail("unexpected error");
    assert!(
        !main_has_archive,
        "test fixture must prove the original startup branch still lacks the archive merge"
    );
}

#[tokio::test]
async fn dependency_resolving_dependents_wait_until_merged() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "change-b", &["change-a"]);
    write_change_proposal(repo_dir.path(), "change-c", &["change-a"]);
    let archived_dir = repo_dir
        .path()
        .join("openspec/changes/archive/2026-05-13-change-a");
    std::fs::create_dir_all(&archived_dir).or_fail("unexpected error");
    std::fs::write(archived_dir.join("proposal.md"), "# Change A\n").or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["change-b".to_string(), "change-c".to_string()],
        dependencies: HashMap::from([
            ("change-b".to_string(), vec!["change-a".to_string()]),
            ("change-c".to_string(), vec!["change-a".to_string()]),
        ]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 2, &in_flight)
        .await;

    assert!(
        selected.is_empty(),
        "B/C dependents must not dispatch while A is archived locally but not merged to base"
    );
    let mut blocked_events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::DependencyBlocked {
            change_id,
            dependency_ids,
        } = event
        {
            blocked_events.push(format!("{change_id}:{}", dependency_ids.join(",")));
        }
    }
    blocked_events.sort();
    assert_eq!(
        blocked_events,
        vec![
            "change-b:change-a".to_string(),
            "change-c:change-a".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_single_queued_dependency_block_classes_fail_closed() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    for dep_id in ["ghost", "rejected-policy", "inflight-policy"] {
        write_change_proposal(repo_dir.path(), &format!("route-{dep_id}"), &[dep_id]);
    }
    let rejected_dir = repo_dir.path().join("openspec/changes/rejected-policy");
    std::fs::create_dir_all(&rejected_dir).or_fail("unexpected error");
    std::fs::write(rejected_dir.join("proposal.md"), "# Rejected Policy\n")
        .or_fail("unexpected error");
    std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED\n").or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let in_flight = HashSet::from(["inflight-policy".to_string()]);

    for dep_id in ["ghost", "rejected-policy", "inflight-policy"] {
        let analysis_result = crate::analyzer::AnalysisResult {
            order: vec![format!("route-{dep_id}")],
            dependencies: HashMap::from([(format!("route-{dep_id}"), vec![dep_id.to_string()])]),
            groups: None,
        };
        let selected = executor
            .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
            .await;
        assert!(
            selected.is_empty(),
            "dependency {dep_id} must fail closed before dispatch"
        );
    }

    let mut blocked_events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::DependencyBlocked {
            change_id,
            dependency_ids,
        } = event
        {
            blocked_events.push(format!("{change_id}:{}", dependency_ids.join(",")));
        }
    }
    blocked_events.sort();
    assert_eq!(
        blocked_events,
        vec![
            "route-ghost:ghost".to_string(),
            "route-inflight-policy:inflight-policy".to_string(),
            "route-rejected-policy:rejected-policy".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_single_queued_active_dependency_does_not_emit_apply_started() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["policy"]);
    let policy_dir = repo_dir.path().join("openspec/changes/policy");
    std::fs::create_dir_all(&policy_dir).or_fail("unexpected error");
    std::fs::write(policy_dir.join("proposal.md"), "# Policy\n").or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("route")];
    let mut in_flight = HashSet::new();

    let (_should_break, _iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &single_queued_route_depends_on_policy_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !in_flight.contains("route"),
        "route must not enter in-flight while policy is active but not queued"
    );

    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::ApplyStarted { change_id, .. } if change_id == "route" => {
                panic!("route must not emit ApplyStarted before policy resolves")
            }
            ExecutionEvent::ProcessingStarted(change_id) if change_id == "route" => {
                panic!("route must not emit ProcessingStarted before policy resolves")
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_inflight_dependency_blocks_dispatch_until_resolved() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("route")];
    let mut in_flight = HashSet::from(["policy".to_string()]);

    let (_should_break, _iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 2,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &dependency_on_inflight_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !in_flight.contains("route"),
        "route must not dispatch while policy is in-flight and unmerged"
    );

    let mut saw_blocked = false;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::DependencyBlocked {
            change_id,
            dependency_ids,
        } = event
        {
            if change_id == "route" && dependency_ids == vec!["policy".to_string()] {
                saw_blocked = true;
            }
        }
    }
    assert!(
        saw_blocked,
        "in-flight dependency should emit DependencyBlocked"
    );
}

#[tokio::test]
async fn test_dependency_blocked_event_emits_once_for_unchanged_snapshot() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["ghost".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let first_selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;
    let second_selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert!(first_selected.is_empty());
    assert!(second_selected.is_empty());
    let blocked_events = drain_dependency_events(&mut rx, "route");
    assert_eq!(
        blocked_events,
        vec!["blocked:ghost".to_string()],
        "unchanged blocker fingerprint should emit one DependencyBlocked event"
    );
}

#[tokio::test]
async fn test_changed_dependency_blocker_snapshot_emits_again() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let in_flight = HashSet::new();
    let missing_analysis = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["ghost".to_string()])]),
        groups: None,
    };
    let queued_analysis = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string(), "policy".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };

    let missing_selected = executor
        .select_changes_for_dispatch(&missing_analysis, 1, &in_flight)
        .await;
    let queued_selected = executor
        .select_changes_for_dispatch(&queued_analysis, 1, &in_flight)
        .await;

    assert!(missing_selected.is_empty());
    assert_eq!(queued_selected, vec!["policy".to_string()]);
    let blocked_events = drain_dependency_events(&mut rx, "route");
    assert_eq!(
        blocked_events,
        vec!["blocked:ghost".to_string(), "blocked:policy".to_string()],
        "changed blocker fingerprint should emit another DependencyBlocked event"
    );
}

#[tokio::test]
async fn test_dependency_resolved_emits_once_and_blocked_again_can_emit() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let in_flight = HashSet::new();
    let blocked_analysis = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["ghost".to_string()])]),
        groups: None,
    };
    let ready_analysis = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::new(),
        groups: None,
    };

    let first_blocked = executor
        .select_changes_for_dispatch(&blocked_analysis, 1, &in_flight)
        .await;
    let first_ready = executor
        .select_changes_for_dispatch(&ready_analysis, 1, &in_flight)
        .await;
    let second_ready = executor
        .select_changes_for_dispatch(&ready_analysis, 1, &in_flight)
        .await;
    let second_blocked = executor
        .select_changes_for_dispatch(&blocked_analysis, 1, &in_flight)
        .await;

    assert!(first_blocked.is_empty());
    assert_eq!(first_ready, vec!["route".to_string()]);
    assert_eq!(second_ready, vec!["route".to_string()]);
    assert!(second_blocked.is_empty());
    let dependency_events = drain_dependency_events(&mut rx, "route");
    assert_eq!(
        dependency_events,
        vec![
            "blocked:ghost".to_string(),
            "resolved".to_string(),
            "blocked:ghost".to_string(),
        ],
        "resolved transition should emit once and a later blocked transition should emit again"
    );
}

#[tokio::test]
async fn test_dependency_suppression_state_does_not_change_dispatch_selection() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let in_flight = HashSet::new();
    let blocked_analysis = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string(), "policy".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["policy".to_string()])]),
        groups: None,
    };

    let before = executor
        .select_changes_for_dispatch(&blocked_analysis, 2, &in_flight)
        .await;
    assert_eq!(before, vec!["policy".to_string()]);
    assert!(
        executor
            .dependency_blocker_fingerprints
            .contains_key("route"),
        "diagnostic fingerprint should be stored only after deriving blockers"
    );

    let after = executor
        .select_changes_for_dispatch(&blocked_analysis, 2, &in_flight)
        .await;
    assert_eq!(
        after,
        vec!["policy".to_string()],
        "in-memory diagnostic suppression must not alter dispatch selection"
    );
}

fn drain_dependency_events(
    rx: &mut tokio::sync::mpsc::Receiver<ExecutionEvent>,
    target_change_id: &str,
) -> Vec<String> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::DependencyBlocked {
                change_id,
                dependency_ids,
            } if change_id == target_change_id => {
                events.push(format!("blocked:{}", dependency_ids.join(",")));
            }
            ExecutionEvent::DependencyResolved { change_id } if change_id == target_change_id => {
                events.push("resolved".to_string());
            }
            _ => {}
        }
    }
    events
}

#[tokio::test]
async fn test_archived_dependency_is_blocked_without_rejection_until_merged() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;
    write_change_proposal(repo_dir.path(), "route", &["contracts"]);
    let archived_dir = repo_dir
        .path()
        .join("openspec")
        .join("changes")
        .join("archive")
        .join("2026-04-29-contracts");
    std::fs::create_dir_all(&archived_dir).or_fail("unexpected error");
    std::fs::write(archived_dir.join("proposal.md"), "# Archived").or_fail("unexpected error");

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["contracts".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert!(selected.is_empty());
    let dependency_events = drain_dependency_events(&mut rx, "route");
    assert_eq!(dependency_events, vec!["blocked:contracts".to_string()]);
    while let Ok(event) = rx.try_recv() {
        assert!(
            !matches!(event, ExecutionEvent::ChangeRejected { .. }),
            "archived dependency must not emit ChangeRejected"
        );
    }
}

#[tokio::test]
async fn test_missing_dependency_fails_closed_without_dispatch() {
    let repo_dir = tempfile::TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let analysis_result = crate::analyzer::AnalysisResult {
        order: vec!["route".to_string()],
        dependencies: HashMap::from([("route".to_string(), vec!["ghost".to_string()])]),
        groups: None,
    };
    let in_flight = HashSet::new();

    let selected = executor
        .select_changes_for_dispatch(&analysis_result, 1, &in_flight)
        .await;

    assert!(selected.is_empty(), "missing dependency must not dispatch");
    let mut saw_missing_diagnostic = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::Error { message } if message.contains("missing dependency 'ghost'") => {
                saw_missing_diagnostic = true;
            }
            ExecutionEvent::ChangeRejected { .. } => {
                panic!("missing dependency should fail closed without ChangeRejected")
            }
            _ => {}
        }
    }
    assert!(
        saw_missing_diagnostic,
        "missing dependency diagnostic should be emitted"
    );
}

#[tokio::test]
async fn test_slot_release_reanalyzes_and_dispatches_queued_follow_up_changes() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let manual_resolve_counter = Arc::new(AtomicUsize::new(1));
    executor.set_manual_resolve_counter(manual_resolve_counter.clone());

    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![
        make_test_change("follow-up-a"),
        make_test_change("follow-up-b"),
    ];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 2,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &ready_analysis_result,
            semaphore: semaphore.clone(),
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !should_break,
        "scheduler should keep running while resolve holds the slot"
    );
    assert_eq!(
        iteration, 2,
        "no dispatch should happen while available slots are zero"
    );
    assert_eq!(
        queued.len(),
        2,
        "queued follow-up changes should remain queued"
    );
    assert!(
        in_flight.is_empty(),
        "nothing should dispatch before the slot is released"
    );

    manual_resolve_counter.store(0, Ordering::SeqCst);

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !should_break,
        "scheduler should continue after dispatching resumed queued work"
    );
    assert_eq!(
        iteration, 3,
        "dispatch should advance the scheduler iteration"
    );
    assert_eq!(
        queued.len(),
        1,
        "one follow-up change should dispatch immediately after slot recovery"
    );
    assert_eq!(
        in_flight.len(),
        1,
        "slot recovery should move a queued follow-up change into flight"
    );

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_resolve_wait_does_not_block_queue_reanalysis_dispatch() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor
        .resolve_wait_changes
        .insert("still-resolving".to_string());

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("queued-during-resolve-wait")];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 2);
    assert!(queued.is_empty());
    assert_eq!(in_flight.len(), 1);

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_resolving_with_free_slot_still_dispatches_queued_change() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    // gamma has scheduler-visible retry intent, while alpha is already consuming one slot.
    executor
        .resolve_wait_changes
        .insert("gamma-merge-wait".to_string());

    let semaphore = Arc::new(Semaphore::new(2));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("beta-queued")];
    let mut in_flight = HashSet::from(["alpha-resolving".to_string()]);

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 2,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 2, "free slot should advance scheduler iteration");
    assert!(queued.is_empty(), "queued change should be dispatched");
    assert_eq!(
        in_flight.len(),
        2,
        "queued change should dispatch even when another change is resolving"
    );
    assert!(
        in_flight.contains("beta-queued"),
        "beta must become in-flight when slot is available"
    );

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_dispatch_zero_reanalysis_is_retried_on_next_loop() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("queued-after-zero-dispatch")];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 1,
            reanalysis_reason: ReanalysisReason::QueueNotification,
            analyzer: &blocked_analysis_result,
            semaphore: semaphore.clone(),
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 1, "dispatch 0件では iteration は進まない");
    assert_eq!(queued.len(), 1, "dispatchできない change はキューに残る");
    assert!(in_flight.is_empty());

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration,
            reanalysis_reason: ReanalysisReason::ResolveCompletion,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(
        iteration, 2,
        "次ループ再評価で dispatch され iteration が進む"
    );
    assert!(queued.is_empty(), "再評価後にキューが消化される");
    assert_eq!(in_flight.len(), 1, "再評価後に change が in-flight になる");

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_resolve_completion_reanalysis_bypasses_debounce_and_dispatches_work() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("follow-up-after-resolve")];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 2,
            reanalysis_reason: ReanalysisReason::ResolveCompletion,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !should_break,
        "resolve completion should resume the scheduler instead of terminating it"
    );
    assert_eq!(
        iteration, 3,
        "resolve completion should immediately trigger a dispatch iteration"
    );
    assert!(
        queued.is_empty(),
        "resolve completion should dispatch queued work without waiting for debounce"
    );
    assert_eq!(
        in_flight.len(),
        1,
        "queued work should become in-flight after resolve completion"
    );

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_repair_candidate_reanalysis_bypasses_debounce_and_dispatches_work() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::parallel::WorkspaceResult;
    use crate::vcs::VcsBackend;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    let workspace_base = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut queued = vec![make_test_change("repair-after-archive")];
    let mut in_flight = HashSet::new();

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(ReanalysisDispatchContext {
            queued: &mut queued,
            in_flight: &mut in_flight,
            max_parallelism: 1,
            iteration: 2,
            reanalysis_reason: ReanalysisReason::RepairCandidate,
            analyzer: &ready_analysis_result,
            semaphore,
            join_set: &mut join_set,
            cleanup_guard: &mut cleanup_guard,
        })
        .await
        .or_fail("unexpected error");

    assert!(
        !should_break,
        "repair candidate should resume the scheduler instead of terminating it"
    );
    assert_eq!(
        iteration, 3,
        "repair candidate should immediately trigger a dispatch iteration"
    );
    assert!(
        queued.is_empty(),
        "repair candidate should dispatch queued work without waiting for queue debounce"
    );
    assert_eq!(
        in_flight.len(),
        1,
        "repair candidate should become in-flight after repair-triggered analysis"
    );

    while join_set.join_next().await.is_some() {}
}

#[tokio::test]
async fn test_rejected_workspace_completion_retries_deferred_merges() {
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor
        .resolve_wait_changes
        .insert("blocked-change".to_string());

    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);
    let mut in_flight = HashSet::new();
    in_flight.insert("blocked-change".to_string());

    executor
        .handle_workspace_completion(
            WorkspaceResult {
                change_id: "blocked-change".to_string(),
                workspace_name: "ws-blocked-change".to_string(),
                final_revision: None,
                error: None,
                rejected: Some("confirmed rejection".to_string()),
            },
            1,
            &mut in_flight,
            &merge_result_tx,
        )
        .await;

    assert!(
        executor.resolve_wait_changes.is_empty(),
        "rejecting completion must trigger deferred-merge retry and clear orphaned resolve-wait entries"
    );
}

#[tokio::test]
async fn test_rejection_review_failure_retries_deferred_merges() {
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor
        .resolve_wait_changes
        .insert("blocked-change".to_string());

    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);
    let mut in_flight = HashSet::new();
    in_flight.insert("blocked-change".to_string());

    executor
        .handle_workspace_completion(
            WorkspaceResult {
                change_id: "blocked-change".to_string(),
                workspace_name: "ws-blocked-change".to_string(),
                final_revision: None,
                error: Some("rejecting review failed".to_string()),
                rejected: None,
            },
            1,
            &mut in_flight,
            &merge_result_tx,
        )
        .await;

    assert!(
        executor.resolve_wait_changes.is_empty(),
        "rejecting failure must trigger deferred-merge retry and clear orphaned resolve-wait entries"
    );
}

#[tokio::test]
async fn test_handle_merge_result_keeps_pending_counter_non_negative() {
    use crate::parallel::{MergeResult, MergeResultOrigin, MergeTaskOutcome};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.store(2, Ordering::Relaxed);
    assert!(
        executor
            .handle_merge_result(MergeResult {
                change_id: "change-ok".to_string(),
                workspace_name: "ws-change-ok".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: Ok(MergeTaskOutcome::Merged),
            })
            .await
    );
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 1);

    assert!(
        !executor
            .handle_merge_result(MergeResult {
                change_id: "change-err".to_string(),
                workspace_name: "ws-change-err".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: Err("merge failed".to_string()),
            })
            .await
    );
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn fix_scheduler_premature_exit_decrements_pending_merge_counter_on_merge_completion() {
    use crate::parallel::{MergeResult, MergeResultOrigin, MergeTaskOutcome};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);

    let merged = executor
        .handle_merge_result(MergeResult {
            change_id: "change-ok".to_string(),
            workspace_name: "ws-change-ok".to_string(),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome: Ok(MergeTaskOutcome::Merged),
        })
        .await;

    assert!(
        merged,
        "actual merged background outcomes must trigger success-only scheduler follow-up"
    );
    assert_eq!(
        executor.pending_merge_count.load(Ordering::Relaxed),
        0,
        "scheduler must clear pending merge counter after merge result is handled"
    );
}

#[tokio::test]
async fn test_handle_merge_result_deferred_is_not_successful_completion() {
    use crate::parallel::{MergeResult, MergeResultOrigin, MergeTaskOutcome};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);
    executor.resolve_wait_changes.insert("beta".to_string());

    let merged = executor
        .handle_merge_result(MergeResult {
            change_id: "alpha".to_string(),
            workspace_name: "ws-alpha".to_string(),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome: Ok(MergeTaskOutcome::deferred(
                "archive verification incomplete",
                false,
            )),
        })
        .await;

    assert!(
        !merged,
        "deferred background merge outcomes must not be reported as completed merges"
    );
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 0);
    assert!(
        executor.resolve_wait_changes.contains("beta"),
        "deferred merge result handling must not run success-only retry_deferred_base_lane_waiters"
    );
}

#[tokio::test]
async fn test_handle_merge_result_failed_emits_error_event_with_context() {
    use crate::parallel::{MergeResult, MergeResultOrigin};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);

    let merged = executor
        .handle_merge_result(MergeResult {
            change_id: "alpha".to_string(),
            workspace_name: "ws-alpha".to_string(),
            origin: MergeResultOrigin::PostArchiveMerge,
            outcome: Err("merge failed hard".to_string()),
        })
        .await;

    assert!(!merged, "failed merge outcomes are not successful merges");
    let event = rx.try_recv().or_fail("expected merge failure event");
    match event {
        ExecutionEvent::Error { message } => {
            assert!(message.contains("alpha"), "missing change id: {message}");
            assert!(
                message.contains("ws-alpha"),
                "missing workspace name: {message}"
            );
            assert!(
                message.contains("merge failed hard"),
                "missing error: {message}"
            );
        }
        other => panic!("expected error event, got {other:?}"),
    }
}

#[tokio::test]
async fn test_scheduler_lifetime_controls_idle_exit_behavior() {
    use tempfile::TempDir;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let mut finite_executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), config.clone(), None);

    let queued = Vec::new();
    let in_flight = HashSet::new();
    assert!(
        finite_executor
            .should_exit_when_idle(true, &queued, &in_flight)
            .await,
        "finite scheduler must exit when all work is drained"
    );

    finite_executor.set_persistent_lifetime();
    assert!(
        !finite_executor
            .should_exit_when_idle(true, &queued, &in_flight)
            .await,
        "persistent scheduler must remain alive while idle"
    );

    assert!(
        !finite_executor
            .should_exit_when_idle(false, &queued, &in_flight)
            .await,
        "scheduler must not exit when active join tasks remain"
    );
}

#[tokio::test]
async fn test_persistent_idle_wait_detection_requires_fully_drained_state() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    let queued = Vec::new();
    let in_flight = HashSet::new();
    assert!(
        !executor
            .should_enter_persistent_idle_wait(true, &queued, &in_flight)
            .await,
        "finite scheduler must not enter persistent idle wait"
    );

    executor.set_persistent_lifetime();
    assert!(
        executor
            .should_enter_persistent_idle_wait(true, &queued, &in_flight)
            .await,
        "persistent scheduler should enter event-driven idle wait only when fully drained"
    );
    assert!(
        !executor
            .should_enter_persistent_idle_wait(false, &queued, &in_flight)
            .await,
        "active join tasks must keep the scheduler on the normal event path"
    );
    let queued_work = vec![make_test_change("queued-work")];
    assert!(
        !executor
            .should_enter_persistent_idle_wait(true, &queued_work, &in_flight)
            .await,
        "dispatchable queued work must keep debounce/reanalysis behavior active"
    );
    let in_flight_work = HashSet::from(["in-flight-work".to_string()]);
    assert!(
        !executor
            .should_enter_persistent_idle_wait(true, &queued, &in_flight_work)
            .await,
        "in-flight work must keep completion handling active"
    );

    executor
        .resolve_wait_changes
        .insert("needs-resolve".to_string());
    assert!(
        executor
            .should_enter_persistent_idle_wait(true, &queued, &in_flight)
            .await,
        "stable ResolveWait-only work should use event-driven persistent idle wait"
    );
}

#[tokio::test]
async fn test_persistent_idle_wait_does_not_poll_worktree_reconciliation_without_wake() {
    use crate::tui::queue::DynamicQueue;
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let queue = Arc::new(DynamicQueue::new());
    let scan_calls = Arc::new(AtomicUsize::new(0));
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    executor.workspace_manager = Box::new(TestWorkspaceManager::new(scan_calls.clone()));
    executor.set_persistent_lifetime();
    executor.set_dynamic_queue(queue);
    let (_merge_result_tx, mut merge_result_rx) = mpsc::channel(1);
    let mut reason = crate::parallel::dynamic_queue::ReanalysisReason::Initial;

    let wait = executor.wait_for_persistent_idle_wake(&mut reason, &mut merge_result_rx);
    tokio::pin!(wait);

    tokio::select! {
        _ = &mut wait => panic!("persistent idle wait should not complete without a wake event"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
    }

    assert_eq!(
        scan_calls.load(Ordering::SeqCst),
        0,
        "event-driven persistent idle wait must not poll worktree reconciliation"
    );
}

#[tokio::test]
async fn test_persistent_idle_wait_wakes_on_queue_push_and_notify_scheduler() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::tui::queue::DynamicQueue;
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let queue = Arc::new(DynamicQueue::new());
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    executor.set_persistent_lifetime();
    executor.set_dynamic_queue(queue.clone());
    let (_merge_result_tx, mut merge_result_rx) = mpsc::channel(1);

    let mut reason = ReanalysisReason::Initial;
    {
        let wait = executor.wait_for_persistent_idle_wake(&mut reason, &mut merge_result_rx);
        tokio::pin!(wait);

        tokio::select! {
            _ = &mut wait => panic!("persistent idle wait should not complete before a wake event"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}
        }

        assert!(queue.push("queued-change".to_string()).await);
        tokio::time::timeout(std::time::Duration::from_secs(1), &mut wait)
            .await
            .or_fail("queue push should wake persistent idle wait");
    }
    assert_eq!(reason.to_string(), "queue");

    let mut reason = ReanalysisReason::Initial;
    {
        let wait = executor.wait_for_persistent_idle_wake(&mut reason, &mut merge_result_rx);
        tokio::pin!(wait);

        tokio::select! {
            _ = &mut wait => panic!("persistent idle wait should not complete before scheduler notification"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {}
        }

        queue.notify_scheduler();
        tokio::time::timeout(std::time::Duration::from_secs(1), &mut wait)
            .await
            .or_fail("notify_scheduler should wake persistent idle wait");
    }
    assert_eq!(reason.to_string(), "queue");
}

#[tokio::test]
async fn test_idle_queue_addition_marks_reanalysis_and_enqueues_change() {
    use crate::parallel::dynamic_queue::ReanalysisReason;
    use crate::tui::queue::DynamicQueue;

    let config = create_test_config();
    let mut executor = ParallelExecutor::new(PathBuf::from("."), config, None);
    executor.set_persistent_lifetime();

    let all_changes = crate::openspec::list_changes_native().unwrap_or_default();
    if all_changes.is_empty() {
        return;
    }

    let preferred_change_id = "refactor-git-sync-log-boilerplate";
    let change_id = all_changes
        .iter()
        .find(|change| change.id == preferred_change_id)
        .map(|change| change.id.clone())
        .or_else(|| all_changes.first().map(|change| change.id.clone()))
        .or_fail("expected at least one change");

    let dynamic_queue = Arc::new(DynamicQueue::new());
    dynamic_queue.push(change_id.to_string()).await;
    executor.set_dynamic_queue(dynamic_queue);

    let mut queued = Vec::new();
    let in_flight = HashSet::new();
    let mut reason = ReanalysisReason::Initial;

    let queue_changed = executor
        .check_dynamic_queue_and_add_changes(&mut queued, &in_flight, &mut reason)
        .await;

    assert!(
        queue_changed,
        "dynamic queue additions should trigger reanalysis"
    );
    assert_eq!(reason.to_string(), "queue");
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, change_id);
}

#[tokio::test]
async fn test_debounce_with_queue_changes() {
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root, config, Some(tx));

    // First check: no queue changes, should reanalyze
    assert!(executor.should_reanalyze(false).await);

    // Simulate a queue change
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(Instant::now());
    }

    // Immediate check: should NOT reanalyze (debounce active)
    assert!(!executor.should_reanalyze(false).await);

    // Simulate debounce period expiry without wall-clock waiting.
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(Instant::now() - Duration::from_secs(11));
    }

    // After simulated debounce expiry: should reanalyze
    assert!(executor.should_reanalyze(false).await);
}

#[tokio::test]
async fn test_queue_notification_triggers_reanalysis() {
    use crate::tui::queue::DynamicQueue;
    use std::sync::Arc;
    use std::time::Duration;

    // Create a dynamic queue
    let queue = Arc::new(DynamicQueue::new());

    // Spawn a task that waits for notification
    let queue_clone = queue.clone();
    let handle = tokio::spawn(async move {
        let notified = queue_clone.notified();

        // Wait for notification with timeout
        tokio::select! {
            _ = notified => {
                // Notification received
                Ok(())
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                // Timeout - notification not received
                Err("Timeout waiting for notification")
            }
        }
    });

    // Give the task time to start waiting
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Push to queue (should trigger notification)
    queue.push("test-change".to_string()).await;

    // Verify the notification was received
    let result = handle.await.or_fail("unexpected error");
    assert!(
        result.is_ok(),
        "Queue notification should have been received"
    );
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_attempt_merge_defers_when_change_not_archived() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create openspec/changes/test-change directory (simulating incomplete archive)
    let change_dir = repo_root.join("openspec/changes/test-change");
    fs::create_dir_all(&change_dir).or_fail("unexpected error");
    fs::write(change_dir.join("spec.md"), "# Test").or_fail("unexpected error");

    // Commit the change directory to ensure working tree is clean
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Add test change (not archived)"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create executor
    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["test-workspace".to_string()];
    let change_ids = vec!["test-change".to_string()];

    // Attempt merge should be deferred because change directory exists
    let archive_paths = vec![repo_root.to_path_buf()];
    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Deferred(deferred)) => {
            assert!(!deferred.auto_resumable);
            assert!(
                deferred.reason.contains("Archive incomplete"),
                "Expected deferred reason to mention archive incomplete, got: {}",
                deferred.reason
            );
            assert!(
                deferred.reason.contains("test-change"),
                "Expected reason to include change ID, got: {}",
                deferred.reason
            );
        }
        Ok(MergeAttempt::Merged { .. }) => {
            panic!("Merge should have been deferred when change directory exists");
        }
        Err(e) => {
            panic!("Expected MergeDeferred, got error: {}", e);
        }
    }
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_attempt_merge_succeeds_when_change_archived() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create archive directory but NOT openspec/changes/test-change (proper archive)
    let archive_dir = repo_root.join("openspec/changes/archive/test-change");
    fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    fs::write(archive_dir.join("spec.md"), "# Archived").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create worktree for the change (outside the main repo to avoid dirty working tree)
    let workspace_base = TempDir::new().or_fail("unexpected error");
    let workspace_path = workspace_base.path().join("ws-test-change");

    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "ws-test-change",
            workspace_path.to_str().or_fail("unexpected error"),
            "HEAD",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create executor
    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["ws-test-change".to_string()];
    let change_ids = vec!["test-change".to_string()];

    // Attempt merge should succeed because change is properly archived
    let archive_paths = vec![workspace_path.clone()];
    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Merged { .. }) => {
            // Success - merge was allowed
        }
        Ok(MergeAttempt::Deferred(deferred)) => {
            panic!(
                "Merge should have succeeded when change is archived, got deferred: {}",
                deferred.reason
            );
        }
        Err(e) => {
            // This is also acceptable - merge may fail for other reasons (e.g., merge conflicts)
            // but it should not be deferred due to archive verification
            println!("Merge failed with error (acceptable): {}", e);
        }
    }
}

/// Test that the has_resolve_wait helper correctly tracks ResolveWait state.
#[tokio::test]
async fn test_scheduler_syncs_manual_resolve_wait_from_shared_state() {
    use std::sync::Arc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        crate::orchestration::state::ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
    }

    executor.shared_orchestrator_state = Some(shared.clone());
    executor.resolve_wait_changes.clear();

    executor.sync_resolve_wait_from_shared_state_nonblocking();

    assert!(
        executor.resolve_wait_changes.contains("change-a"),
        "scheduler must mirror reducer-owned ResolveWait intent before idle/drained checks"
    );

    let queued = Vec::new();
    let in_flight = HashSet::new();
    let should_exit = executor
        .should_exit_when_idle(true, &queued, &in_flight)
        .await;
    assert!(
        !should_exit,
        "finite scheduler must remain alive while reducer-visible ResolveWait work can unblock queued dependents"
    );
}

#[tokio::test]
async fn handle_merge_result_releases_resolve_wait_retry_lane_on_auto_deferred() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("change-a".to_string(), WaitState::ResolveWait))
        );
        assert!(guard.is_base_mutating_lane_occupied());
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let merged = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "change-a".to_string(),
                workspace_name: "ws-change-a".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Ok(MergeTaskOutcome::deferred("Merge lane busy", true)),
            },
            &merge_result_tx,
        )
        .await;

    assert!(!merged);
    let guard = shared.read().await;
    assert!(!guard.is_base_mutating_lane_occupied());
    assert_eq!(
        guard.resolve_wait_change_ids(),
        vec!["change-a".to_string()]
    );
    assert!(guard.global_invariants_hold());
}

#[tokio::test]
async fn handle_merge_result_releases_reject_wait_retry_lane_and_suppresses_duplicate_error() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let mut executor = ParallelExecutor::new(repo_root, config, Some(event_tx));
    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["lane".to_string(), "reject-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived("lane".to_string()));
        guard.mark_reject_wait("reject-a");
        guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "lane".to_string(),
            revision: "rev".to_string(),
        });
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("reject-a".to_string(), WaitState::RejectWait))
        );
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let merged = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "reject-a".to_string(),
                workspace_name: "ws-reject-a".to_string(),
                origin: MergeResultOrigin::RejectWaitRetry,
                outcome: Err("specific rejection review failure already emitted".to_string()),
            },
            &merge_result_tx,
        )
        .await;

    assert!(!merged);
    let guard = shared.read().await;
    assert!(!guard.is_base_mutating_lane_occupied());
    assert_eq!(guard.reject_wait_change_ids(), vec!["reject-a".to_string()]);
    assert!(guard.global_invariants_hold());
    assert!(
        event_rx.try_recv().is_err(),
        "retry-origin errors must not emit duplicate generic ParallelEvent::Error"
    );
}

#[tokio::test]
async fn handle_merge_result_suppresses_resolve_retry_generic_error() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let mut executor = ParallelExecutor::new(repo_root, config, Some(event_tx));
    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);
    let merged = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "resolve-a".to_string(),
                workspace_name: "ws-resolve-a".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Err("ResolveFailed already emitted by retry body".to_string()),
            },
            &merge_result_tx,
        )
        .await;

    assert!(!merged);
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 0);
    assert!(
        event_rx.try_recv().is_err(),
        "ResolveWait retry errors must not emit duplicate generic ParallelEvent::Error"
    );
}

#[tokio::test]
async fn resolve_retry_workspace_lookup_failure_is_operator_visible() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    init_git_repo(repo_dir.path()).await;
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    executor.workspace_manager = Box::new(TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0))));
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["missing-ws".to_string(), "next-ws".to_string()],
        0,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        for change_id in ["missing-ws", "next-ws"] {
            guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
                change_id: change_id.to_string(),
                reason: "manual conflict".to_string(),
                auto_resumable: false,
            });
            guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()));
        }
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("missing-ws".to_string(), WaitState::ResolveWait))
        );
        assert!(guard.is_base_mutating_lane_occupied());
    }
    executor.set_shared_orchestrator_state(shared.clone());
    executor
        .resolve_wait_changes
        .insert("missing-ws".to_string());

    let outcome = executor
        .retry_deferred_merges_for(vec!["missing-ws".to_string()])
        .await
        .or_fail("missing workspace is treated as stale intent cleanup");

    assert_eq!(outcome, MergeTaskOutcome::Merged);
    let mut saw_workspace_error = false;
    while let Ok(event) = event_rx.try_recv() {
        if let ExecutionEvent::Error { message } = event {
            saw_workspace_error =
                message.contains("No workspace found for ResolveWait retry 'missing-ws'");
        }
    }
    assert!(
        saw_workspace_error,
        "missing-workspace retry path must emit an operator-visible Error event"
    );
    {
        let guard = shared.read().await;
        assert!(!guard.is_base_mutating_lane_occupied());
        assert!(!guard
            .resolve_wait_change_ids()
            .contains(&"missing-ws".to_string()));
        assert!(!guard
            .reject_wait_change_ids()
            .contains(&"missing-ws".to_string()));
        assert!(guard.global_invariants_hold());
    }
    assert_eq!(
        shared
            .write()
            .await
            .promote_next_base_mutating_lane_waiter(),
        Some(("next-ws".to_string(), WaitState::ResolveWait))
    );
}

#[tokio::test]
async fn reject_retry_workspace_lookup_failure_is_operator_visible() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    init_git_repo(repo_dir.path()).await;
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        create_test_config(),
        Some(event_tx),
    );
    executor.workspace_manager = Box::new(TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0))));
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![
            "lane".to_string(),
            "missing-reject-ws".to_string(),
            "next-reject-ws".to_string(),
        ],
        0,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived("lane".to_string()));
        guard.mark_reject_wait("missing-reject-ws");
        guard.mark_reject_wait("next-reject-ws");
        guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "lane".to_string(),
            revision: "rev".to_string(),
        });
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("missing-reject-ws".to_string(), WaitState::RejectWait))
        );
        assert!(guard.is_base_mutating_lane_occupied());
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let outcome = executor
        .retry_deferred_rejection_review_for("missing-reject-ws".to_string())
        .await
        .or_fail("missing rejection workspace is treated as stale intent cleanup");

    assert_eq!(outcome, MergeTaskOutcome::Merged);
    let mut saw_workspace_error = false;
    while let Ok(event) = event_rx.try_recv() {
        if let ExecutionEvent::Error { message } = event {
            saw_workspace_error =
                message.contains("No workspace found for RejectWait retry 'missing-reject-ws'");
        }
    }
    assert!(
        saw_workspace_error,
        "missing RejectWait workspace path must emit an operator-visible Error event"
    );
    {
        let guard = shared.read().await;
        assert!(!guard.is_base_mutating_lane_occupied());
        assert!(!guard
            .resolve_wait_change_ids()
            .contains(&"missing-reject-ws".to_string()));
        assert!(!guard
            .reject_wait_change_ids()
            .contains(&"missing-reject-ws".to_string()));
        assert!(guard.global_invariants_hold());
    }
    assert_eq!(
        shared
            .write()
            .await
            .promote_next_base_mutating_lane_waiter(),
        Some(("next-reject-ws".to_string(), WaitState::RejectWait))
    );
}

#[tokio::test]
async fn resolve_give_up_promotes_next_waiter_without_user_action() {
    // This test spawns a retry that uses the process-global merge lock.
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    let repo_dir = TempDir::new().or_fail("create temp repo");
    init_git_repo(repo_dir.path()).await;
    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.workspace_manager = Box::new(TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0))));
    let (merge_result_tx, mut merge_result_rx) = mpsc::channel(8);
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["first".to_string(), "second".to_string()],
        0,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        for change_id in ["first", "second"] {
            guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
                change_id: change_id.to_string(),
                reason: "manual conflict".to_string(),
                auto_resumable: false,
            });
            guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()));
        }
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("first".to_string(), WaitState::ResolveWait))
        );
    }
    executor.set_shared_orchestrator_state(shared.clone());
    executor.resolve_wait_changes.insert("first".to_string());

    let outcome = executor
        .retry_deferred_merges_for(vec!["first".to_string()])
        .await
        .or_fail("missing first workspace gives up as merged trigger");
    assert_eq!(outcome, MergeTaskOutcome::Merged);

    let merged = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "first".to_string(),
                workspace_name: "ws-first".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Ok(MergeTaskOutcome::Merged),
            },
            &merge_result_tx,
        )
        .await;
    assert!(merged);

    {
        let guard = shared.read().await;
        assert_eq!(
            guard.base_mutating_lane_occupant(),
            Some("second".to_string())
        );
        assert!(!guard
            .resolve_wait_change_ids()
            .contains(&"first".to_string()));
        assert!(!guard
            .reject_wait_change_ids()
            .contains(&"first".to_string()));
        assert!(guard.global_invariants_hold());
    }
    let retry_result =
        tokio::time::timeout(std::time::Duration::from_secs(2), merge_result_rx.recv())
            .await
            .or_fail("second retry result should arrive")
            .or_fail("second retry result channel should remain open");
    assert_eq!(retry_result.change_id, "second");
    assert_eq!(retry_result.origin, MergeResultOrigin::ResolveWaitRetry);
}

#[tokio::test]
async fn retry_lane_busy_release_allows_subsequent_repromotion() {
    // Serialize against other suites that manipulate the global merge lock so
    // their `try_lock` contention checks do not race this test's held guard.
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    let (merge_result_tx, _merge_result_rx) = mpsc::channel(4);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("change-a".to_string(), WaitState::ResolveWait))
        );
        assert!(guard.is_base_mutating_lane_occupied());
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let merge_guard = global_merge_lock().lock().await;
    let deferred_by_lock = executor
        .attempt_merge(
            &["retry-rev".to_string()],
            &["change-a".to_string()],
            &[PathBuf::from("/tmp/retry-archive")],
        )
        .await
        .or_fail("attempt_merge should report lock contention as a deferred retry");
    let outcome = match deferred_by_lock {
        MergeAttempt::Deferred(deferred) => {
            assert!(deferred.auto_resumable);
            assert!(
                deferred.reason.contains("Merge lane busy"),
                "test must exercise the global_merge_lock contention branch, got: {}",
                deferred.reason
            );
            MergeTaskOutcome::Deferred {
                reason: deferred.reason,
                auto_resumable: deferred.auto_resumable,
            }
        }
        other => panic!("expected lock-contention deferral, got {other:?}"),
    };
    drop(merge_guard);

    let merged = executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "change-a".to_string(),
                workspace_name: "ws-change-a".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Ok(outcome),
            },
            &merge_result_tx,
        )
        .await;

    assert!(!merged);
    {
        let guard = shared.read().await;
        assert!(!guard.is_base_mutating_lane_occupied());
        assert_eq!(
            guard.resolve_wait_change_ids(),
            vec!["change-a".to_string()]
        );
    }
    let promoted_again = shared
        .write()
        .await
        .promote_next_base_mutating_lane_waiter();
    assert_eq!(
        promoted_again,
        Some(("change-a".to_string(), WaitState::ResolveWait)),
        "released lane-busy retry must be promotable again on the next trigger"
    );
    assert!(shared.read().await.global_invariants_hold());
}

#[tokio::test]
async fn deferred_retry_lane_repromotes_after_merge_completion_trigger() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    let (merge_result_tx, mut merge_result_rx) = mpsc::channel(8);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
        assert_eq!(
            guard.promote_next_base_mutating_lane_waiter(),
            Some(("change-a".to_string(), WaitState::ResolveWait))
        );
    }
    executor.set_shared_orchestrator_state(shared.clone());

    executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "change-a".to_string(),
                workspace_name: "ws-change-a".to_string(),
                origin: MergeResultOrigin::ResolveWaitRetry,
                outcome: Ok(MergeTaskOutcome::deferred("Merge lane busy", true)),
            },
            &merge_result_tx,
        )
        .await;
    assert!(!shared.read().await.is_base_mutating_lane_occupied());

    executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "blocking-merge".to_string(),
                workspace_name: "ws-blocking-merge".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: Ok(MergeTaskOutcome::Merged),
            },
            &merge_result_tx,
        )
        .await;

    assert!(
        shared.read().await.is_base_mutating_lane_occupied(),
        "merged trigger must promote the released retry waiter without user action"
    );
    let retry_result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        merge_result_rx.recv(),
    )
    .await
    .or_fail("promotion should spawn a retry task result")
    .or_fail("promotion result channel closed");
    assert_eq!(retry_result.change_id, "change-a");
    assert_eq!(retry_result.origin, MergeResultOrigin::ResolveWaitRetry);
}

#[tokio::test]
async fn finite_scheduler_does_not_drain_while_spawned_retry_is_pending() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (merge_result_tx, mut merge_result_rx) = mpsc::channel(4);
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    let queued = Vec::new();
    let in_flight = HashSet::new();

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["retry-a".to_string(), "retry-b".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        for change_id in ["retry-a", "retry-b"] {
            guard.apply_observation(
                change_id,
                crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
            );
            guard.apply_command(ReducerCommand::ResolveMerge(change_id.to_string()));
        }
    }
    executor.set_shared_orchestrator_state(shared.clone());
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);
    executor
        .handle_merge_result_with_tx(
            MergeResult {
                change_id: "blocking-merge".to_string(),
                workspace_name: "ws-blocking-merge".to_string(),
                origin: MergeResultOrigin::PostArchiveMerge,
                outcome: Ok(MergeTaskOutcome::Merged),
            },
            &merge_result_tx,
        )
        .await;

    assert_eq!(
        executor.pending_merge_count.load(Ordering::Relaxed),
        1,
        "post-archive merge completion should release one count and spawn one retry count"
    );
    assert!(!executor.is_fully_drained(true, true, true));
    assert!(
        !executor
            .should_exit_when_idle(true, &queued, &in_flight)
            .await,
        "finite scheduler must not exit while detached retry result is pending"
    );

    let spawned_retry = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        merge_result_rx.recv(),
    )
    .await
    .or_fail("spawned retry should report through the merge-result channel")
    .or_fail("merge-result channel closed before spawned retry reported");
    assert_eq!(spawned_retry.change_id, "retry-a");
    assert_eq!(spawned_retry.origin, MergeResultOrigin::ResolveWaitRetry);
    assert!(
        !executor.is_fully_drained(true, true, true),
        "receiving a spawned result is not enough; the scheduler must handle it first"
    );

    assert!(
        executor
            .handle_merge_result_with_tx(
                MergeResult {
                    change_id: "retry-a".to_string(),
                    workspace_name: "ws-retry-a".to_string(),
                    origin: MergeResultOrigin::ResolveWaitRetry,
                    outcome: Ok(MergeTaskOutcome::Merged),
                },
                &merge_result_tx,
            )
            .await
    );

    assert!(
        !shared
            .read()
            .await
            .resolve_wait_change_ids()
            .contains(&"retry-a".to_string()),
        "handled merged retry should clear the completed ResolveWait entry"
    );
    assert!(
        shared.read().await.is_base_mutating_lane_occupied(),
        "handling the completed retry should promote the next waiter"
    );
    assert!(
        shared.read().await.is_base_mutating_lane_occupied(),
        "handling the completed retry should promote the next waiter before drain can complete"
    );
}

#[tokio::test]
async fn test_manual_resolve_wait_retries_after_in_flight_apply_completes() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use crate::parallel::WorkspaceResult;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace("change-a", workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["applying-change".to_string(), "change-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
    }
    executor.set_shared_orchestrator_state(shared);
    executor.sync_resolve_wait_from_shared_state_nonblocking();
    executor.last_dispatched_resolve_wait_changes = executor.resolve_wait_changes.clone();

    assert!(
        !executor.should_dispatch_resolve_wait_retry(),
        "unchanged ResolveWait should stay pending while unrelated apply work is still in flight"
    );

    executor
        .handle_workspace_completion(
            WorkspaceResult {
                change_id: "applying-change".to_string(),
                workspace_name: "applying-change".to_string(),
                final_revision: None,
                error: None,
                rejected: None,
            },
            1,
            &mut HashSet::from(["applying-change".to_string()]),
            &mpsc::channel(1).0,
        )
        .await;
    executor.trigger_resolve_wait_retry_dispatch();
    executor.maybe_dispatch_resolve_wait_retry().await;

    let mut saw_retry_dispatch = false;
    let mut saw_manual_deferral = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            crate::events::ExecutionEvent::Log(log)
                if log
                    .message
                    .contains("ResolveWait retry dispatch started for 'change-a'") =>
            {
                saw_retry_dispatch = true;
            }
            crate::events::ExecutionEvent::MergeDeferred {
                change_id,
                auto_resumable: false,
                ..
            } if change_id == "change-a" => saw_manual_deferral = true,
            _ => {}
        }
    }

    assert!(
        saw_retry_dispatch,
        "completion of unrelated in-flight apply work must wake scheduler-owned ResolveWait retry"
    );
    assert!(
        saw_manual_deferral,
        "retry attempt must produce a visible terminal wait outcome instead of silent pending"
    );
}

#[tokio::test]
async fn test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work() {
    use std::sync::Arc;
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, mut rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(repo_root, config, Some(tx));

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        crate::orchestration::state::ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
    }
    executor.set_shared_orchestrator_state(shared);

    executor.sync_resolve_wait_from_shared_state_nonblocking();
    executor.trigger_resolve_wait_retry_dispatch();

    assert!(
        executor.should_dispatch_resolve_wait_retry(),
        "synced ResolveWait intent should be dispatchable even when no queued/in-flight work exists"
    );

    executor.maybe_dispatch_resolve_wait_retry().await;

    let saw_retry_event = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            match rx.recv().await {
                Some(crate::events::ExecutionEvent::Log(log))
                    if log
                        .message
                        .contains("ResolveWait retry dispatch started for 'change-a'") =>
                {
                    break true;
                }
                Some(_) => continue,
                None => break false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(
        saw_retry_event,
        "manual startup path must reach retry dispatch instead of stopping as a zero-change no-op"
    );

    assert_eq!(
        executor.last_dispatched_resolve_wait_changes, executor.resolve_wait_changes,
        "dispatch path should snapshot synced ResolveWait ids"
    );
    assert!(
        !executor.resolve_wait_retry_triggered,
        "dispatch path should consume retry trigger"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn test_scheduler_reconciliation_missing_candidate_warn_is_observable_but_bounded() {
    use crate::events::{ExecutionEvent, LogLevel};
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex as StdMutex};
    use tokio::sync::mpsc;
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone)]
    struct CapturedLogs(Arc<StdMutex<Vec<u8>>>);

    struct CapturedLogWriter(Arc<StdMutex<Vec<u8>>>);

    impl Write for CapturedLogWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("captured log buffer poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CapturedLogs {
        type Writer = CapturedLogWriter;

        fn make_writer(&'a self) -> Self::Writer {
            CapturedLogWriter(self.0.clone())
        }
    }

    let captured_logs = Arc::new(StdMutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::WARN)
        .with_writer(CapturedLogs(captured_logs.clone()))
        .finish();
    let _subscriber_guard = tracing::subscriber::set_default(subscriber);

    let config = create_test_config();
    let repo_dir = tempfile::tempdir().or_fail("create temp repo");
    let changes_dir = repo_dir.path().join("openspec/changes");
    let loadable_change_id = "fix-missing-candidate-log-spam";
    let loadable_change_dir = changes_dir.join(loadable_change_id);
    std::fs::create_dir_all(&loadable_change_dir).or_fail("create loadable change dir");
    std::fs::write(
        loadable_change_dir.join("proposal.md"),
        "# Fix missing candidate log spam\n",
    )
    .or_fail("write loadable proposal");
    std::fs::write(
        loadable_change_dir.join("tasks.md"),
        "- [ ] Add bounded warning\n",
    )
    .or_fail("write loadable tasks");

    let (tx, mut rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    let missing_change_id = "definitely-missing-candidate-for-reconciliation";
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![
            missing_change_id.to_string(),
            loadable_change_id.to_string(),
        ],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(missing_change_id.to_string()));
        guard.apply_command(ReducerCommand::AddToQueue(loadable_change_id.to_string()));
    }
    executor.set_shared_orchestrator_state(shared);

    let mut queued = Vec::new();
    let in_flight = HashSet::new();

    let first_added = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;
    let second_added = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &in_flight)
        .await;

    assert_eq!(
        first_added.queued_added, 1,
        "loadable reducer-queued change should be added"
    );
    assert_eq!(
        first_added.repair_added, 0,
        "missing candidate test should not add repair candidates"
    );
    assert_eq!(
        second_added.total_added(),
        0,
        "second reconciliation should not add duplicates"
    );
    assert!(
        queued.iter().any(|change| change.id == loadable_change_id),
        "loadable reducer-queued change should be present in scheduler-local queue"
    );
    assert!(
        !queued.iter().any(|change| change.id == missing_change_id),
        "missing reducer-queued candidate must not be inserted into scheduler-local queue"
    );

    let mut candidate_not_found_events = 0usize;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::Log(log) = event {
            if log.level == LogLevel::Warn && log.message.contains("candidate_not_found") {
                candidate_not_found_events += 1;
            }
        }
    }
    assert_eq!(
        candidate_not_found_events, 1,
        "candidate_not_found should remain visible once without repeated TUI-visible warnings"
    );

    drop(_subscriber_guard);
    let captured = String::from_utf8(
        captured_logs
            .lock()
            .expect("captured log buffer poisoned")
            .clone(),
    )
    .expect("tracing output should be valid UTF-8");
    let structured_warn_count = captured
        .matches("Queue reconciliation could not load reducer-queued change")
        .count();
    assert_eq!(
        structured_warn_count, 1,
        "tracing warn should be emitted once for missing reducer-queued candidate"
    );
}

#[tokio::test]
async fn test_reducer_visible_queue_addition_marks_reanalysis_timestamp_and_enqueues_change() {
    let config = create_test_config();
    let repo_dir = tempfile::tempdir().or_fail("create temp repo");
    let change_id = "reducer-visible-queue-addition";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create reducer-visible change dir");
    std::fs::write(
        change_dir.join("proposal.md"),
        "# Reducer visible queue addition\n",
    )
    .or_fail("write reducer-visible proposal");
    std::fs::write(change_dir.join("tasks.md"), "- [ ] Add queue coverage\n")
        .or_fail("write reducer-visible tasks");

    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, None);
    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
    }
    executor.set_shared_orchestrator_state(shared);

    let mut queued = Vec::new();
    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;

    assert_eq!(outcome.queued_added, 1);
    assert_eq!(outcome.repair_added, 0);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, change_id);
    assert!(
        executor.last_queue_change_at.lock().await.is_some(),
        "reducer-visible queued additions should stamp queue-change debounce state"
    );
}

#[tokio::test]
async fn test_reducer_visible_queue_addition_preserves_existing_reanalysis_timestamp() {
    let config = create_test_config();
    let repo_dir = tempfile::tempdir().or_fail("create temp repo");
    let change_id = "reducer-visible-queue-debounce-preserved";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create reducer-visible change dir");
    std::fs::write(
        change_dir.join("proposal.md"),
        "# Reducer visible queue debounce preserved\n",
    )
    .or_fail("write reducer-visible proposal");
    std::fs::write(change_dir.join("tasks.md"), "- [ ] Add queue coverage\n")
        .or_fail("write reducer-visible tasks");

    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, None);
    let existing_timestamp = std::time::Instant::now() - std::time::Duration::from_secs(5);
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(existing_timestamp);
    }

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
    }
    executor.set_shared_orchestrator_state(shared);

    let mut queued = Vec::new();
    let outcome = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;

    assert_eq!(outcome.queued_added, 1);
    assert_eq!(outcome.repair_added, 0);
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].id, change_id);
    assert_eq!(
        *executor.last_queue_change_at.lock().await,
        Some(existing_timestamp),
        "reducer-visible reconciliation must not refresh an existing queue debounce timestamp"
    );
}

#[tokio::test]
async fn test_archived_dirty_reconciliation_skips_workspace_already_merged_to_base() {
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    init_git_repo(workspace_dir.path()).await;

    let change_id = "fix-dependency-target-handling";
    let archive_dir = workspace_dir
        .path()
        .join("openspec/changes/archive/2026-05-08-fix-dependency-target-handling");
    std::fs::create_dir_all(&archive_dir).or_fail("create archived change dir");
    std::fs::write(
        archive_dir.join("proposal.md"),
        "# Fix dependency target handling\n",
    )
    .or_fail("write archived proposal");
    std::fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] Archive move completed\n",
    )
    .or_fail("write archived tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git add archived change");
    Command::new("git")
        .args(["commit", "-m", "Archive fix-dependency-target-handling"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git commit archived change");

    let (tx, mut rx) = mpsc::channel(64);
    let config = create_test_config();
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace(change_id, workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        Vec::new(),
        1,
        ExecutionMode::Parallel,
    )));
    executor.set_shared_orchestrator_state(shared);

    let mut queued = Vec::new();
    let added = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;

    assert_eq!(
        added.total_added(),
        0,
        "terminal merged leftover worktree must not be rediscovered as an archived dirty repair candidate"
    );
    assert!(
        queued.is_empty(),
        "merged leftover worktree must not enter scheduler-local queued work"
    );

    while let Ok(event) = rx.try_recv() {
        if let crate::events::ExecutionEvent::Log(log) = event {
            assert!(
                !log.message.contains("archived_dirty_repair_candidate"),
                "merged terminal worktree must not emit archived-dirty repair diagnostics: {}",
                log.message
            );
        }
    }
}

#[tokio::test]
async fn test_archived_dirty_reconciliation_skips_manual_merge_wait() {
    use crate::events::ExecutionEvent;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    init_git_repo(workspace_dir.path()).await;

    let change_id = "add-session-signers";
    let archive_dir = workspace_dir
        .path()
        .join("openspec/changes/archive/2026-05-14-add-session-signers");
    std::fs::write(workspace_dir.path().join("base-only.txt"), "base\n")
        .or_fail("write base content");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git add base content");
    Command::new("git")
        .args(["commit", "-m", "base before archived workspace"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git commit base content");
    std::fs::create_dir_all(&archive_dir).or_fail("create archived change dir");
    std::fs::write(archive_dir.join("proposal.md"), "# Add session signers\n")
        .or_fail("write archived proposal");
    std::fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] Archive move completed\n",
    )
    .or_fail("write archived tasks");
    std::fs::write(
        archive_dir.join("report.md"),
        "# uncommitted archive report\n",
    )
    .or_fail("leave archived workspace dirty");

    let (tx, mut rx) = mpsc::channel(64);
    let config = create_test_config();
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace(change_id, workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived(change_id.to_string()));
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "base has unresolved conflicts".to_string(),
            auto_resumable: false,
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let added = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;
    let rediscovered = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;

    assert_eq!(added.total_added(), 0);
    assert_eq!(rediscovered.total_added(), 0);
    assert!(queued.is_empty());
    assert!(shared.read().await.queued_change_ids().is_empty());
    assert_eq!(
        shared.read().await.merge_wait_change_ids(),
        vec![change_id.to_string()]
    );

    let mut manual_wait_diagnostic_count = 0usize;
    while let Ok(event) = rx.try_recv() {
        if let crate::events::ExecutionEvent::Log(log) = event {
            assert!(
                !log.message.contains("archived_dirty_repair_candidate"),
                "manual merge-wait worktree must not be requeued as archived-dirty repair: {}",
                log.message
            );
            if log.message.contains("manual_merge_wait") {
                manual_wait_diagnostic_count += 1;
            }
        }
    }
    assert_eq!(manual_wait_diagnostic_count, 1);
}

#[tokio::test]
async fn test_archived_dirty_reconciliation_keeps_terminal_error_stopped_until_retry() {
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    init_git_repo(workspace_dir.path()).await;

    let change_id = "fix-dependency-target-handling";
    let archive_dir = workspace_dir
        .path()
        .join("openspec/changes/archive/2026-05-08-fix-dependency-target-handling");
    std::fs::write(workspace_dir.path().join("base-only.txt"), "base\n")
        .or_fail("write unrelated base content");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git add base content before dirty repair fixture");
    Command::new("git")
        .args(["commit", "-m", "base before archived dirty repair"])
        .current_dir(workspace_dir.path())
        .output()
        .await
        .or_fail("git commit base content before dirty repair fixture");
    std::fs::create_dir_all(&archive_dir).or_fail("create archived change dir after base commit");
    std::fs::write(
        archive_dir.join("proposal.md"),
        "# Fix dependency target handling\n",
    )
    .or_fail("write archived proposal after base commit");
    std::fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] Archive move completed\n- [ ] Commit finalization pending\n",
    )
    .or_fail("write archived tasks after base commit");
    std::fs::write(archive_dir.join("report.md"), "# final report\n")
        .or_fail("leave archived workspace dirty after base commit");

    let (tx, mut rx) = mpsc::channel(64);
    let config = create_test_config();
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace(change_id, workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        guard.apply_execution_event(&crate::events::ExecutionEvent::ApplyStarted {
            change_id: change_id.to_string(),
            command: "apply".to_string(),
        });
        guard.apply_execution_event(&crate::events::ExecutionEvent::ArchiveFailed {
            change_id: change_id.to_string(),
            error: "Archive commit finalization failed".to_string(),
            reason: Some("archive_commit_incomplete".to_string()),
            summary: Some("archive move complete, commit incomplete".to_string()),
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    let mut queued = Vec::new();
    let added = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;
    let last_queue_change_after_repair = *executor.last_queue_change_at.lock().await;
    let rediscovered = executor
        .reconcile_queued_candidates_from_shared_state(&mut queued, &HashSet::new())
        .await;

    assert_eq!(
        added.total_added(),
        0,
        "terminal-error reducer state must not be rediscovered as ordinary archived-dirty repair work"
    );
    assert_eq!(
        rediscovered.total_added(),
        0,
        "unchanged terminal-error rediscovery should remain stopped"
    );
    assert_eq!(
        *executor.last_queue_change_at.lock().await,
        last_queue_change_after_repair,
        "terminal-error reconciliation must not refresh normal queue debounce"
    );
    assert!(queued.is_empty());

    let reducer_queued = shared.read().await.queued_change_ids();
    assert!(
        reducer_queued.is_empty(),
        "test must exercise the real post-failure reducer shape: terminal ArchiveFailed excludes queued_change_ids"
    );

    let mut retry_required_diagnostic_count = 0usize;
    while let Ok(event) = rx.try_recv() {
        if let crate::events::ExecutionEvent::Log(log) = event {
            assert!(
                !log.message.contains("archived_dirty_repair_candidate"),
                "terminal-error worktree must not emit archived-dirty repair diagnostics: {}",
                log.message
            );
            if log.message.contains("terminal_error_retry_required") {
                retry_required_diagnostic_count += 1;
            }
        }
    }
    assert_eq!(
        retry_required_diagnostic_count, 1,
        "reconciliation should emit one retry-required diagnostic while bounding unchanged repeats"
    );
}

async fn assert_parallel_acceptance_failure_stalls_within_one_run(stale_checkpoint: bool) {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    init_git_repo(repo_dir.path()).await;

    let change_id = "acceptance-restart";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(change_dir.join("proposal.md"), "# Acceptance restart\n")
        .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write active tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git add active change");
    Command::new("git")
        .args(["commit", "-m", "Add active change"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git commit active change");
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let workspace_path = workspace_base.path().join(change_id);
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            change_id,
            workspace_path.to_string_lossy().as_ref(),
            "HEAD",
        ])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("create resumed worktree");
    Command::new("git")
        .args([
            "commit",
            "--allow-empty",
            "-m",
            &format!("Apply: {change_id}"),
        ])
        .current_dir(&workspace_path)
        .output()
        .await
        .or_fail("create applied resume commit");

    let checkpoint_path = workspace_path.join(".cflx/acceptance-state.json");
    if stale_checkpoint {
        // A checkpoint left behind by an older Conflux version claims an almost
        // exhausted retry budget for this exact change. Dispatch must ignore it.
        let finding_identity =
            crate::orchestration::acceptance::normalize_findings(&["repeated finding"
                .to_string()
                .into()])[0]
                .identity
                .clone();
        std::fs::create_dir_all(checkpoint_path.parent().or_fail("checkpoint parent"))
            .or_fail("create stale checkpoint dir");
        std::fs::write(
            &checkpoint_path,
            format!(
                "{{\"state\":\"failed\",\"revision\":\"stale\",\"updated_at\":\"now\",                 \"workspace_path\":\"{}\",\"change_id\":\"{change_id}\",                 \"previous_finding_identities\":[\"{finding_identity}\"],                 \"semantic_fingerprint\":\"stale\",\"cycle_count\":{}}}",
                workspace_path.display(),
                MAX_ACCEPTANCE_RETRY_CYCLES - 1,
            ),
        )
        .or_fail("write stale checkpoint");
    }

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        // Apply only checks off the acceptance follow-up boxes, so the
        // semantic fingerprint (which ignores follow-up sections) stays
        // unchanged and only repeated findings drive the stall decision.
        apply_command: Some(format!(
            "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
             > openspec/changes/{change_id}/tasks.next \
             && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md\""
        )),
        acceptance_command: Some(
            "sh -c 'echo ACCEPTANCE: FAIL; echo FINDINGS:; echo - repeated finding'".to_string(),
        ),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision.clone(),
            semaphore.clone(),
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch resumed workspace");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");
    // Repeated findings without semantic progress are a runtime retry judgement,
    // not a reviewer-validated external blocker. The loop stops and requires an
    // explicit retry, without inventing a blocker category or writing anything
    // into the change directory.
    let error = result
        .error
        .as_deref()
        .or_fail("repeated findings must stop the retry loop with a diagnostic");
    assert!(
        error.contains("repeated_acceptance_finding"),
        "diagnostic must name the retry judgement: {error}"
    );
    assert!(
        error.contains("retry explicitly"),
        "diagnostic must state the operator action: {error}"
    );
    assert!(
        crate::parallel::acceptance_state::parse_blocked_marker(&workspace_path, change_id)
            .or_fail("read change directory")
            .is_none(),
        "acceptance must not create a blocked marker under the change directory"
    );

    if stale_checkpoint {
        assert!(
            checkpoint_path.exists(),
            "dispatch must not consume generated acceptance state"
        );
    } else {
        assert!(
            !checkpoint_path.exists(),
            "dispatch must never create an acceptance checkpoint"
        );
    }

    let mut acceptance_count = 0;
    let mut apply_count = 0;
    let mut saw_stalled = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                acceptance_count += 1;
            }
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                apply_count += 1;
            }
            ExecutionEvent::AcceptanceGated { change_id: id, .. } if id == change_id => {
                saw_stalled = true;
            }
            _ => {}
        }
    }
    assert_eq!(
        acceptance_count, 2,
        "the first failure must retry apply and only the repeated failure may stop"
    );
    assert_eq!(
        apply_count, 1,
        "exactly one apply retry precedes the repeated-finding stop"
    );
    assert!(
        !saw_stalled,
        "evidence-free retry exhaustion must not emit a stalled lifecycle transition"
    );
}

/// Observations from one dispatch whose acceptance command emits bare
/// `ACCEPTANCE: GATED` for its first `bare_attempts` invocations and then the
/// supplied final verdict payload.
struct GatedDispatch {
    result: WorkspaceResult,
    workspace_path: std::path::PathBuf,
    acceptance_invocations: u32,
    apply_invocations: u32,
    stalled_events: Vec<crate::events::StalledBlocker>,
    prompts: Vec<String>,
}

/// Dispatch one change against a gated-then-final acceptance fixture.
///
/// `stall_state_root` isolates runtime stall state per test, so nothing reaches
/// a developer's real XDG state directory and concurrent tests cannot observe
/// each other's holds.
#[allow(clippy::too_many_arguments)]
async fn dispatch_gated_run(
    repo_root: &std::path::Path,
    workspace_base_dir: &std::path::Path,
    state_dir: &std::path::Path,
    stall_state_root: &std::path::Path,
    change_id: &str,
    bare_attempts: u32,
    final_verdict: &str,
    explicit_retry: bool,
) -> GatedDispatch {
    let base_revision = get_current_commit(repo_root)
        .await
        .or_fail("get base revision");

    let counter = state_dir.join("attempts");
    let prompt_dir = state_dir.join("prompts");
    std::fs::create_dir_all(&prompt_dir).or_fail("create prompt capture dir");
    // The final verdict is read from a file so JSON quoting survives the shell
    // round trip intact.
    let verdict_file = state_dir.join("final-verdict.txt");
    std::fs::write(&verdict_file, final_verdict).or_fail("write final verdict fixture");
    let counter_display = counter.display().to_string();
    let prompt_dir_display = prompt_dir.display().to_string();
    let verdict_display = verdict_file.display().to_string();

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base_dir.to_string_lossy().to_string()),
        apply_command: Some(format!(
            "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
             > openspec/changes/{change_id}/tasks.next \
             && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md\""
        )),
        acceptance_command: Some(format!(
            "sh -c 'n=$(cat \"{counter_display}\" 2>/dev/null || echo 0); n=$((n+1)); \
             echo $n > \"{counter_display}\"; \
             printf \"%s\" \"$0\" > \"{prompt_dir_display}/attempt-$n.txt\"; \
             if [ $n -gt {bare_attempts} ]; then cat \"{verdict_display}\"; \
             else echo \"ACCEPTANCE: GATED\"; fi' {{prompt}}"
        )),
        archive_command: Some(format!(
            "sh -c 'mkdir -p openspec/changes/archive \
             && mv openspec/changes/{change_id} openspec/changes/archive/{change_id}'"
        )),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });

    let (tx, mut rx) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));
    executor.set_acceptance_stall_state_root(stall_state_root.to_path_buf());
    executor.set_explicit_retry(explicit_retry);
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_root.to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch gated change");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    let workspace_path = workspace_base_dir.join(format!("cflx-{change_id}"));
    let workspace_path = if workspace_path.exists() {
        workspace_path
    } else {
        executor
            .workspace_manager
            .find_existing_workspace(change_id)
            .await
            .ok()
            .flatten()
            .map(|workspace| workspace.path)
            .unwrap_or_else(|| workspace_base_dir.join(change_id))
    };

    let mut observed = GatedDispatch {
        result,
        workspace_path,
        acceptance_invocations: 0,
        apply_invocations: 0,
        stalled_events: Vec::new(),
        prompts: Vec::new(),
    };

    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                observed.acceptance_invocations += 1;
            }
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                observed.apply_invocations += 1;
            }
            ExecutionEvent::AcceptanceGated {
                change_id: id,
                blocker,
            } if id == change_id => observed.stalled_events.push(blocker),
            _ => {}
        }
    }

    let mut attempt = 1;
    while let Ok(prompt) =
        std::fs::read_to_string(prompt_dir.join(format!("attempt-{attempt}.txt")))
    {
        observed.prompts.push(prompt);
        attempt += 1;
    }

    observed
}

fn workspace_porcelain_status(workspace_path: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(workspace_path)
        .output()
        .or_fail("read workspace git status");
    String::from_utf8(output.stdout).or_fail("decode git status")
}

fn workspace_head(workspace_path: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace_path)
        .output()
        .or_fail("read workspace HEAD");
    String::from_utf8(output.stdout)
        .or_fail("decode HEAD")
        .trim()
        .to_string()
}

const VALIDATED_BLOCKER_VERDICT: &str = concat!(
    r#"{"acceptance":"gated","blocker":{"category":"credential","#,
    r#""evidence":["STAGING_API_KEY is unset in the verification environment"],"#,
    r#""next_action":"provision STAGING_API_KEY then retry acceptance","resumable":true}}"#,
    "\n"
);

/// gated → gated → PASS: exactly two acceptance-only retries, one apply, no
/// stalled transition, and a clean worktree with the apply commit intact.
#[tokio::test]
async fn parallel_bare_gated_retries_then_passes_without_stalling() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let state_dir = TempDir::new().or_fail("create fixture state dir");
    let stall_state = TempDir::new().or_fail("create stall state root");
    let change_id = "parallel-bare-gated-pass";
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    let observed = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        state_dir.path(),
        stall_state.path(),
        change_id,
        2,
        "ACCEPTANCE: PASS\n",
        false,
    )
    .await;

    assert!(
        observed.result.error.is_none(),
        "bare gated within budget must not fail the workspace: {:?}",
        observed.result.error
    );
    assert_eq!(
        observed.acceptance_invocations, 3,
        "the initial attempt plus exactly two protocol retries must run"
    );
    assert_eq!(
        observed.apply_invocations, 1,
        "a bare gated protocol retry must never rerun apply"
    );
    assert!(
        observed.stalled_events.is_empty(),
        "bare gated input must emit no stalled lifecycle transition"
    );

    assert!(!observed.prompts[0].contains("<acceptance_protocol_retry>"));
    for attempt in [1usize, 2] {
        let prompt = &observed.prompts[attempt];
        assert!(prompt.contains("<acceptance_protocol_retry>"));
        assert!(prompt.contains("without a validated structured blocker"));
        assert!(prompt.contains("Supported categories:"));
    }

    assert!(
        !observed
            .workspace_path
            .join("openspec/changes")
            .join(change_id)
            .join("APPLY_BLOCKED")
            .exists(),
        "acceptance must not create a marker under the change directory"
    );
}

/// Three consecutive bare gated results exhaust the shared budget, produce a
/// terminal protocol error, and still leave a clean worktree with no hold.
#[tokio::test]
async fn parallel_bare_gated_exhaustion_is_terminal_and_creates_no_hold() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let state_dir = TempDir::new().or_fail("create fixture state dir");
    let stall_state = TempDir::new().or_fail("create stall state root");
    let change_id = "parallel-bare-gated-exhausted";
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    let observed = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        state_dir.path(),
        stall_state.path(),
        change_id,
        5,
        "ACCEPTANCE: PASS\n",
        false,
    )
    .await;

    let error = observed
        .result
        .error
        .as_deref()
        .or_fail("exhausted bare gated retries must be terminal");
    assert!(error.contains("bare-blocker protocol failure"), "{error}");
    assert!(
        error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"),
        "{error}"
    );
    assert_eq!(
        observed.acceptance_invocations, 3,
        "no fourth protocol retry may start after exhaustion"
    );
    assert_eq!(observed.apply_invocations, 1);
    assert!(observed.stalled_events.is_empty());

    // No durable hold anywhere, and the worktree is untouched by acceptance.
    let store = crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
    assert!(store
        .load(
            &crate::parallel::acceptance_state::repository_identity(repo_dir.path()),
            change_id
        )
        .or_fail("read stall store")
        .is_none());
    assert_eq!(workspace_porcelain_status(&observed.workspace_path), "");
}

/// A validated structured blocker stalls immediately, records a revision-bound
/// hold outside the worktree, and preserves the clean worktree and apply commit.
/// A restart then restores `stalled` without re-running anything, and an
/// explicit retry resumes acceptance only.
/// Heavy: three full dispatch rounds against real git worktrees put this over
/// the one-second default-suite budget. The individual behaviours it chains are
/// also covered by faster default tests (stall persistence and restart
/// reconciliation in `execution::state`, dispatch suppression in
/// `runtime_stalled_change_is_not_requeued_until_its_hold_is_invalid`); this
/// test exists to prove they compose end to end.
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn parallel_validated_blocker_stalls_survives_restart_and_retries_acceptance_only() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let stall_state = TempDir::new().or_fail("create stall state root");
    let change_id = "parallel-validated-stall";
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    // --- 1. Enter the stall -------------------------------------------------
    let first = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        TempDir::new().or_fail("state dir").path(),
        stall_state.path(),
        change_id,
        0,
        VALIDATED_BLOCKER_VERDICT,
        false,
    )
    .await;

    assert!(
        first.result.error.is_none(),
        "a validated stall is not an error: {:?}",
        first.result.error
    );
    assert_eq!(
        first.acceptance_invocations, 1,
        "a validated blocker must not consume protocol retry budget"
    );
    assert_eq!(first.stalled_events.len(), 1);
    assert_eq!(first.stalled_events[0].category, "credential");
    assert!(first.stalled_events[0].resumable);

    let apply_revision = workspace_head(&first.workspace_path);
    assert_eq!(
        workspace_porcelain_status(&first.workspace_path),
        "",
        "entering a stall must leave the managed worktree clean"
    );
    assert!(!first
        .workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("APPLY_BLOCKED")
        .exists());

    let store = crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
    let repository_id = crate::parallel::acceptance_state::repository_identity(repo_dir.path());
    let record = store
        .load(&repository_id, change_id)
        .or_fail("read stall store")
        .or_fail("a validated blocker must persist a runtime hold");
    assert_eq!(record.category, "credential");
    assert_eq!(record.apply_revision, apply_revision);
    assert_eq!(record.phase, "acceptance");
    assert!(record.resumable);

    // --- 2. Restart: the hold suppresses dispatch entirely ------------------
    let restart_state = TempDir::new().or_fail("restart state dir");
    let restarted = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        restart_state.path(),
        stall_state.path(),
        change_id,
        0,
        VALIDATED_BLOCKER_VERDICT,
        false,
    )
    .await;

    assert!(restarted.result.error.is_none());
    assert_eq!(
        restarted.acceptance_invocations, 0,
        "a reconciled hold must start neither apply nor acceptance"
    );
    assert_eq!(restarted.apply_invocations, 0);
    assert_eq!(restarted.stalled_events.len(), 1);
    assert_eq!(restarted.stalled_events[0].category, "credential");
    assert_eq!(
        restarted.stalled_events[0].next_action,
        "provision STAGING_API_KEY then retry acceptance"
    );
    assert_eq!(workspace_porcelain_status(&first.workspace_path), "");
    assert_eq!(
        workspace_head(&first.workspace_path),
        apply_revision,
        "the apply commit must survive a stalled restart"
    );

    // --- 3. Explicit retry resumes acceptance only --------------------------
    let retry_state = TempDir::new().or_fail("retry state dir");
    let retried = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        retry_state.path(),
        stall_state.path(),
        change_id,
        0,
        "ACCEPTANCE: PASS\n",
        true,
    )
    .await;

    assert_eq!(
        retried.acceptance_invocations, 1,
        "explicit retry must re-run acceptance"
    );
    assert_eq!(
        retried.apply_invocations, 0,
        "explicit retry must resume at acceptance without rerunning apply"
    );
    assert!(
        store
            .load(&repository_id, change_id)
            .or_fail("read stall store")
            .is_none(),
        "a successful explicit retry consumes the hold"
    );
}

/// Ordinary queue reconciliation must not re-submit a runtime-stalled change,
/// and a hold that has lost its binding must release the change again.
#[tokio::test]
async fn runtime_stalled_change_is_not_requeued_until_its_hold_is_invalid() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let stall_state = TempDir::new().or_fail("create stall state root");
    let change_id = "parallel-queue-stalled";
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    let observed = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        TempDir::new().or_fail("state dir").path(),
        stall_state.path(),
        change_id,
        0,
        VALIDATED_BLOCKER_VERDICT,
        false,
    )
    .await;
    assert_eq!(observed.stalled_events.len(), 1);

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, None);
    executor.set_acceptance_stall_state_root(stall_state.path().to_path_buf());

    let queued = vec![make_test_change(change_id)];
    let classification = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;

    assert_eq!(
        classification.class_for(change_id),
        Some(crate::parallel::queue_state::QueuedWorkClass::AcceptanceStalled),
        "a runtime-stalled change must not be classified as dispatchable"
    );
    assert!(
        !classification.has_dispatchable_apply(),
        "ordinary reconciliation must not re-submit a stalled change (Blocked -> Blocked)"
    );
    assert!(classification.has_blocked_or_waiting_work());

    // Once the hold loses its binding it is quarantined and the change is free
    // to be dispatched again — a stale record can never block work forever.
    let store = crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
    let repository_id = crate::parallel::acceptance_state::repository_identity(repo_dir.path());
    let mut record = store
        .load(&repository_id, change_id)
        .or_fail("read stall store")
        .or_fail("hold must exist");
    record.apply_revision = "0".repeat(40);
    std::fs::write(
        store.record_path(&repository_id, change_id),
        serde_json::to_vec_pretty(&record).or_fail("encode record"),
    )
    .or_fail("plant stale record");

    let classification = executor
        .classify_queued_work(&queued, &HashSet::new())
        .await;
    assert_ne!(
        classification.class_for(change_id),
        Some(crate::parallel::queue_state::QueuedWorkClass::AcceptanceStalled),
        "a hold that lost its binding must release the change"
    );
}

/// Explicit retry is a preparation transaction. When it refuses to proceed —
/// here because the hold is not resumable — the blocker evidence must survive
/// untouched and no ambiguous work may be dispatched.
#[tokio::test]
async fn refused_explicit_retry_retains_the_acceptance_hold() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let stall_state = TempDir::new().or_fail("create stall state root");
    let change_id = "parallel-retry-refused";
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    // Enter a stall, then rewrite the hold as non-resumable.
    let observed = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        TempDir::new().or_fail("state dir").path(),
        stall_state.path(),
        change_id,
        0,
        VALIDATED_BLOCKER_VERDICT,
        false,
    )
    .await;
    assert_eq!(observed.stalled_events.len(), 1);

    let store = crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
    let repository_id = crate::parallel::acceptance_state::repository_identity(repo_dir.path());
    let mut record = store
        .load(&repository_id, change_id)
        .or_fail("read stall store")
        .or_fail("hold must exist");
    record.resumable = false;
    let before = store.save(&record).or_fail("store non-resumable hold");

    // Explicit retry must refuse and change nothing.
    let retried = dispatch_gated_run(
        repo_dir.path(),
        workspace_base.path(),
        TempDir::new().or_fail("retry state dir").path(),
        stall_state.path(),
        change_id,
        0,
        "ACCEPTANCE: PASS\n",
        true,
    )
    .await;

    assert_eq!(
        retried.acceptance_invocations, 0,
        "a refused retry must not dispatch acceptance"
    );
    assert_eq!(
        retried.apply_invocations, 0,
        "a refused retry must not dispatch apply"
    );

    let after = store
        .load(&repository_id, change_id)
        .or_fail("read stall store")
        .or_fail("a refused retry must retain the blocker evidence");
    assert_eq!(after, before);
    assert_eq!(workspace_porcelain_status(&observed.workspace_path), "");
}

/// Cleanup, archive, and merge decisions must stay entirely repository-derived.
/// Runtime stall state is not an input to any of them, so none of those modules
/// may reach for the store at all.
#[test]
fn cleanup_archive_and_merge_do_not_consult_acceptance_stall_state() {
    for (label, source) in [
        ("cleanup", include_str!("../cleanup.rs")),
        ("merge", include_str!("../merge.rs")),
        ("archive", include_str!("../../execution/archive.rs")),
    ] {
        for forbidden in ["AcceptanceStallStore", "load_valid_acceptance_stall"] {
            assert!(
                !source.contains(forbidden),
                "{label} must not consult acceptance stall state ({forbidden})"
            );
        }
    }
}

/// Observations collected from one dispatch driven by a stateful fake
/// acceptance command that withholds a canonical verdict for the first
/// `missing_attempts` invocations and then emits `ACCEPTANCE: PASS`./// Observations collected from one dispatch driven by a stateful fake
/// acceptance command that withholds a canonical verdict for the first
/// `missing_attempts` invocations and then emits `ACCEPTANCE: PASS`.
struct MissingVerdictDispatch {
    result: WorkspaceResult,
    workspace_path: std::path::PathBuf,
    acceptance_invocations: u32,
    apply_invocations: u32,
    retry_progress_logs: Vec<String>,
    acceptance_error_logs: Vec<String>,
    saw_processing_error: bool,
    /// Prompt text handed to each acceptance invocation, in order.
    prompts: Vec<String>,
}

/// Dispatch one change whose acceptance command exits without a canonical
/// verdict for its first `missing_attempts` invocations.
///
/// The command is the ordinary configured acceptance command on every
/// invocation — the fixture never supplies a harness session, resume flag, or
/// job identifier, so continuity can only come from Conflux-managed prompt
/// context.
async fn dispatch_with_missing_verdict_attempts(
    change_id: &str,
    missing_attempts: u32,
) -> MissingVerdictDispatch {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let state_dir = TempDir::new().or_fail("create acceptance fixture state dir");
    init_missing_verdict_repo(repo_dir.path(), change_id).await;
    dispatch_missing_verdict_run(
        repo_dir.path(),
        workspace_base.path(),
        state_dir.path(),
        change_id,
        missing_attempts,
    )
    .await
}

/// Create the base repository fixture for a missing-verdict dispatch.
///
/// The change starts with an incomplete task so apply genuinely runs once; a
/// protocol retry must then re-run acceptance only.
async fn init_missing_verdict_repo(repo_root: &std::path::Path, change_id: &str) {
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(change_dir.join("proposal.md"), "# Missing verdict\n")
        .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [ ] done\n",
    )
    .or_fail("write active tasks");
    init_git_repo(repo_root).await;
}

/// Run one dispatch against an existing repository fixture. Calling this twice
/// with the same repository and workspace base — but a fresh executor, agent,
/// and acceptance counter — models a process restart over the same unarchived
/// workspace.
async fn dispatch_missing_verdict_run(
    repo_root: &std::path::Path,
    workspace_base_dir: &std::path::Path,
    state_dir: &std::path::Path,
    change_id: &str,
    missing_attempts: u32,
) -> MissingVerdictDispatch {
    let repo_dir = repo_root;
    let workspace_base = workspace_base_dir;
    let base_revision = get_current_commit(repo_dir)
        .await
        .or_fail("get base revision");

    let counter = state_dir.join("attempts");
    let prompt_dir = state_dir.join("prompts");
    std::fs::create_dir_all(&prompt_dir).or_fail("create prompt capture dir");
    let counter_display = counter.display().to_string();
    let prompt_dir_display = prompt_dir.display().to_string();

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        apply_command: Some(format!(
            "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
             > openspec/changes/{change_id}/tasks.next \
             && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md\""
        )),
        acceptance_command: Some(format!(
            "sh -c 'n=$(cat \"{counter_display}\" 2>/dev/null || echo 0); n=$((n+1)); \
             echo $n > \"{counter_display}\"; \
             printf \"%s\" \"$0\" > \"{prompt_dir_display}/attempt-$n.txt\"; \
             if [ $n -gt {missing_attempts} ]; then echo \"ACCEPTANCE: PASS\"; \
             else echo \"Monitoring verification, waiting for the owned job to finish\"; fi' \
             {{prompt}}"
        )),
        archive_command: Some(format!(
            "sh -c 'mkdir -p openspec/changes/archive \
             && mv openspec/changes/{change_id} openspec/changes/archive/{change_id}'"
        )),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });

    let (tx, mut rx) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(repo_dir.to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch missing-verdict change");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    let workspace_path = workspace_base.join(format!("cflx-{change_id}"));
    let workspace_path = if workspace_path.exists() {
        workspace_path
    } else {
        executor
            .workspace_manager
            .find_existing_workspace(change_id)
            .await
            .ok()
            .flatten()
            .map(|workspace| workspace.path)
            .unwrap_or_else(|| workspace_base.join(change_id))
    };

    let mut observed = MissingVerdictDispatch {
        result,
        workspace_path,
        acceptance_invocations: 0,
        apply_invocations: 0,
        retry_progress_logs: Vec::new(),
        acceptance_error_logs: Vec::new(),
        saw_processing_error: false,
        prompts: Vec::new(),
    };

    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                observed.acceptance_invocations += 1;
            }
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                observed.apply_invocations += 1;
            }
            ExecutionEvent::ProcessingError { id, .. } if id == change_id => {
                observed.saw_processing_error = true;
            }
            ExecutionEvent::Log(log) => {
                if log.message.contains("protocol retry") {
                    observed.retry_progress_logs.push(log.message.clone());
                }
                if matches!(log.level, crate::events::LogLevel::Error)
                    && log.operation.as_deref() == Some("acceptance")
                {
                    observed.acceptance_error_logs.push(log.message.clone());
                }
            }
            _ => {}
        }
    }

    let mut attempt = 1;
    while let Ok(prompt) =
        std::fs::read_to_string(prompt_dir.join(format!("attempt-{attempt}.txt")))
    {
        observed.prompts.push(prompt);
        attempt += 1;
    }

    observed
}

/// A status-only acceptance exit must re-invoke the ordinary acceptance command
/// with bounded continuation context instead of failing the workspace, and a
/// later canonical PASS must archive normally.
#[tokio::test]
async fn parallel_missing_verdict_retries_then_passes_without_terminal_error() {
    let change_id = "missing-verdict-retry-pass";
    let observed = dispatch_with_missing_verdict_attempts(change_id, 2).await;

    assert!(
        observed.result.error.is_none(),
        "acceptance must continue through the protocol retry instead of failing: {:?}",
        observed.result.error
    );
    assert!(
        observed.result.final_revision.is_some(),
        "a canonical PASS after protocol retries must still hand off to archive"
    );
    assert_eq!(
        observed.acceptance_invocations, 3,
        "the initial attempt plus two protocol retries must run the acceptance command"
    );
    assert_eq!(
        observed.apply_invocations, 1,
        "a protocol retry re-runs acceptance only; the implementation did not fail"
    );
    assert!(
        !observed.saw_processing_error,
        "an in-budget protocol retry must not report the change as a terminal error, so queue \
         reconciliation cannot classify it as terminal_error_retry_required"
    );
    assert!(
        observed.acceptance_error_logs.is_empty(),
        "no acceptance error event may be emitted before exhaustion: {:?}",
        observed.acceptance_error_logs
    );

    // Non-terminal progress must be visible with attempt/maximum values.
    assert_eq!(
        observed.retry_progress_logs.len(),
        2,
        "each protocol retry must report progress: {:?}",
        observed.retry_progress_logs
    );
    assert!(observed.retry_progress_logs[0].contains("protocol retry 1/2"));
    assert!(observed.retry_progress_logs[1].contains("protocol retry 2/2"));

    // Continuation comes only from Conflux-managed prompt context.
    assert_eq!(observed.prompts.len(), 3);
    assert!(
        !observed.prompts[0].contains("<acceptance_protocol_retry>"),
        "the initial attempt must not receive corrective retry context"
    );
    for prompt in &observed.prompts[1..] {
        assert!(prompt.contains("<acceptance_protocol_retry>"));
        assert!(prompt.contains("emit exactly one canonical verdict"));
        assert!(
            prompt.contains("Monitoring verification"),
            "the retry must carry bounded prior acceptance output"
        );
        assert!(
            prompt.contains("Missing acceptance verdict"),
            "the retry must carry the recorded missing-verdict diagnostic"
        );
        let lower = prompt.to_ascii_lowercase();
        for forbidden in ["session_id", "--resume", "job_id"] {
            assert!(
                !lower.contains(forbidden),
                "continuation must stay harness neutral, found `{forbidden}`"
            );
        }
    }

    assert!(
        !observed
            .workspace_path
            .join("ACCEPTANCE_REPORT.json")
            .exists(),
        "protocol retries must not create a workspace acceptance report"
    );
    assert!(
        !observed
            .workspace_path
            .join(".cflx/acceptance-state.json")
            .exists(),
        "protocol retries must not create a durable acceptance checkpoint"
    );
}

/// Three consecutive missing verdicts exhaust the dedicated budget and produce
/// exactly one bounded terminal diagnostic.
#[tokio::test]
async fn parallel_missing_verdict_exhaustion_is_terminal_with_bounded_evidence() {
    let change_id = "missing-verdict-exhausted";
    let observed = dispatch_with_missing_verdict_attempts(change_id, u32::MAX).await;

    let error = observed
        .result
        .error
        .as_ref()
        .or_fail("exhausted protocol retries must fail the workspace");
    assert!(error.contains("missing-verdict protocol failure"));
    assert!(error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"));
    assert!(
        error.contains("Monitoring verification"),
        "terminal diagnostic must retain bounded evidence, got: {error}"
    );

    assert_eq!(
        observed.acceptance_invocations, 3,
        "no fourth protocol retry may start"
    );
    assert_eq!(observed.apply_invocations, 1);
    assert_eq!(
        observed.retry_progress_logs.len(),
        2,
        "only the two in-budget retries report non-terminal progress"
    );
    assert_eq!(
        observed.acceptance_error_logs.len(),
        1,
        "exhaustion must emit exactly one terminal acceptance error: {:?}",
        observed.acceptance_error_logs
    );
    assert!(observed.acceptance_error_logs[0].contains("Missing acceptance verdict"));

    assert!(
        !observed
            .workspace_path
            .join("ACCEPTANCE_REPORT.json")
            .exists(),
        "an exhausted missing verdict must not create a workspace acceptance report"
    );
}

/// Constitutional restart behavior: the protocol counter is active-run memory,
/// so a run that starts on a still-unarchived workspace runs acceptance again
/// from workspace state and cannot infer PASS from prior narrative output.
#[tokio::test]
async fn parallel_restart_reruns_acceptance_without_inferring_pass_from_prior_output() {
    let change_id = "missing-verdict-restart";
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let state_dir = TempDir::new().or_fail("create acceptance state dir");
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    // Model the post-restart world directly: an applied but unarchived
    // workspace and no out-of-worktree runtime state at all. A previous run's
    // protocol retries left nothing behind.
    let workspace_path = workspace_base.path().join(change_id);
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            change_id,
            workspace_path.to_string_lossy().as_ref(),
            "HEAD",
        ])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("create applied-but-unarchived worktree");
    std::fs::write(
        workspace_path
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("mark tasks applied in the resumed worktree");
    for args in [
        vec!["add", "-A"],
        vec!["commit", "-m", &format!("Apply: {change_id}")],
    ] {
        Command::new("git")
            .args(args)
            .current_dir(&workspace_path)
            .output()
            .await
            .or_fail("record the applied worktree commit");
    }

    let observed = dispatch_missing_verdict_run(
        repo_dir.path(),
        workspace_base.path(),
        state_dir.path(),
        change_id,
        u32::MAX,
    )
    .await;

    assert_eq!(
        observed.acceptance_invocations, 3,
        "the still-unarchived workspace must run acceptance again with a full, fresh protocol budget"
    );
    assert_eq!(
        observed.retry_progress_logs.len(),
        2,
        "the restarted run must start a fresh protocol budget at retry 1/2, got {:?}",
        observed.retry_progress_logs
    );
    assert!(observed.retry_progress_logs[0].contains("protocol retry 1/2"));
    assert!(observed.retry_progress_logs[1].contains("protocol retry 2/2"));
    assert!(
        observed
            .prompts
            .first()
            .is_some_and(|prompt| !prompt.contains("<acceptance_protocol_retry>")),
        "the restarted run's first attempt must not resume a prior protocol retry"
    );
    assert!(
        observed.result.final_revision.is_none(),
        "an unarchived workspace must never be treated as accepted from prior narrative output"
    );
    assert!(
        observed
            .result
            .error
            .as_ref()
            .is_some_and(|error| error.contains("missing-verdict protocol failure")),
        "acceptance must reach a verdict again rather than inheriting a prior outcome: {:?}",
        observed.result.error
    );
    assert!(
        !workspace_path.join(".cflx/acceptance-state.json").exists(),
        "restart routing must stay derivable from workspace file/git state"
    );
    assert!(
        !workspace_path.join("ACCEPTANCE_REPORT.json").exists(),
        "restart must not require a generated acceptance retry checkpoint"
    );
}

#[tokio::test]
async fn parallel_repeated_acceptance_failure_stops_without_a_change_directory_marker() {
    assert_parallel_acceptance_failure_stalls_within_one_run(false).await;
}

#[tokio::test]
async fn parallel_restart_ignores_generated_acceptance_state_when_deciding_retries() {
    assert_parallel_acceptance_failure_stalls_within_one_run(true).await;
}

/// End-to-end regression for the post-archive false `MergeWait`.
///
/// Runs apply -> acceptance PASS -> archive commit -> post-archive merge
/// verification in one dispatch and proves that no generated acceptance
/// checkpoint is created, that archive leaves a clean worktree, and that merge
/// verification therefore never produces a manual deferral.
#[tokio::test]
async fn parallel_pass_to_archive_to_merge_never_creates_or_cleans_an_acceptance_checkpoint() {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");

    // The active change is part of the base commit so the dispatched worktree
    // and the base branch share a head; post-archive merge then exercises the
    // archive verification path rather than pre-sync handling.
    let change_id = "post-archive-clean";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(change_dir.join("proposal.md"), "# Post archive\n")
        .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write active tasks");
    init_git_repo(repo_dir.path()).await;
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some("sh -c 'true'".to_string()),
        acceptance_command: Some("sh -c 'echo ACCEPTANCE: PASS'".to_string()),
        archive_command: Some(format!(
            "sh -c 'mkdir -p openspec/changes/archive \
             && mv openspec/changes/{change_id} openspec/changes/archive/{change_id}'"
        )),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch change through the full pipeline");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    assert!(
        result.error.is_none(),
        "pass-to-archive pipeline must succeed: {:?}",
        result.error
    );
    assert!(
        result.final_revision.is_some(),
        "archive must produce a final revision"
    );

    let workspace = executor
        .workspace_manager
        .find_existing_workspace(change_id)
        .await
        .or_fail("look up archived workspace")
        .or_fail("archived workspace should still exist before merge");

    assert!(
        !workspace.path.join(".cflx/acceptance-state.json").exists(),
        "acceptance must never create a workspace checkpoint"
    );
    assert!(
        !repo_dir.path().join(".cflx/acceptance-state.json").exists(),
        "acceptance must never create a base-repository checkpoint"
    );

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("read post-archive worktree status");
    assert!(
        String::from_utf8_lossy(&status.stdout).trim().is_empty(),
        "post-archive worktree must be clean, got: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    assert!(
        crate::execution::archive::is_archive_commit_complete(change_id, Some(&workspace.path))
            .await
            .or_fail("verify archive completion"),
        "archive completion verification must succeed without checkpoint cleanup"
    );

    while rx.try_recv().is_ok() {}

    // Post-archive merge verification must clear the archive gate. The stub
    // resolve command in this fixture may still fail the later conflict phase;
    // what this regression asserts is that archive verification never produces
    // a manual deferral and that the change reaches the resolving stage.
    let attempt = executor
        .attempt_merge(
            std::slice::from_ref(&workspace.workspace_name),
            &[change_id.to_string()],
            std::slice::from_ref(&workspace.path),
        )
        .await;
    if let Ok(crate::parallel::merge::MergeAttempt::Deferred(deferred)) = &attempt {
        assert!(
            deferred.auto_resumable,
            "a valid archive must never enter manual MergeWait: {}",
            deferred.reason
        );
    }

    let mut reached_resolving = false;
    while let Ok(event) = rx.try_recv() {
        if let ParallelEvent::ResolveStarted { change_id: id, .. } = event {
            if id == change_id {
                reached_resolving = true;
            }
        }
    }
    assert!(
        reached_resolving,
        "archive verification must pass and hand the change to the resolving stage"
    );
}

#[tokio::test]
async fn test_resumed_workspace_marker_stops_parallel_dispatch_before_apply_acceptance_and_archive()
{
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    init_git_repo(repo_dir.path()).await;

    let change_id = "acceptance-stalled";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(change_dir.join("proposal.md"), "# Acceptance stalled\n")
        .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write active tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git add active change");
    Command::new("git")
        .args(["commit", "-m", "Add active change"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git commit active change");
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let workspace_path = workspace_base.path().join(format!("cflx-{change_id}"));
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            change_id,
            workspace_path.to_string_lossy().as_ref(),
            "HEAD",
        ])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("create resumed worktree");
    // Apply-origin markers keep their existing conservative blocked routing;
    // they are never migrated or consumed by acceptance.
    let marker_path = workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("APPLY_BLOCKED/marker.md");
    std::fs::create_dir_all(marker_path.parent().or_fail("marker parent"))
        .or_fail("create marker dir");
    std::fs::write(&marker_path, "origin: apply\nreason: apply blocked\n")
        .or_fail("write apply marker");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some("sh -c 'echo unexpected-apply >&2; exit 42'".to_string()),
        acceptance_command: Some("sh -c 'echo unexpected-acceptance >&2; exit 43'".to_string()),
        archive_command: Some("sh -c 'echo unexpected-archive >&2; exit 44'".to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch resumed blocked workspace");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    assert!(result.error.is_none());
    assert!(result.final_revision.is_none());
    assert!(
        crate::parallel::acceptance_state::parse_blocked_marker(&workspace_path, change_id)
            .or_fail("read preserved marker")
            .is_some()
    );

    let mut saw_apply_started = false;
    let mut saw_acceptance_started = false;
    let mut saw_archive_started = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                saw_apply_started = true;
            }
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                saw_acceptance_started = true;
            }
            ExecutionEvent::ArchiveStarted { change_id: id, .. } if id == change_id => {
                saw_archive_started = true;
            }
            _ => {}
        }
    }
    assert!(!saw_apply_started);
    assert!(!saw_acceptance_started);
    assert!(!saw_archive_started);
}

#[tokio::test]
async fn test_resumed_merged_leftover_worktree_does_not_emit_apply_or_acceptance_started() {
    use crate::events::ExecutionEvent;
    use tempfile::TempDir;
    use tokio::sync::{mpsc, Semaphore};
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    init_git_repo(repo_dir.path()).await;

    let change_id = "fix-dependency-target-handling";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(
        change_dir.join("proposal.md"),
        "# Fix dependency target handling\n",
    )
    .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write active tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git add active change");
    Command::new("git")
        .args(["commit", "-m", "Add active change"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git commit active change");
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let workspace_path = workspace_base.path().join(format!("cflx-{change_id}"));
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            change_id,
            workspace_path.to_string_lossy().as_ref(),
            "HEAD",
        ])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("create leftover worktree");

    std::fs::remove_dir_all(workspace_path.join("openspec/changes").join(change_id))
        .or_fail("remove active change dir in leftover worktree");
    let archive_dir =
        workspace_path.join("openspec/changes/archive/2026-05-08-fix-dependency-target-handling");
    std::fs::create_dir_all(&archive_dir).or_fail("create archive dir in leftover worktree");
    std::fs::write(
        archive_dir.join("proposal.md"),
        "# Fix dependency target handling\n",
    )
    .or_fail("write archived proposal in leftover worktree");
    std::fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write archived tasks in leftover worktree");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_path)
        .output()
        .await
        .or_fail("git add archive in leftover worktree");
    Command::new("git")
        .args(["commit", "-m", "Archive fix-dependency-target-handling"])
        .current_dir(&workspace_path)
        .output()
        .await
        .or_fail("git commit archive in leftover worktree");

    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("checkout base branch");
    Command::new("git")
        .args(["merge", "--ff-only", change_id])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("merge archive branch to base");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some("sh -c 'echo unexpected-apply >&2; exit 42'".to_string()),
        acceptance_command: Some("sh -c 'echo unexpected-acceptance >&2; exit 43'".to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch merged leftover worktree");

    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    assert!(
        result.error.is_none(),
        "merged leftover should be terminal no-op: {:?}",
        result.error
    );
    assert!(
        result.final_revision.is_none(),
        "merged leftover should not hand off another revision for merge"
    );

    let mut saw_apply_started = false;
    let mut saw_acceptance_started = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                saw_apply_started = true;
            }
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                saw_acceptance_started = true;
            }
            _ => {}
        }
    }

    assert!(
        !saw_apply_started,
        "merged leftover must not re-enter apply"
    );
    assert!(
        !saw_acceptance_started,
        "merged leftover must not re-enter acceptance"
    );
}

#[test]
fn test_stale_retry_reason_detects_deleted_workspace_path() {
    let temp_dir = TempDir::new().or_fail("create temp dir");
    let workspace_path = temp_dir.path().join("deleted-worktree");
    let workspace = WorkspaceInfo {
        path: workspace_path.clone(),
        change_id: "alpha".to_string(),
        workspace_name: "ws-alpha".to_string(),
        last_modified: std::time::SystemTime::now(),
    };

    let reason = ParallelExecutor::stale_retry_reason(&workspace).or_fail("stale reason");

    assert!(reason.contains(&workspace_path.display().to_string()));
}

#[test]
fn test_stale_retry_reason_allows_existing_workspace_path() {
    let temp_dir = TempDir::new().or_fail("create temp dir");
    let workspace = WorkspaceInfo {
        path: temp_dir.path().to_path_buf(),
        change_id: "alpha".to_string(),
        workspace_name: "ws-alpha".to_string(),
        last_modified: std::time::SystemTime::now(),
    };

    assert!(ParallelExecutor::stale_retry_reason(&workspace).is_none());
}

#[tokio::test]
async fn test_missing_workspace_retry_clears_resolve_wait_in_reducer() {
    use std::sync::Arc;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    init_git_repo(repo_dir.path()).await;

    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(
        repo_dir.path().to_path_buf(),
        create_test_config(),
        Some(tx),
    );
    executor.workspace_manager = Box::new(TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0))));

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "manual retry requested".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge("alpha".to_string()));
    }
    executor.set_shared_orchestrator_state(shared.clone());
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    executor.retry_deferred_merges().await;
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    assert!(
        executor.resolve_wait_changes.is_empty(),
        "missing archived workspace must not leave executor-local ResolveWait pending"
    );
    assert!(
        shared.read().await.resolve_wait_change_ids().is_empty(),
        "missing archived workspace must clear reducer-owned ResolveWait"
    );
    assert_ne!(
        shared.read().await.display_status("alpha"),
        "resolve pending",
        "TUI display must not remain indefinitely resolve pending after missing workspace handling"
    );

    let mut saw_retry_dispatch = false;
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::Log(log) = event {
            if log
                .message
                .contains("ResolveWait retry dispatch started for 'alpha'")
            {
                saw_retry_dispatch = true;
            }
        }
    }
    assert!(
        saw_retry_dispatch,
        "missing workspace path must still prove retry evaluation ran"
    );
}

#[tokio::test]
async fn test_stale_workspace_retry_clears_resolve_wait_in_reducer() {
    use std::sync::Arc;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    let stale_path = workspace_dir.path().join("deleted-workspace");
    init_git_repo(repo_dir.path()).await;

    let mut executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), create_test_config(), None);
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace("alpha", stale_path),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "manual retry requested".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge("alpha".to_string()));
    }
    executor.set_shared_orchestrator_state(shared.clone());
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    executor.retry_deferred_merges().await;
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    assert!(executor.resolve_wait_changes.is_empty());
    assert!(shared.read().await.resolve_wait_change_ids().is_empty());
    assert_ne!(
        shared.read().await.display_status("alpha"),
        "resolve pending"
    );
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_deferred_merge_success_clears_shared_resolve_wait_and_runs_hook_once() {
    use crate::hooks::{HookConfig, HookConfigValue, HookRunner, HooksConfig};
    use crate::vcs::GitWorkspaceManager;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let workspace_base = repo_root.join("worktrees");
    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    });
    let mut manager = GitWorkspaceManager::new(
        workspace_base.clone(),
        repo_root.to_path_buf(),
        1,
        config.clone(),
    );
    let workspace = manager
        .create_workspace("alpha", None)
        .await
        .or_fail("create workspace");
    let original_change_dir = workspace.path.join("openspec/changes/alpha");
    std::fs::create_dir_all(&original_change_dir).or_fail("create original change dir");
    std::fs::write(original_change_dir.join("proposal.md"), "# alpha draft\n")
        .or_fail("write original change file");
    std::fs::create_dir_all(workspace.path.join("openspec/changes/archive/alpha"))
        .or_fail("create archive dir");
    std::fs::write(
        workspace
            .path
            .join("openspec/changes/archive/alpha/proposal.md"),
        "# alpha\n",
    )
    .or_fail("write archive file");
    std::fs::remove_dir_all(&original_change_dir).or_fail("remove original change dir");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git add workspace archive");
    Command::new("git")
        .args(["commit", "-m", "Archive alpha"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git commit workspace archive");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("git add worktree gitfile");
    Command::new("git")
        .args(["commit", "-m", "Track alpha worktree"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("git commit worktree gitfile");

    let hook_marker = repo_root.join("hook-count.txt");
    let hook_command = format!("printf 'alpha\\n' >> {}", hook_marker.to_string_lossy());
    let hooks = HookRunner::new(
        HooksConfig {
            on_merged: Some(HookConfigValue::Full(HookConfig {
                command: hook_command,
                continue_on_failure: false,
                timeout: 5,
                git_commit_no_verify: false,
                max_retries: 0,
                retry_delay_secs: 1,
            })),
            ..Default::default()
        },
        repo_root,
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        0,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "Resolve in progress for another change".to_string(),
            auto_resumable: true,
        });
    }

    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(manager);
    executor.set_shared_orchestrator_state(shared.clone());
    executor.set_hooks(hooks);

    match (MergeAttempt::Merged {
        revision: "merge-rev-alpha".to_string(),
    }) {
        MergeAttempt::Merged { revision } => {
            executor
                .clear_resolve_wait_intent_for_outcome("alpha")
                .await;
            if let Some(ref hooks) = executor.hooks {
                let hook_ctx = crate::hooks::HookContext::new(0, 0, 0, false)
                    .with_change("alpha", 0, 0)
                    .with_apply_count(0)
                    .with_parallel_context(&workspace.path.to_string_lossy(), None);
                hooks
                    .run_hook(crate::hooks::HookType::OnMerged, &hook_ctx)
                    .await
                    .or_fail("on_merged hook should succeed");
            }
            executor
                .mark_deferred_merge_completed_in_shared_state("alpha", &revision)
                .await;
            crate::parallel::events::send_event(
                &executor.event_tx,
                ParallelEvent::MergeCompleted {
                    change_id: "alpha".to_string(),
                    revision,
                },
            )
            .await;
        }
        MergeAttempt::Deferred(deferred) => {
            panic!("expected merge success, got deferred: {}", deferred.reason);
        }
    }
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    assert!(executor.resolve_wait_changes.is_empty());
    assert!(!executor.has_resolve_wait());
    assert!(shared.read().await.resolve_wait_change_ids().is_empty());

    executor.trigger_resolve_wait_retry_dispatch();
    executor.maybe_dispatch_resolve_wait_retry().await;

    let hook_output = std::fs::read_to_string(&hook_marker).or_fail("read hook marker");
    assert_eq!(
        hook_output.lines().filter(|line| *line == "alpha").count(),
        1,
        "on_merged hook must run exactly once for deferred retry success"
    );

    let mut merge_completed = 0usize;
    let mut resolve_started = 0usize;
    while let Ok(event) = rx.try_recv() {
        match event {
            crate::events::ExecutionEvent::MergeCompleted { change_id, .. }
                if change_id == "alpha" =>
            {
                merge_completed += 1;
            }
            crate::events::ExecutionEvent::ResolveStarted { change_id, .. }
                if change_id == "alpha" =>
            {
                resolve_started += 1;
            }
            _ => {}
        }
    }
    assert_eq!(merge_completed, 1);
    assert_eq!(resolve_started, 0);
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_stale_already_merged_resolve_wait_skips_merge_and_hook() {
    use crate::hooks::{HookConfig, HookConfigValue, HookRunner, HooksConfig};
    use crate::vcs::GitWorkspaceManager;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let workspace_base = repo_root.join("worktrees");
    init_git_repo(repo_root).await;

    std::fs::create_dir_all(repo_root.join("openspec/changes/archive/alpha"))
        .or_fail("create archive dir");
    std::fs::write(
        repo_root.join("openspec/changes/archive/alpha/proposal.md"),
        "# alpha\n",
    )
    .or_fail("write archive file");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("git add base archive");
    Command::new("git")
        .args(["commit", "-m", "Archive alpha on base"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("git commit base archive");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    });
    let manager = GitWorkspaceManager::new(
        workspace_base.clone(),
        repo_root.to_path_buf(),
        1,
        config.clone(),
    );

    let hook_marker = repo_root.join("stale-hook-count.txt");
    let hook_command = format!("printf 'alpha\\n' >> {}", hook_marker.to_string_lossy());
    let hooks = HookRunner::new(
        HooksConfig {
            on_merged: Some(HookConfigValue::Full(HookConfig {
                command: hook_command,
                continue_on_failure: false,
                timeout: 5,
                git_commit_no_verify: false,
                max_retries: 0,
                retry_delay_secs: 1,
            })),
            ..Default::default()
        },
        repo_root,
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["alpha".to_string()],
        0,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "Resolve in progress for another change".to_string(),
            auto_resumable: true,
        });
    }

    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(manager);
    executor.set_shared_orchestrator_state(shared.clone());
    executor.set_hooks(hooks);

    executor.retry_deferred_merges().await;
    executor.sync_resolve_wait_from_shared_state_nonblocking();

    assert!(executor.resolve_wait_changes.is_empty());
    assert!(!executor.has_resolve_wait());
    assert!(shared.read().await.resolve_wait_change_ids().is_empty());
    assert!(
        !hook_marker.exists(),
        "stale already-merged retry must not run on_merged hook"
    );

    while let Ok(event) = rx.try_recv() {
        match event {
            crate::events::ExecutionEvent::MergeStarted { .. }
            | crate::events::ExecutionEvent::ResolveStarted { .. }
            | crate::events::ExecutionEvent::MergeCompleted { .. } => {
                panic!("stale already-merged retry must not emit merge/resolve event: {event:?}");
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_resumed_archived_dispatch_clears_reducer_queue_intent() {
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;
    use tokio::task::JoinSet;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    init_git_repo(repo_dir.path()).await;

    let change_id = "improve-warning-popup-readability";
    let change_dir = repo_dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("create active change dir");
    std::fs::write(
        change_dir.join("proposal.md"),
        "# Improve warning popup readability\n",
    )
    .or_fail("write active proposal");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write active tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git add active change");
    Command::new("git")
        .args(["commit", "-m", "Add active change"])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("git commit active change");
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let workspace_path = workspace_base.path().join(format!("cflx-{change_id}"));
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            change_id,
            workspace_path.to_string_lossy().as_ref(),
            "HEAD",
        ])
        .current_dir(repo_dir.path())
        .output()
        .await
        .or_fail("create archived worktree");
    std::fs::remove_dir_all(workspace_path.join("openspec/changes").join(change_id))
        .or_fail("remove active change dir in worktree");
    let archive_dir = workspace_path
        .join("openspec/changes/archive/2026-05-08-improve-warning-popup-readability");
    std::fs::create_dir_all(&archive_dir).or_fail("create archive dir in worktree");
    std::fs::write(
        archive_dir.join("proposal.md"),
        "# Improve warning popup readability\n",
    )
    .or_fail("write archived proposal");
    std::fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] done\n",
    )
    .or_fail("write archived tasks");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_path)
        .output()
        .await
        .or_fail("git add archive in worktree");
    Command::new("git")
        .args(["commit", "-m", "Archive improve-warning-popup-readability"])
        .current_dir(&workspace_path)
        .output()
        .await
        .or_fail("git commit archive in worktree");

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![change_id.to_string()],
        1,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
            change_id.to_string(),
        ));
    }

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some("sh -c 'echo unexpected-apply >&2; exit 42'".to_string()),
        acceptance_command: Some("sh -c 'echo unexpected-acceptance >&2; exit 43'".to_string()),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(128);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    executor.set_shared_orchestrator_state(shared.clone());
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch archived worktree");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    assert!(result.error.is_none());
    assert!(result.final_revision.is_some());
    assert!(
        shared.read().await.queued_change_ids().is_empty(),
        "resumed Archived dispatch must clear reducer queued intent before scheduler reconciliation can re-add it"
    );

    let mut saw_apply_started = false;
    let mut saw_archived = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                saw_apply_started = true;
            }
            ExecutionEvent::ChangeArchived(id) if id == change_id => {
                saw_archived = true;
            }
            _ => {}
        }
    }
    assert!(!saw_apply_started);
    assert!(saw_archived);
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn push_post_archive_success_cleans_workspace_and_does_not_merge_base() {
    use crate::parallel::WorkspaceResult;
    use crate::vcs::GitWorkspaceManager;

    let repo = TempDir::new().or_fail("create repo tempdir");
    let remote = TempDir::new().or_fail("create remote tempdir");
    let workspace_base = repo.path().join("worktrees");
    init_git_repo(repo.path()).await;
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(remote.path())
        .output()
        .await
        .or_fail("init bare remote");
    Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            remote.path().to_str().or_fail("remote path utf8"),
        ])
        .current_dir(repo.path())
        .output()
        .await
        .or_fail("add origin remote");

    let base_head = get_current_commit(repo.path())
        .await
        .or_fail("read base head before push");
    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    });
    let mut manager = GitWorkspaceManager::new(
        workspace_base.clone(),
        repo.path().to_path_buf(),
        1,
        config.clone(),
    );
    let workspace = manager
        .create_workspace("alpha", None)
        .await
        .or_fail("create alpha workspace");
    std::fs::create_dir_all(workspace.path.join("openspec/changes/archive/alpha"))
        .or_fail("create archive dir");
    std::fs::write(
        workspace
            .path
            .join("openspec/changes/archive/alpha/proposal.md"),
        "# alpha\n",
    )
    .or_fail("write archive marker");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git add archive in workspace");
    Command::new("git")
        .args(["commit", "-m", "Archive alpha"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git commit archive in workspace");
    let workspace_head = get_current_commit(&workspace.path)
        .await
        .or_fail("read workspace head");

    let (tx, mut rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(repo.path().to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(manager);
    executor.post_archive_action = PostArchiveAction::PushToRemote {
        remote: "origin".to_string(),
    };

    let outcome = executor
        .handle_merge_and_cleanup(WorkspaceResult {
            change_id: "alpha".to_string(),
            workspace_name: workspace.name.clone(),
            final_revision: Some(workspace_head.clone()),
            error: None,
            rejected: None,
        })
        .await
        .or_fail("push post-archive should succeed");
    assert_eq!(outcome, MergeTaskOutcome::Merged);
    assert_eq!(
        get_current_commit(repo.path())
            .await
            .or_fail("read base head after push"),
        base_head,
        "push mode must not merge into base"
    );
    assert!(
        !workspace.path.exists(),
        "successful push should cleanup worktree"
    );
    let remote_head =
        crate::vcs::git::commands::run_git(&["rev-parse", "refs/heads/alpha"], remote.path())
            .await
            .or_fail("read pushed remote branch");
    assert_eq!(remote_head, workspace_head);

    let mut saw_push_completed = false;
    let mut saw_merge_completed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::PushCompleted { change_id, .. } if change_id == "alpha" => {
                saw_push_completed = true;
            }
            ExecutionEvent::MergeCompleted { change_id, .. } if change_id == "alpha" => {
                saw_merge_completed = true;
            }
            _ => {}
        }
    }
    assert!(saw_push_completed);
    assert!(!saw_merge_completed);
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn push_post_archive_failure_preserves_workspace_and_skips_on_merged_hook() {
    use crate::hooks::{HookConfig, HookConfigValue, HookRunner, HooksConfig};
    use crate::parallel::WorkspaceResult;
    use crate::vcs::GitWorkspaceManager;

    let repo = TempDir::new().or_fail("create repo tempdir");
    let workspace_base = repo.path().join("worktrees");
    init_git_repo(repo.path()).await;
    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    });
    let mut manager = GitWorkspaceManager::new(
        workspace_base.clone(),
        repo.path().to_path_buf(),
        1,
        config.clone(),
    );
    let workspace = manager
        .create_workspace("alpha", None)
        .await
        .or_fail("create alpha workspace");
    std::fs::create_dir_all(workspace.path.join("openspec/changes/archive/alpha"))
        .or_fail("create archive dir");
    std::fs::write(
        workspace
            .path
            .join("openspec/changes/archive/alpha/proposal.md"),
        "# alpha\n",
    )
    .or_fail("write archive marker");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git add archive in workspace");
    Command::new("git")
        .args(["commit", "-m", "Archive alpha"])
        .current_dir(&workspace.path)
        .output()
        .await
        .or_fail("git commit archive in workspace");
    let workspace_head = get_current_commit(&workspace.path)
        .await
        .or_fail("read workspace head");
    let hook_marker = repo.path().join("on-merged-ran.txt");
    let hooks = HookRunner::new(
        HooksConfig {
            on_merged: Some(HookConfigValue::Full(HookConfig {
                command: format!("touch {}", hook_marker.to_string_lossy()),
                continue_on_failure: false,
                timeout: 5,
                git_commit_no_verify: false,
                max_retries: 0,
                retry_delay_secs: 1,
            })),
            ..Default::default()
        },
        repo.path(),
    );
    let (tx, mut rx) = mpsc::channel(16);
    let mut executor = ParallelExecutor::new(repo.path().to_path_buf(), config, Some(tx));
    executor.workspace_manager = Box::new(manager);
    executor.set_hooks(hooks);
    executor.post_archive_action = PostArchiveAction::PushToRemote {
        remote: "missing-remote".to_string(),
    };

    let error = executor
        .handle_merge_and_cleanup(WorkspaceResult {
            change_id: "alpha".to_string(),
            workspace_name: workspace.name.clone(),
            final_revision: Some(workspace_head),
            error: None,
            rejected: None,
        })
        .await
        .expect_err("push to missing remote should fail");
    assert!(error.to_string().contains("Failed to push archived alpha"));
    assert!(
        workspace.path.exists(),
        "failed push must preserve worktree for inspection/retry"
    );
    assert!(
        !hook_marker.exists(),
        "push mode must not execute on_merged hooks"
    );

    let mut saw_push_failed = false;
    let mut saw_hook_failed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::PushFailed { change_id, .. } if change_id == "alpha" => {
                saw_push_failed = true;
            }
            ExecutionEvent::HookFailed { change_id, .. } if change_id == "alpha" => {
                saw_hook_failed = true;
            }
            _ => {}
        }
    }
    assert!(saw_push_failed);
    assert!(!saw_hook_failed);
}

#[tokio::test]
async fn test_reject_wait_lane_clear_promotion_starts_rejection_review() {
    use crate::events::ExecutionEvent;
    use std::sync::Arc;
    use tempfile::TempDir;

    use tokio::sync::mpsc;

    let workspace_dir = TempDir::new().or_fail("unexpected error");
    let change_id = "change-rejected";
    let rejected_dir = workspace_dir
        .path()
        .join("openspec")
        .join("changes")
        .join(change_id);
    std::fs::create_dir_all(&rejected_dir).or_fail("unexpected error");
    std::fs::write(
        rejected_dir.join("REJECTED.md"),
        "# REJECTED\n\n- change_id: change-rejected\n- reason: regression\n",
    )
    .or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo REJECTION_REVIEW: BLOCK'".to_string()),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_strict_process_cleanup: Some(false),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(PathBuf::from("/tmp/test-repo"), config, Some(tx));
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace(change_id, workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["lane-owner".to_string(), change_id.to_string()],
        2,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::WorkspaceStatusUpdated {
            change_id: "lane-owner".to_string(),
            workspace_name: "lane-owner".to_string(),
            status: WorkspaceStatus::Resolving,
        });
        guard.mark_reject_wait(change_id);
        assert_eq!(guard.display_status(change_id), "reject pending");
        guard.apply_execution_event(&ExecutionEvent::ResolveCompleted {
            change_id: "lane-owner".to_string(),
            worktree_change_ids: None,
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    executor.retry_deferred_base_lane_waiters().await;

    let mut saw_rejecting_status = false;
    let mut saw_review_completed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::WorkspaceStatusUpdated {
                change_id: id,
                status: WorkspaceStatus::Rejecting,
                ..
            } if id == change_id => saw_rejecting_status = true,
            ExecutionEvent::RejectionReviewCompleted { change_id: id, .. }
            | ExecutionEvent::RejectionReviewFailed { change_id: id, .. }
                if id == change_id =>
            {
                saw_review_completed = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_rejecting_status,
        "lane-clear promotion must emit active rejecting status before running review"
    );
    assert!(
        saw_review_completed,
        "lane-clear promotion must execute the deferred rejection review"
    );
    let guard = shared.read().await;
    assert!(guard.reject_wait_change_ids().is_empty());
    let final_status = guard.display_status(change_id);
    assert!(
        matches!(final_status, "stalled" | "error"),
        "the deferred review attempt must leave reject pending after execution"
    );
}

#[tokio::test]
async fn test_reject_wait_lane_clear_promotes_only_one_waiter() {
    use crate::events::ExecutionEvent;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let first_workspace = TempDir::new().or_fail("unexpected error");
    let second_workspace = TempDir::new().or_fail("unexpected error");
    let first_id = "change-rejected-a";
    let second_id = "change-rejected-b";
    for (change_id, workspace) in [
        (first_id, first_workspace.path()),
        (second_id, second_workspace.path()),
    ] {
        let rejected_dir = workspace.join("openspec").join("changes").join(change_id);
        std::fs::create_dir_all(&rejected_dir).or_fail("unexpected error");
        std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED\n")
            .or_fail("unexpected error");
    }

    let config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo REJECTION_REVIEW: BLOCK'".to_string()),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_strict_process_cleanup: Some(false),
        ..Default::default()
    });
    let (tx, mut rx) = mpsc::channel(64);
    let mut executor = ParallelExecutor::new(PathBuf::from("/tmp/test-repo"), config, Some(tx));
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace(first_id, first_workspace.path().to_path_buf())
            .with_existing_workspace(second_id, second_workspace.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec![
            "lane-owner".to_string(),
            first_id.to_string(),
            second_id.to_string(),
        ],
        2,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::WorkspaceStatusUpdated {
            change_id: "lane-owner".to_string(),
            workspace_name: "lane-owner".to_string(),
            status: WorkspaceStatus::Resolving,
        });
        guard.mark_reject_wait(first_id);
        guard.mark_reject_wait(second_id);
        guard.apply_execution_event(&ExecutionEvent::ResolveCompleted {
            change_id: "lane-owner".to_string(),
            worktree_change_ids: None,
        });
    }
    executor.set_shared_orchestrator_state(shared.clone());

    executor.retry_deferred_base_lane_waiters().await;

    let mut rejecting_updates = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let ExecutionEvent::WorkspaceStatusUpdated {
            change_id,
            status: WorkspaceStatus::Rejecting,
            ..
        } = event
        {
            rejecting_updates.push(change_id);
        }
    }

    assert_eq!(rejecting_updates, vec![first_id.to_string()]);
    let guard = shared.read().await;
    let first_status = guard.display_status(first_id);
    assert!(
        matches!(first_status, "stalled" | "error"),
        "the promoted first waiter must leave reject pending after its review attempt"
    );
    assert_eq!(guard.display_status(second_id), "reject pending");
    assert_eq!(guard.reject_wait_change_ids(), vec![second_id.to_string()]);
}

#[tokio::test]
async fn test_scheduler_does_not_busy_retry_unchanged_resolve_wait() {
    use std::sync::Arc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        crate::orchestration::state::ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_observation(
            "change-a",
            crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
        );
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
    }
    executor.set_shared_orchestrator_state(shared);

    executor.sync_resolve_wait_from_shared_state_nonblocking();
    executor.trigger_resolve_wait_retry_dispatch();
    executor.maybe_dispatch_resolve_wait_retry().await;

    let dispatched_snapshot = executor.resolve_wait_changes.clone();
    assert_eq!(
        executor.last_dispatched_resolve_wait_changes,
        dispatched_snapshot
    );
    assert!(!executor.resolve_wait_retry_triggered);
    assert!(
        !executor.should_dispatch_resolve_wait_retry(),
        "unchanged resolve-wait intent must not be retried again without a new trigger"
    );
}

#[tokio::test]
async fn test_dirty_to_clean_resolve_wait_wakes_retry_without_new_trigger() {
    use std::sync::Arc;
    use tempfile::TempDir;

    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_dir = TempDir::new().or_fail("create temp workspace");
    init_git_repo(repo_dir.path()).await;
    std::fs::write(repo_dir.path().join("dirty.txt"), "dirty").or_fail("dirty base");

    let config = create_test_config();
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, None);
    executor.workspace_manager = Box::new(
        TestWorkspaceManager::new(Arc::new(AtomicUsize::new(0)))
            .with_existing_workspace("change-a", workspace_dir.path().to_path_buf()),
    );

    let shared = Arc::new(RwLock::new(OrchestratorState::with_mode(
        vec!["change-a".to_string()],
        3,
        ExecutionMode::Parallel,
    )));
    {
        let mut guard = shared.write().await;
        guard.apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "change-a".to_string(),
            reason: "base dirty".to_string(),
            auto_resumable: false,
        });
        guard.apply_command(ReducerCommand::ResolveMerge("change-a".to_string()));
    }
    executor.set_shared_orchestrator_state(shared);
    executor.sync_resolve_wait_from_shared_state_nonblocking();
    executor.last_dispatched_resolve_wait_changes = executor.resolve_wait_changes.clone();

    executor.maybe_dispatch_resolve_wait_retry().await;
    assert!(
        !executor.should_dispatch_resolve_wait_retry(),
        "dirty observation should be deduped after the first scheduler evaluation"
    );

    std::fs::remove_file(repo_dir.path().join("dirty.txt")).or_fail("clean base");

    executor.maybe_dispatch_resolve_wait_retry().await;

    assert_eq!(
        executor.last_dispatched_resolve_wait_changes, executor.resolve_wait_changes,
        "dirty-to-clean transition should wake scheduler-owned ResolveWait retry without another M keypress"
    );
    assert!(!executor.resolve_wait_retry_triggered);
}

#[test]
fn test_resolve_wait_helper_tracks_state() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    assert!(!executor.has_resolve_wait());

    executor
        .resolve_wait_changes
        .insert("test-change".to_string());

    assert!(executor.has_resolve_wait());

    executor.resolve_wait_changes.clear();

    assert!(!executor.has_resolve_wait());
}

#[test]
fn test_auto_resumable_deferral_uses_resolve_pending_not_manual_merge_wait() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    executor.resolve_wait_changes.insert("change-a".to_string());
    executor.merge_wait_changes.remove("change-a");

    assert!(executor.resolve_wait_changes.contains("change-a"));
    assert!(executor.merge_wait_changes.is_empty());
    assert!(executor.has_resolve_wait());
}

#[test]
fn test_manual_deferral_uses_merge_wait_not_resolve_pending() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    executor.resolve_wait_changes.insert("change-a".to_string());
    executor.resolve_wait_changes.remove("change-a");
    executor.merge_wait_changes.insert("change-a".to_string());

    assert!(!executor.resolve_wait_changes.contains("change-a"));
    assert!(executor.merge_wait_changes.contains("change-a"));
    assert!(!executor.has_resolve_wait());
}

/// Test that changes in MergeWait state are correctly filtered during loop iteration.
/// This test validates the spec requirement:
/// "The loop continues processing runnable changes and MergeWait is not treated as a terminal completion reason."
#[test]
fn test_merge_wait_does_not_block_runnable_changes() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    // MergeWait は scheduler break 条件に含まれないため、
    // ResolveWait が空なら completion 判定に影響しない。
    executor
        .merge_wait_changes
        .insert("merge-wait-only".to_string());
    assert!(executor.resolve_wait_changes.is_empty());
    assert!(!executor.has_resolve_wait());
}

/// Test concurrent re-analysis: verify that re-analysis reason is properly tracked
/// and logged during execution.
///
/// This test validates task 2.1 requirement:
/// - Initial analysis has reason "initial"
/// - Completion triggers have reason "completion"
/// - Queue notifications have reason "queue"
#[tokio::test]
async fn test_concurrent_reanalysis_queue_dispatch() {
    use crate::tui::queue::DynamicQueue;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, _rx) = mpsc::channel(100);

    // Create executor with dynamic queue
    let queue = Arc::new(DynamicQueue::new());
    let mut executor = ParallelExecutor::new(repo_root.clone(), config.clone(), Some(tx));
    executor.set_dynamic_queue(queue.clone());

    // Add initial change to queue (will trigger queue notification)
    queue.push("test-change".to_string()).await;

    // Verify queue has one item
    assert_eq!(queue.len().await, 1);

    // Verify executor is set up correctly
    assert!(executor.dynamic_queue.is_some());

    // Test debounce logic
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    // Immediate check: should NOT reanalyze (debounce active)
    assert!(!executor.should_reanalyze(false).await);

    // Simulate debounce period expiry without waiting 11 real seconds.
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now() - std::time::Duration::from_secs(11));
    }

    // After simulated debounce expiry: should reanalyze
    assert!(executor.should_reanalyze(false).await);

    // Verify AnalysisStarted event would be emitted
    // (Full execution test would require mocking apply/archive commands)
}

/// Test that on_merged hook is executed when parallel merge succeeds
#[tokio::test]
async fn test_on_merged_hook_execution() {
    use crate::hooks::{HookConfig, HookRunner, HooksConfig};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path().to_path_buf();

    // Create a marker file path to verify hook execution
    let marker_file = repo_root.join("hook_executed.marker");
    let marker_file_str = marker_file.to_string_lossy().to_string();

    // Set up hooks configuration with on_merged hook that creates a marker file
    let hook_command = if cfg!(target_os = "windows") {
        format!("cmd /C echo executed > {}", marker_file_str)
    } else {
        format!("touch {}", marker_file_str)
    };

    let hooks_config = HooksConfig {
        on_merged: Some(crate::hooks::HookConfigValue::Full(HookConfig {
            command: hook_command,
            continue_on_failure: true,
            timeout: 5,
            git_commit_no_verify: false,
            max_retries: 0,
            retry_delay_secs: 3,
        })),
        ..Default::default()
    };

    let hook_runner = HookRunner::new(hooks_config, ".");

    // Create a simple HookContext for testing
    let hook_context = crate::hooks::HookContext::new(1, 1, 0, false)
        .with_change("test-change", 5, 5)
        .with_parallel_context("/tmp/test-workspace", None);

    // Execute the hook
    let result = hook_runner
        .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
        .await;
    assert!(result.is_ok(), "Hook execution should succeed");

    // Allow some time for file creation
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify the marker file was created
    assert!(
        marker_file.exists(),
        "Hook marker file should exist at {:?}",
        marker_file
    );
}

/// Test that attempt_merge defers when worktree is dirty
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_attempt_merge_deferred_when_resolve_active() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(10);
    let mut executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    // Simulate an already running manual resolve for another change.
    let manual_resolve_counter = Arc::new(AtomicUsize::new(1));
    executor.set_manual_resolve_counter(manual_resolve_counter.clone());

    let revisions = vec!["test-workspace".to_string()];
    let change_ids = vec!["test-change".to_string()];
    let archive_paths = vec![repo_root.to_path_buf()];

    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Deferred(deferred)) => {
            assert!(deferred.auto_resumable);
            assert!(
                deferred.reason.contains("Resolve in progress"),
                "Expected deferred reason to mention resolve in progress, got: {}",
                deferred.reason
            );
        }
        Ok(MergeAttempt::Merged { .. }) => {
            panic!("Merge should have been deferred while resolve is active");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }

    manual_resolve_counter.store(0, Ordering::SeqCst);
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_deferred_when_worktree_dirty() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create archive directory to simulate that archive was successful (change moved to archive)
    let archive_dir = repo_root.join("openspec/changes/archive/2024-01-01-test-change");
    fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    fs::write(archive_dir.join("spec.md"), "# Archived Test").or_fail("unexpected error");

    // Commit the archive
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create a dirty file (uncommitted change)
    fs::write(repo_root.join("dirty.txt"), "dirty content").or_fail("unexpected error");

    // Create executor
    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["test-workspace".to_string()];
    let change_ids = vec!["test-change".to_string()];
    let archive_paths = vec![repo_root.to_path_buf()];

    // Attempt merge should be deferred because worktree is dirty
    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Deferred(deferred)) => {
            assert!(!deferred.auto_resumable);
            assert!(
                deferred.reason.contains("incomplete") || deferred.reason.contains("dirty"),
                "Expected deferred reason to mention incomplete archive or dirty worktree, got: {}",
                deferred.reason
            );
        }
        Ok(MergeAttempt::Merged { .. }) => {
            panic!("Merge should have been deferred due to dirty worktree");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test that attempt_merge defers when archive entry is missing
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_deferred_when_archive_entry_missing() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Note: No archive directory created - this simulates archive entry missing
    // And no openspec/changes/test-change directory (simulating change was removed but not archived)

    // Create executor
    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["test-workspace".to_string()];
    let change_ids = vec!["test-change".to_string()];
    let archive_paths = vec![repo_root.to_path_buf()];

    // Attempt merge should be deferred because archive entry is missing
    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Deferred(deferred)) => {
            assert!(!deferred.auto_resumable);
            assert!(
                deferred.reason.contains("incomplete")
                    || deferred.reason.contains("archive")
                    || deferred.reason.contains("missing"),
                "Expected deferred reason to mention incomplete archive or missing entry, got: {}",
                deferred.reason
            );
        }
        Ok(MergeAttempt::Merged { .. }) => {
            panic!("Merge should have been deferred due to missing archive entry");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test that attempt_merge proceeds when archive is complete
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_proceeds_when_archive_complete() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create archive directory but NOT openspec/changes/test-change (proper archive)
    let archive_dir = repo_root.join("openspec/changes/archive/test-change");
    fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    fs::write(archive_dir.join("spec.md"), "# Archived").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create worktree for the change (outside the main repo to avoid dirty working tree)
    let workspace_base = TempDir::new().or_fail("unexpected error");
    let workspace_path = workspace_base.path().join("ws-test-change");

    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "ws-test-change",
            workspace_path.to_str().or_fail("unexpected error"),
            "HEAD",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create executor
    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["ws-test-change".to_string()];
    let change_ids = vec!["test-change".to_string()];

    // Attempt merge should succeed because change is properly archived
    let archive_paths = vec![workspace_path.clone()];
    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Merged { .. }) => {
            // Success - merge was allowed
        }
        Ok(MergeAttempt::Deferred(deferred)) => {
            panic!(
                "Merge should have succeeded when change is archived, got deferred: {}",
                deferred.reason
            );
        }
        Err(e) => {
            // This is also acceptable - merge may fail for other reasons (e.g., merge conflicts)
            // but it should not be deferred due to archive verification.
            println!("Merge failed with error (acceptable): {}", e);
        }
    }
}

/// Regression: detached HEAD must be reported as execution error, not MergeWait/deferred.
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_attempt_merge_errors_on_detached_head() {
    let _merge_lock_test_guard = merge_lock_test_mutex().lock().await;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();

    // Initialize git repo
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

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create archive directory but NOT openspec/changes/test-change (proper archive)
    let archive_dir = repo_root.join("openspec/changes/archive/test-change");
    fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    fs::write(archive_dir.join("spec.md"), "# Archived").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Detach HEAD explicitly
    let detached_rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    let detached_rev = String::from_utf8_lossy(&detached_rev.stdout)
        .trim()
        .to_string();
    Command::new("git")
        .args(["checkout", detached_rev.as_str()])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create worktree for the change (outside the main repo to avoid dirty working tree)
    let workspace_base = TempDir::new().or_fail("unexpected error");
    let workspace_path = workspace_base.path().join("ws-test-change");

    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "ws-test-change",
            workspace_path.to_str().or_fail("unexpected error"),
            "HEAD",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");

    // Create executor
    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        ..Default::default()
    });
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root.to_path_buf(), config, Some(tx));

    let revisions = vec!["ws-test-change".to_string()];
    let change_ids = vec!["test-change".to_string()];
    let archive_paths = vec![workspace_path.clone()];

    let result = executor
        .attempt_merge(&revisions, &change_ids, &archive_paths)
        .await;

    match result {
        Ok(MergeAttempt::Deferred(deferred)) => {
            panic!(
                "Detached HEAD must not become MergeDeferred: {}",
                deferred.reason
            );
        }
        Ok(MergeAttempt::Merged { revision }) => {
            panic!("Detached HEAD must not merge successfully: {}", revision);
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Detached HEAD state detected"),
                "Expected detached HEAD error, got: {}",
                msg
            );
        }
    }
}

/// Regression: when an acceptance command emits a canonical standalone verdict
/// but the child process keeps stdout open (e.g. opencode + MCP children),
/// the executor MUST finalize the verdict via the grace period and terminate
/// the child instead of waiting for the inactivity-timeout retry.
///
/// The test shortens the grace period to 1 second using a task-local override
/// (so concurrent tests are not affected) and uses an acceptance command that
/// sleeps for 30 seconds after emitting `ACCEPTANCE: PASS`. The execution
/// must complete in well under the sleep duration and must return PASS.
#[tokio::test]
async fn test_acceptance_finalizes_on_standalone_verdict_without_inactivity_retry() {
    use std::time::{Duration, Instant};
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        // Emit canonical standalone PASS, then keep the process alive long
        // enough that the executor MUST cut it off via the grace period.
        acceptance_command: Some("sh -c 'echo ACCEPTANCE: PASS; sleep 30'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        // 5s inactivity timeout would otherwise also rescue this test, so
        // disable it to ensure we are exercising the verdict-grace path only.
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let started = Instant::now();
    let (result, _iteration) = crate::parallel::executor::scoped_verdict_grace_secs_for_test(
        1,
        execute_acceptance_in_workspace(
            "change-a",
            repo_root.path(),
            &mut agent,
            None,
            None,
            &ai_runner,
            &acceptance_config,
            &acceptance_tail_injected,
            &acceptance_history,
            Some("main"),
            None,
        ),
    )
    .await
    .or_fail("unexpected error");
    let elapsed = started.elapsed();

    assert!(
        matches!(result, crate::orchestration::AcceptanceResult::Pass),
        "expected acceptance PASS via verdict grace, got {:?}",
        result
    );
    assert!(
        elapsed < Duration::from_secs(15),
        "verdict-grace finalization should complete well under the 30s sleep, took {:?}",
        elapsed
    );

    let report_path = repo_root.path().join("ACCEPTANCE_REPORT.json");
    assert!(
        !report_path.exists(),
        "verdict-finalized PASS must record acceptance history without creating {}",
        report_path.display()
    );

    let history = acceptance_history.lock().await;
    let attempts = history
        .get("change-a")
        .expect("verdict-finalized PASS must be recorded in acceptance history");
    assert_eq!(attempts.len(), 1);
    assert!(attempts[0].passed);
    assert!(
        attempts[0]
            .stdout_tail
            .as_deref()
            .unwrap_or_default()
            .contains("ACCEPTANCE: PASS"),
        "acceptance history should retain PASS stdout tail, got {:?}",
        attempts[0].stdout_tail
    );
    assert!(
        attempts[0].commit_hash.is_some(),
        "acceptance history should retain final revision"
    );
}

/// Regression: malformed trailing-text verdicts (e.g. `ACCEPTANCE: PASSAll ...`)
/// MUST NOT be treated as canonical PASS by the parallel executor. The legacy
/// `starts_with` check accepted these and could lock in a bogus pass; the
/// strict canonical contract falls through to CONTINUE so the loop can retry.
#[tokio::test]
async fn test_acceptance_command_failure_does_not_create_acceptance_report() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(
            "sh -c 'echo command-failed-before-verdict; echo stderr-tail >&2; exit 42'".to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    assert!(
        matches!(
            result,
            crate::orchestration::AcceptanceResult::CommandFailed { .. }
        ),
        "failing acceptance command should return CommandFailed, got {:?}",
        result
    );
    let report_path = repo_root.path().join("ACCEPTANCE_REPORT.json");
    assert!(
        !report_path.exists(),
        "command-failure acceptance must not create misleading {}",
        report_path.display()
    );

    let history = acceptance_history.lock().await;
    let attempts = history
        .get("change-a")
        .expect("command-failure acceptance must be recorded in history");
    assert_eq!(attempts.len(), 1);
    assert!(!attempts[0].passed);
    assert_eq!(attempts[0].exit_code, Some(42));
    assert!(
        attempts[0]
            .stdout_tail
            .as_deref()
            .unwrap_or_default()
            .contains("command-failed-before-verdict"),
        "acceptance history should retain command-failure stdout tail, got {:?}",
        attempts[0].stdout_tail
    );
}

#[tokio::test]
async fn test_acceptance_cancels_while_waiting_for_silent_streaming_output() {
    use std::time::{Duration, Instant};

    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'sleep 30'".to_string()),
        ..Default::default()
    });
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        cancel_clone.cancel();
    });

    let started = Instant::now();
    let (result, _iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        Some(&cancel),
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("acceptance cancellation should return a result");

    assert!(
        matches!(result, crate::orchestration::AcceptanceResult::Cancelled),
        "silent acceptance command should cancel instead of waiting for output, got {:?}",
        result
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancellation should not wait for the 30s child sleep"
    );
}

#[tokio::test]
async fn test_archive_cancels_while_waiting_for_silent_streaming_output() {
    use std::time::{Duration, Instant};

    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    let change_id = "change-a";
    let change_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] done\n",
    )
    .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        archive_command: Some("sh -c 'sleep 30'".to_string()),
        ..Default::default()
    });
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let archive_history = Arc::new(Mutex::new(crate::history::ArchiveHistory::new()));
    let apply_history = Arc::new(Mutex::new(crate::history::ApplyHistory::new()));
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        cancel_clone.cancel();
    });

    let started = Instant::now();
    let result = execute_archive_in_workspace(
        change_id,
        repo_root.path(),
        config.get_archive_command().or_fail("unexpected error"),
        &config,
        None,
        VcsBackend::Git,
        None,
        None,
        Some(&cancel),
        &ai_runner,
        &archive_history,
        &apply_history,
        &shared_stagger_state,
    )
    .await;

    let err = result.expect_err("silent archive command should be cancelled");
    assert!(
        err.to_string().contains("Cancelled archive"),
        "expected archive cancellation error, got {err}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancellation should not wait for the 30s child sleep"
    );
}

#[tokio::test]
async fn test_acceptance_trailing_text_pass_is_not_canonical() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(
            "sh -c 'echo ACCEPTANCE: PASSAll acceptance criteria verified'".to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    assert!(
        matches!(
            result,
            crate::orchestration::AcceptanceResult::MissingVerdict { .. }
        ),
        "trailing-text PASS must NOT satisfy canonical verdict; expected missing-verdict protocol failure, got {:?}",
        result
    );
    assert!(
        !matches!(result, crate::orchestration::AcceptanceResult::Continue),
        "a malformed verdict must never be classified as an intentional CONTINUE"
    );

    assert!(
        !repo_root.path().join("ACCEPTANCE_REPORT.json").exists(),
        "malformed trailing-text verdict must not produce a workspace-root acceptance report"
    );
}

/// Regression for `prevent-premature-acceptance-exit`: an acceptance agent
/// that starts a long-running check, reports it is monitoring/waiting for the
/// completion result, and exits without that result or a canonical verdict
/// must be classified as an explicit missing-verdict protocol failure — never
/// as an intentional `CONTINUE` — and must not consume the explicit-CONTINUE
/// retry counter.
#[tokio::test]
async fn test_acceptance_status_only_exit_is_missing_verdict_not_continue() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    // Simulate a premature acceptance exit: the agent narrates that it is
    // monitoring a long-running verification and exits without a completion
    // result or any canonical verdict.
    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(
            "sh -c 'echo Started long-running verification job; \
             echo Monitoring verification, will emit the verdict once the \
             completion notification arrives'"
                .to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    match &result {
        crate::orchestration::AcceptanceResult::MissingVerdict { findings } => {
            assert!(
                findings
                    .iter()
                    .any(|finding| finding.contains("Monitoring verification")),
                "missing-verdict result must retain bounded output evidence, got {:?}",
                findings
            );
        }
        other => panic!(
            "status-only acceptance exit must be a missing-verdict protocol failure, got {:?}",
            other
        ),
    }
    assert!(
        !matches!(result, crate::orchestration::AcceptanceResult::Continue),
        "status-only exit must never be reported as an intentional CONTINUE"
    );
    assert_eq!(iteration, 1, "attempt must still be recorded");

    // The recorded attempt must carry the actionable missing-verdict
    // diagnostic instead of the CONTINUE history marker, so the explicit
    // CONTINUE retry counter is not consumed.
    let history_findings = acceptance_history
        .lock()
        .await
        .last_findings("change-a")
        .or_fail("missing-verdict attempt must be recorded in acceptance history");
    assert!(
        history_findings
            .first()
            .is_some_and(|first| first.contains("Missing acceptance verdict")),
        "attempt evidence must identify the missing verdict, got {:?}",
        history_findings
    );
    assert!(
        !history_findings
            .iter()
            .any(|finding| finding.contains("Investigation incomplete - continue later")),
        "missing-verdict attempt must not record the CONTINUE history marker"
    );
    assert_eq!(
        agent.count_consecutive_acceptance_continues("change-a"),
        0,
        "missing verdict must not consume the explicit-CONTINUE retry counter"
    );

    assert!(
        !repo_root.path().join("ACCEPTANCE_REPORT.json").exists(),
        "missing-verdict outcome must not produce a workspace-root acceptance report"
    );
}

/// Control case for `prevent-premature-acceptance-exit`: an explicit canonical
/// `CONTINUE` verdict keeps its intentional-continuation routing and continues
/// to feed the configured explicit-CONTINUE retry counter.
#[tokio::test]
async fn test_acceptance_explicit_continue_verdict_retains_continue_routing() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some("sh -c 'echo {\\\"acceptance\\\":\\\"continue\\\"}'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    assert!(
        matches!(result, crate::orchestration::AcceptanceResult::Continue),
        "explicit canonical CONTINUE must retain intentional-continuation routing, got {:?}",
        result
    );
    assert_eq!(iteration, 1);
    assert_eq!(
        agent.count_consecutive_acceptance_continues("change-a"),
        1,
        "explicit CONTINUE must keep feeding the configured retry counter"
    );
}

/// Regression for `adopt-json-acceptance-verdict`: a malformed trailing-text
/// PASS previously forced CONTINUE (because the legacy standalone marker
/// contract was the only accepted verdict). Under the JSON-primary contract,
/// when the same acceptance run also emits a strict JSON verdict object
/// (e.g. as the opencode final payload), the runtime MUST finalize acceptance
/// as PASS and proceed to archive handoff instead of retrying.
#[tokio::test]
async fn test_acceptance_json_verdict_pass_overrides_malformed_text() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    std::fs::write(repo_root.path().join("feature.rs"), "fn gate() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let change_id = "change-a";
    let tasks_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&tasks_dir).or_fail("unexpected error");
    std::fs::write(
        tasks_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] 1. done\n",
    )
    .or_fail("unexpected error");

    // Emit the same malformed trailing-text PASS that previously fell through
    // to CONTINUE, followed by a strict JSON verdict. With the JSON-primary
    // contract, the JSON verdict wins and acceptance finalizes as PASS.
    let acceptance_config = create_test_config_with(OrchestratorConfig {
        acceptance_command: Some(
            "sh -c 'echo ACCEPTANCE: PASSAll acceptance criteria verified; \
             echo {\\\"acceptance\\\":\\\"pass\\\"}'"
                .to_string(),
        ),
        archive_command: Some(
            "sh -c 'mkdir -p openspec/changes/archive && mv openspec/changes/change-a openspec/changes/archive/change-a && echo archive-ran > archive-ran.txt'"
                .to_string(),
        ),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let mut agent = AgentRunner::new(acceptance_config.clone());
    let acceptance_tail_injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let acceptance_history = Arc::new(Mutex::new(crate::history::AcceptanceHistory::new()));

    let (result, _iteration) = execute_acceptance_in_workspace(
        "change-a",
        repo_root.path(),
        &mut agent,
        None,
        None,
        &ai_runner,
        &acceptance_config,
        &acceptance_tail_injected,
        &acceptance_history,
        Some("main"),
        None,
    )
    .await
    .or_fail("unexpected error");

    assert!(
        matches!(result, crate::orchestration::AcceptanceResult::Pass),
        "JSON verdict MUST finalize acceptance as PASS even when preceded by a \
         malformed trailing-text marker that legacy contract would reject; \
         got {:?}",
        result
    );

    execute_archive_in_workspace(
        "change-a",
        repo_root.path(),
        acceptance_config
            .get_archive_command()
            .or_fail("unexpected error"),
        &acceptance_config,
        None,
        VcsBackend::Git,
        None,
        None,
        None,
        &ai_runner,
        &Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        &Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        &shared_stagger_state,
    )
    .await
    .or_fail("archive should pass when JSON verdict finalizes acceptance");
}

#[cfg(unix)]
#[tokio::test]
async fn test_parallel_archive_commit_finalization_retries_hook_modified_files_without_rerunning_archive_command(
) {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    let change_id = "change-a";
    let change_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] done\n",
    )
    .or_fail("unexpected error");
    std::fs::write(repo_root.path().join("README.md"), "base\n").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    std::fs::write(repo_root.path().join("feature.rs"), "fn applied() {}\n")
        .or_fail("unexpected error");
    Command::new("git")
        .args(["add", "feature.rs"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let hook_path = repo_root.path().join(".git/hooks/commit-msg");
    std::fs::write(
        &hook_path,
        "#!/bin/sh\n\
if grep -q '^Archive: change-a$' \"$1\"; then echo archive-msg >> .git/hooks/commit-msg-seen; fi\n\
if grep -q '^Archive: change-a$' \"$1\" && [ ! -f .git/hooks/finalization-hook-ran ]; then\n\
  echo 'could not find dependency_targets in the crate root' >&2\n\
  echo 'hook-fixed' >> openspec/changes/archive/change-a/tasks.md\n\
  touch .git/hooks/finalization-hook-ran\n\
  exit 1\n\
fi\n\
exit 0\n",
    )
    .or_fail("unexpected error");
    let mut perms = std::fs::metadata(&hook_path)
        .or_fail("unexpected error")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&hook_path, perms).or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        archive_command: Some(
            "sh -c 'mkdir -p openspec/changes/archive && mv openspec/changes/change-a openspec/changes/archive/change-a && count=$(cat archive-count.txt 2>/dev/null || echo 0); count=$((count + 1)); printf %s $count > archive-count.txt'"
                .to_string(),
        ),
        resolve_command: Some("printf '%s\\n' {prompt} > resolve-prompt.txt; git add -A; git commit -m 'Archive: change-a'".to_string()),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let archive_history = Arc::new(Mutex::new(crate::history::ArchiveHistory::new()));
    let apply_history = Arc::new(Mutex::new(crate::history::ApplyHistory::new()));

    let result = execute_archive_in_workspace(
        change_id,
        repo_root.path(),
        config.get_archive_command().or_fail("unexpected error"),
        &config,
        Some(event_tx.clone()),
        VcsBackend::Git,
        None,
        None,
        None,
        &ai_runner,
        &archive_history,
        &apply_history,
        &shared_stagger_state,
    )
    .await
    .or_fail("archive finalization should recover from hook-modified files");

    assert!(
        !result.trim().is_empty(),
        "archive should return final revision"
    );
    assert_eq!(
        std::fs::read_to_string(repo_root.path().join("archive-count.txt"))
            .or_fail("unexpected error"),
        "1",
        "archive command should not be rerun when only commit finalization fails"
    );

    let log_subject = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    assert_eq!(
        String::from_utf8_lossy(&log_subject.stdout).trim(),
        "Archive: change-a"
    );
    let hook_seen = std::fs::read_to_string(repo_root.path().join(".git/hooks/commit-msg-seen"))
        .or_fail("archive commit hook should run during finalization");
    assert!(
        hook_seen.lines().count() >= 2,
        "archive commit hook should run for failed direct commit and later resolve retry; got {hook_seen:?}"
    );

    drop(event_tx);
    let mut saw_finalization_retry_log = false;
    let mut saw_prior_stderr_context = false;
    while let Ok(event) = event_rx.try_recv() {
        match event {
            crate::events::ExecutionEvent::Log(entry)
                if entry.operation.as_deref() == Some("archive-finalization")
                    && entry
                        .message
                        .contains("Archive commit finalization retry scheduled") =>
            {
                saw_finalization_retry_log = true;
            }
            crate::events::ExecutionEvent::ArchiveOutput { output, .. }
                if output.contains("could not find dependency_targets in the crate root") =>
            {
                saw_prior_stderr_context = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_finalization_retry_log || hook_seen.lines().count() >= 2,
        "finalization retry should emit a user-visible archive-finalization log event or show hook retry evidence"
    );
    let _ = saw_prior_stderr_context;
}

#[cfg(unix)]
#[tokio::test]
async fn test_archived_dirty_finalization_resume_does_not_rerun_archive_command() {
    let repo_root = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_root.path()).await;

    let change_id = "fix-dependency-target-handling";
    let change_dir = repo_root.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(
        change_dir.join("tasks.md"),
        "## Implementation Tasks\n\n- [x] reducer\n- [x] scheduler\n",
    )
    .or_fail("unexpected error");
    std::fs::write(change_dir.join("proposal.md"), "# Change\n").or_fail("unexpected error");
    std::fs::write(repo_root.path().join("README.md"), "base\n").or_fail("unexpected error");
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");

    let archive_dir = repo_root
        .path()
        .join("openspec/changes/archive/2026-05-08-fix-dependency-target-handling");
    std::fs::create_dir_all(&archive_dir).or_fail("unexpected error");
    std::fs::rename(
        change_dir.join("proposal.md"),
        archive_dir.join("proposal.md"),
    )
    .or_fail("unexpected error");
    std::fs::rename(change_dir.join("tasks.md"), archive_dir.join("tasks.md"))
        .or_fail("unexpected error");
    std::fs::remove_dir_all(&change_dir).or_fail("unexpected error");
    std::fs::write(archive_dir.join("report.md"), "# final report\n").or_fail("unexpected error");
    std::fs::write(repo_root.path().join("archive-count.txt"), "0").or_fail("unexpected error");

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some(
            "printf '%s\n' {prompt} > resolve-prompt.txt; git add -A; git commit -m 'Archive: fix-dependency-target-handling'"
                .to_string(),
        ),
        ..Default::default()
    });
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let shared_stagger_state = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);

    let result = execute_archive_finalization_in_workspace(
        change_id,
        repo_root.path(),
        &config,
        Some(event_tx.clone()),
        VcsBackend::Git,
        &ai_runner,
        &shared_stagger_state,
    )
    .await
    .or_fail("archive finalization resume should complete from archived dirty state");

    assert!(!result.trim().is_empty());
    assert_eq!(
        std::fs::read_to_string(repo_root.path().join("archive-count.txt"))
            .or_fail("unexpected error"),
        "0",
        "resume finalization must not run the archive move command"
    );
    let log_subject = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    assert_eq!(
        String::from_utf8_lossy(&log_subject.stdout).trim(),
        "Archive: fix-dependency-target-handling",
        "finalization resume should create the archive commit"
    );
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root.path())
        .output()
        .await
        .or_fail("unexpected error");
    assert_eq!(String::from_utf8_lossy(&status.stdout).trim(), "");

    drop(event_tx);
    let mut saw_resume_event = false;
    while let Ok(event) = event_rx.try_recv() {
        if let crate::events::ExecutionEvent::ArchiveResumed {
            reason, summary, ..
        } = event
        {
            saw_resume_event = reason.as_deref() == Some("archive_commit_incomplete")
                && summary
                    .as_deref()
                    .unwrap_or_default()
                    .contains("commit finalization");
        }
    }
    assert!(
        saw_resume_event,
        "resume path should emit dedicated ArchiveResumed event"
    );
}

// --- Repair loop through the real parallel dispatch cycle ---
//
// These drive `dispatch_change_to_workspace` end to end with scripted apply and
// acceptance commands so the diff-coverage gate, the per-ID repair budget, and
// the resulting stop diagnostics are observed on the production path rather
// than by calling the shared decision APIs directly.

/// Observations from one scripted repair-cycle dispatch.
struct ScriptedRepairDispatch {
    result: WorkspaceResult,
    workspace_path: PathBuf,
    acceptance_invocations: u32,
    apply_invocations: u32,
    acceptance_error_logs: Vec<String>,
}

/// One structured FAIL verdict as the acceptance command would print it.
fn scripted_structured_fail(id: &str, implementation: &str, verification: &str) -> String {
    serde_json::json!({
        "acceptance": "fail",
        "findings": [{
            "id": id,
            "severity": "major",
            "summary": "Issued values are never asserted by value",
            "evidence": [format!("{implementation} records counts only")],
            "required_changes": [{
                "file": implementation,
                "description": "Expose issued challenge and presented proof values",
            }],
            "verification": [{
                "file": verification,
                "description": "Assert recorded values are absent from audit output",
            }],
        }],
    })
    .to_string()
}

/// Dispatch one change whose acceptance command emits the recorded verdict for
/// invocation `n`, falling back to the last recorded verdict. `apply_extra` is
/// the shell fragment the apply command runs after completing every open task,
/// which is how a scripted repair chooses whether it covers the declared files.
async fn dispatch_scripted_repair_cycle(
    change_id: &str,
    verdicts: &[String],
    apply_extra: &str,
) -> ScriptedRepairDispatch {
    let repo_dir = TempDir::new().or_fail("create temp repo");
    let workspace_base = TempDir::new().or_fail("create temp workspace base");
    let state_dir = TempDir::new().or_fail("create scripted verdict state dir");
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    let verdict_dir = state_dir.path().join("verdicts");
    std::fs::create_dir_all(&verdict_dir).or_fail("create verdict dir");
    for (index, verdict) in verdicts.iter().enumerate() {
        std::fs::write(
            verdict_dir.join(format!("verdict-{}.json", index + 1)),
            verdict,
        )
        .or_fail("write scripted verdict");
    }
    std::fs::write(
        verdict_dir.join("verdict-last.json"),
        verdicts.last().or_fail("at least one verdict"),
    )
    .or_fail("write trailing verdict");

    let counter = state_dir.path().join("attempts").display().to_string();
    let verdict_dir = verdict_dir.display().to_string();
    let base_revision = get_current_commit(repo_dir.path())
        .await
        .or_fail("get base revision");

    let config = create_test_config_with(OrchestratorConfig {
        workspace_base_dir: Some(workspace_base.path().to_string_lossy().to_string()),
        apply_command: Some(format!(
            "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
             > openspec/changes/{change_id}/tasks.next \
             && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md; \
             {apply_extra}\""
        )),
        acceptance_command: Some(format!(
            "sh -c 'n=$(cat \"{counter}\" 2>/dev/null || echo 0); n=$((n+1)); \
             echo $n > \"{counter}\"; verdict=\"{verdict_dir}/verdict-$n.json\"; \
             [ -f \"$verdict\" ] || verdict=\"{verdict_dir}/verdict-last.json\"; cat \"$verdict\"'"
        )),
        archive_command: Some(format!(
            "sh -c 'mkdir -p openspec/changes/archive \
             && mv openspec/changes/{change_id} openspec/changes/archive/{change_id}'"
        )),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });

    let (tx, mut rx) = mpsc::channel(256);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));
    let semaphore = Arc::new(Semaphore::new(1));
    let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
    let mut cleanup_guard = crate::parallel::cleanup::WorkspaceCleanupGuard::new(
        VcsBackend::Git,
        repo_dir.path().to_path_buf(),
    );
    let mut in_flight = HashSet::new();

    executor
        .dispatch_change_to_workspace(
            change_id.to_string(),
            base_revision,
            semaphore,
            &mut join_set,
            &mut in_flight,
            &mut cleanup_guard,
        )
        .await
        .or_fail("dispatch scripted repair cycle");
    let result = join_set
        .join_next()
        .await
        .or_fail("workspace task should exist")
        .or_fail("workspace task join should succeed");

    let workspace_path = workspace_base.path().join(format!("cflx-{change_id}"));
    let mut observed = ScriptedRepairDispatch {
        result,
        workspace_path,
        acceptance_invocations: 0,
        apply_invocations: 0,
        acceptance_error_logs: Vec::new(),
    };

    while let Ok(event) = rx.try_recv() {
        match event {
            ExecutionEvent::AcceptanceStarted { change_id: id, .. } if id == change_id => {
                observed.acceptance_invocations += 1;
            }
            ExecutionEvent::ApplyStarted { change_id: id, .. } if id == change_id => {
                observed.apply_invocations += 1;
            }
            ExecutionEvent::Log(log)
                if matches!(log.level, crate::events::LogLevel::Error)
                    && log.operation.as_deref() == Some("acceptance") =>
            {
                observed.acceptance_error_logs.push(log.message.clone());
            }
            _ => {}
        }
    }

    observed
}

/// A repair that only nudges an unrelated calibration constant must not buy
/// another acceptance invocation.
#[tokio::test]
async fn parallel_calibration_only_repair_holds_before_a_second_acceptance_invocation() {
    let change_id = "parallel-mismatch";
    let observed = dispatch_scripted_repair_cycle(
        change_id,
        &[scripted_structured_fail(
            "acceptance-secret-value-scan",
            "src/relay.rs",
            "tests/relay_test.rs",
        )],
        "mkdir -p calibration && echo tweak >> calibration/thresholds.txt",
    )
    .await;

    let error = observed
        .result
        .error
        .as_ref()
        .or_fail("a missing-coverage repair must stop the workspace");
    assert!(error.contains("acceptance_remediation_mismatch"), "{error}");
    assert!(error.contains("src/relay.rs"), "{error}");
    assert!(error.contains("tests/relay_test.rs"), "{error}");
    assert!(error.contains("\"coverage_complete\":false"), "{error}");
    assert!(
        error.contains("calibration/thresholds.txt"),
        "the unrelated repair must be reported as a diagnostic: {error}"
    );
    assert_eq!(
        observed.acceptance_invocations, 1,
        "the gate must stop before spending a second acceptance invocation"
    );
    assert_eq!(
        observed.apply_invocations, 2,
        "the repair apply itself runs before the gate evaluates its delta"
    );
    assert!(
        observed.result.final_revision.is_none(),
        "a held repair cycle must never hand off to archive"
    );
    assert!(
        !observed
            .workspace_path
            .join(".cflx/acceptance-state.json")
            .exists(),
        "a repair hold must not create a durable acceptance checkpoint"
    );
}

/// The same stable finding ID on the next FAIL stops before a second automatic
/// repair apply, even though the repair covered every declared path.
#[tokio::test]
async fn parallel_repeated_finding_id_stops_the_dispatch_cycle_before_a_second_repair_apply() {
    let change_id = "parallel-repeated";
    let observed = dispatch_scripted_repair_cycle(
        change_id,
        &[scripted_structured_fail(
            "acceptance-secret-value-scan",
            "src/relay.rs",
            "tests/relay_test.rs",
        )],
        "mkdir -p src tests && echo repair >> src/relay.rs && echo proof >> tests/relay_test.rs",
    )
    .await;

    let error = observed
        .result
        .error
        .as_ref()
        .or_fail("a repeated finding ID must stop the workspace");
    assert!(error.contains("repeated_acceptance_finding"), "{error}");
    assert!(error.contains("acceptance-secret-value-scan"), "{error}");
    assert!(error.contains("\"resumable\":true"), "{error}");
    assert!(
        error.contains("\"proves_acceptance_pass\":false"),
        "{error}"
    );
    assert_eq!(
        observed.apply_invocations, 2,
        "the first repair is automatic; the second one is not"
    );
    assert_eq!(
        observed.acceptance_invocations, 2,
        "the stop happens on the FAIL that repeats the ID, not later"
    );
    assert_eq!(
        observed.acceptance_error_logs.len(),
        1,
        "exactly one operator-facing stop diagnostic: {:?}",
        observed.acceptance_error_logs
    );
    assert!(observed.result.final_revision.is_none());
}

/// A genuinely new ID gets its own automatic repair apply — the prior ID's spent
/// budget does not carry over — and that opportunity is likewise spent once.
///
/// The serial counterpart covers the canonical PASS that follows two repaired
/// IDs. Three real apply/acceptance cycles through managed worktrees put this
/// one just over one second, so it belongs to the heavy tier.
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
#[tokio::test]
async fn parallel_new_finding_id_receives_its_own_repair_apply_in_the_dispatch_cycle() {
    let change_id = "parallel-new-id";
    let observed = dispatch_scripted_repair_cycle(
        change_id,
        &[
            scripted_structured_fail(
                "acceptance-secret-value-scan",
                "src/relay.rs",
                "tests/relay_test.rs",
            ),
            scripted_structured_fail(
                "acceptance-audit-gap",
                "src/audit.rs",
                "tests/audit_test.rs",
            ),
        ],
        "mkdir -p src tests && echo repair >> src/relay.rs && echo proof >> tests/relay_test.rs \
         && echo repair >> src/audit.rs && echo proof >> tests/audit_test.rs",
    )
    .await;

    let error = observed
        .result
        .error
        .as_ref()
        .or_fail("the new ID's own budget is spent by its second occurrence");
    assert!(error.contains("repeated_acceptance_finding"), "{error}");
    assert!(
        error.contains("acceptance-audit-gap"),
        "the new ID is the stop reason, not the Acceptance-closed prior ID: {error}"
    );
    assert!(
        !error.contains("acceptance-secret-value-scan"),
        "the prior ID was closed by Acceptance and must not reappear: {error}"
    );
    assert_eq!(
        observed.acceptance_invocations, 3,
        "the new ID's repair is re-reviewed instead of stopping at its first FAIL"
    );
    assert_eq!(
        observed.apply_invocations, 3,
        "the new ID received its own automatic repair apply"
    );
    assert!(observed.result.final_revision.is_none());
}

/// Drive the same scripted repeated-ID scenario through the real serial cycle
/// and return the stop diagnostic serial produced.
async fn run_serial_repeated_cycle(change_id: &str, verdict: &str, apply_extra: &str) -> String {
    use crate::hooks::{HookRunner, HooksConfig};
    use crate::orchestration::output::NullOutputHandler;
    use crate::serial_run_service::{ChangeProcessResult, SerialRunService};

    let repo_dir = TempDir::new().or_fail("create temp serial repo");
    let state_dir = TempDir::new().or_fail("create serial verdict state dir");
    init_missing_verdict_repo(repo_dir.path(), change_id).await;

    let verdict_path = state_dir.path().join("verdict.json");
    std::fs::write(&verdict_path, verdict).or_fail("write serial verdict");
    let verdict_path = verdict_path.display().to_string();

    let config = create_test_config_with(OrchestratorConfig {
        apply_command: Some(format!(
            "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
             > openspec/changes/{change_id}/tasks.next \
             && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md; \
             {apply_extra}\""
        )),
        acceptance_command: Some(format!("sh -c 'cat \"{verdict_path}\"'")),
        command_queue_stagger_delay_ms: Some(0),
        command_queue_max_retries: Some(0),
        command_queue_retry_delay_ms: Some(0),
        command_queue_retry_if_duration_under_secs: Some(0),
        ..Default::default()
    });

    let queue_config = CommandQueueConfig {
        stagger_delay_ms: 0,
        max_retries: 0,
        retry_delay_ms: 0,
        retry_error_patterns: default_retry_patterns(),
        retry_if_duration_under_secs: 0,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 10,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };
    let ai_runner = AiCommandRunner::new(queue_config, Arc::new(Mutex::new(None)));
    let mut agent = AgentRunner::new(config.clone());
    let mut service = SerialRunService::new(repo_dir.path().to_path_buf(), config.clone());
    let change = crate::openspec::list_changes_native_from(repo_dir.path())
        .or_fail("list serial changes")
        .into_iter()
        .find(|change| change.id == change_id)
        .or_fail("the scripted change exists");

    let mut last = None;
    for _ in 0..2 {
        last = Some(
            service
                .process_change(
                    &change,
                    &mut agent,
                    &ai_runner,
                    &HookRunner::new(HooksConfig::default(), repo_dir.path()),
                    &NullOutputHandler::new(),
                    1,
                    1,
                    || false,
                    || false,
                    None,
                )
                .await
                .or_fail("serial cycle should not error"),
        );
    }

    match last.or_fail("two serial cycles ran") {
        ChangeProcessResult::Stalled { error } => error,
        other => panic!("expected a serial repeated-finding stop, got {other:?}"),
    }
}

/// Parse the machine-readable diagnostics out of a repair stop summary.
fn repair_stop_diagnostics(summary: &str) -> serde_json::Value {
    let (_, json) = summary
        .split_once("Diagnostics: ")
        .or_fail("a repair stop summary carries machine-readable diagnostics");
    serde_json::from_str(json).or_fail("repair stop diagnostics are valid JSON")
}

/// Equivalent observations in the two execution modes must produce equivalent
/// operator evidence. Both sides here come from a real cycle — serial through
/// `process_change`, parallel through `dispatch_change_to_workspace` — so this
/// compares two independent runs rather than one function against itself.
#[tokio::test]
async fn serial_and_parallel_repeated_finding_stops_report_equivalent_diagnostics() {
    let change_id = "parity-repeated";
    let verdict = scripted_structured_fail(
        "acceptance-secret-value-scan",
        "src/relay.rs",
        "tests/relay_test.rs",
    );
    let apply_extra =
        "mkdir -p src tests && echo repair >> src/relay.rs && echo proof >> tests/relay_test.rs";

    // The two runs are independent (separate repositories, workspaces, and
    // scripted counters), so they run concurrently to keep this test inside the
    // repository's one-second default-suite budget.
    let (parallel, serial_error) = tokio::join!(
        dispatch_scripted_repair_cycle(change_id, std::slice::from_ref(&verdict), apply_extra),
        run_serial_repeated_cycle(change_id, &verdict, apply_extra),
    );
    let parallel_error = parallel
        .result
        .error
        .as_ref()
        .or_fail("parallel must stop on the repeated ID");

    let parallel_json = repair_stop_diagnostics(parallel_error);
    let serial_json = repair_stop_diagnostics(&serial_error);

    // Revision identifiers and the raw delta are legitimately mode-specific:
    // parallel commits apply inside its worktree, serial does not. Everything
    // that explains *why* automation stopped must match exactly.
    for field in [
        "change_id",
        "stop_reason",
        "findings",
        "finding_occurrences",
        "repeated_identities",
        "required_files",
        "verification_files",
        "uncovered_files",
        "coverage_complete",
        "legacy_findings_without_declared_paths",
        "remediation_evidence",
        "resumable",
        "next_action",
        "proves_completion",
        "proves_acceptance_pass",
        "proves_archive_readiness",
    ] {
        assert_eq!(
            serial_json.get(field),
            parallel_json.get(field),
            "field `{field}` diverges between execution modes:\nserial={serial_json}\nparallel={parallel_json}"
        );
    }
    assert_eq!(
        serial_json
            .get("stop_reason")
            .and_then(|value| value.as_str()),
        Some(crate::orchestration::acceptance::REPEATED_FINDING_REASON)
    );
}
