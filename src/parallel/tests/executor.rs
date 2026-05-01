//! Tests for ParallelExecutor and related functionality.

use super::super::*;
use crate::agent::AgentRunner;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::default_retry_patterns;
use crate::config::OrchestratorConfig;
use crate::parallel::executor::{execute_acceptance_in_workspace, execute_archive_in_workspace};
use crate::vcs::git::commands::get_current_commit;
#[cfg(feature = "heavy-tests")]
use crate::vcs::GitWorkspaceManager;
use crate::vcs::{VcsBackend, VcsError, VcsResult, VcsWarning, Workspace, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

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

#[allow(dead_code)]
struct TestWorkspaceManager {
    merge_calls: Arc<AtomicUsize>,
    conflict_files: Vec<String>,
    #[allow(dead_code)]
    repo_root: PathBuf,
}

impl TestWorkspaceManager {
    #[allow(dead_code)]
    fn new(merge_calls: Arc<AtomicUsize>) -> Self {
        Self {
            merge_calls,
            conflict_files: vec!["conflict.txt".to_string()],
            repo_root: PathBuf::from("/tmp/test-repo"),
        }
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
        _change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        Ok(None)
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 1,
        repo_root: PathBuf::from("/tmp/test-repo"),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies,
        resolve_wait_changes,
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    init_git_repo(repo_root).await;

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
    commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;

    std::fs::write(repo_root.join("dirty.txt"), "dirty").or_fail("unexpected error");

    let result = resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a").await;
    assert!(result.is_err());

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .or_fail("unexpected error");
    let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
    assert!(!merge_messages.contains("Merge change: change-a"));
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_resolve_merge_executes_selected_change_only() {
    let temp_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let repo_root = temp_dir.path();
    let worktree_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let base_dir = worktree_dir.path().join("worktrees");
    let resolver_dir = tempfile::TempDir::new().or_fail("unexpected error");
    let resolver_script = resolver_dir.path().join("merge-resolver.sh");

    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some(format!("sh {}", resolver_script.display())),
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
    commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;
    commit_workspace_change(&workspace_b, "change-b.txt", "B", "Apply: change-b").await;

    // Create archive entries in workspace_a and workspace_b to satisfy archive verification
    for (workspace, change_id) in [(&workspace_a, "change-a"), (&workspace_b, "change-b")] {
        // Remove openspec/changes/<change_id> directory to simulate completed archive
        let changes_dir = workspace
            .path
            .join(format!("openspec/changes/{}", change_id));
        if changes_dir.exists() {
            std::fs::remove_dir_all(&changes_dir).or_fail("unexpected error");
        }

        // Create archive entry as a directory (archive_entry_exists checks directory names)
        let archive_dir = workspace.path.join("openspec/changes/archive");
        let archive_entry = archive_dir.join(change_id);
        std::fs::create_dir_all(&archive_entry).or_fail("unexpected error");
        std::fs::write(
            archive_entry.join("proposal.md"),
            format!("# Archive entry for {}", change_id),
        )
        .or_fail("unexpected error");

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace.path)
            .output()
            .await
            .or_fail("unexpected error");
        Command::new("git")
            .args(["commit", "-m", &format!("Archive: {}", change_id)])
            .current_dir(&workspace.path)
            .output()
            .await
            .or_fail("unexpected error");
    }

    let script_contents = format!(
        "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n",
        workspace_a.path.to_string_lossy(),
        workspace_a.name,
        workspace_a.name
    );
    std::fs::write(&resolver_script, script_contents).or_fail("unexpected error");

    resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a")
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
    assert!(!merge_messages.contains("Merge change: change-b"));
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_uses_resolve_command_with_change_ids() {
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
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-b' main\n\
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_allows_non_merge_head_after_merges() {
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
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-b' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-b' {}\n\
            echo 'post-merge' >> README.md\n\
            git add -A\n\
            git commit -m 'Post-merge commit'\n",
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
}

#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn test_merge_retries_when_merge_left_in_progress() {
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
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    let workspace_a = manager
        .create_workspace("change-a", None)
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

    let resolver_script = repo_root.join("merge-resolver.sh");
    let script_contents = format!(
        "#!/bin/sh\nset -e\n\
            if [ -f .git/merge-in-progress-marker ]; then\n\
              git commit -m 'Merge change: change-a'\n\
              exit 0\n\
            fi\n\
            git checkout main\n\
            git merge --no-ff --no-commit {}\n\
            touch .git/merge-in-progress-marker\n",
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
        workspace_manager: Box::new(manager),
        config,
        apply_command: String::new(),
        archive_command: String::new(),
        event_tx: None,
        max_conflict_retries: 2,
        repo_root: repo_root.to_path_buf(),
        no_resume: false,
        failed_tracker: FailedChangeTracker::new(),
        change_dependencies: HashMap::new(),
        resolve_wait_changes: HashSet::new(),
        merge_wait_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
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
        shared_orchestrator_state: None,
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
    use tempfile::TempDir;

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
    use tempfile::TempDir;

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
            "sh -c 'echo ACCEPTANCE: FAIL; echo; echo FINDINGS:; echo - missing regression test; echo - add repo coverage'"
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
    )
    .await
    .or_fail("unexpected error");

    match result {
        crate::orchestration::AcceptanceResult::Fail { findings } => {
            assert_eq!(findings.len(), 2);
        }
        other => panic!("expected acceptance fail, got {:?}", other),
    }

    crate::task_parser::record_acceptance_follow_up(
        &tasks_dir.join("tasks.md"),
        iteration,
        &[
            "missing regression test".to_string(),
            "add repo coverage".to_string(),
        ],
    )
    .or_fail("unexpected error");

    let updated_tasks =
        std::fs::read_to_string(tasks_dir.join("tasks.md")).or_fail("unexpected error");
    assert!(updated_tasks.contains("## Acceptance #1 Failure Follow-up"));
    assert!(updated_tasks.contains("- [ ] missing regression test"));
    assert!(updated_tasks.contains("- [ ] add repo coverage"));

    let progress = crate::task_parser::parse_file(&tasks_dir.join("tasks.md"), Some(change_id))
        .or_fail("unexpected error");
    assert_eq!(progress.completed, 1);
    assert_eq!(progress.total, 3);
}

#[tokio::test]
async fn test_acceptance_history_records_end_revision_when_head_changes() {
    use tempfile::TempDir;

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
    use tempfile::TempDir;

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
    use tempfile::TempDir;

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
    .or_fail("archive should pass when workspace-local acceptance evidence allows handoff");

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
        "slot recovery should bypass debounce"
    );
    assert!(
        !executor.should_reanalyze(false).await,
        "regular queue edits should still respect debounce"
    );
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

fn ready_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>>
{
    let order = changes.iter().map(|change| change.id.clone()).collect();
    Box::pin(async move {
        crate::analyzer::AnalysisResult {
            order,
            dependencies: HashMap::new(),
            groups: None,
        }
    })
}

fn blocked_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>>
{
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
    })
}

fn selective_dependency_analysis_result<'a>(
    changes: &'a [crate::openspec::Change],
    _in_flight: &'a [String],
    _iteration: u32,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + 'a>>
{
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
    })
}

#[cfg_attr(not(feature = "heavy-tests"), ignore)]
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            1,
            ReanalysisReason::QueueNotification,
            &selective_dependency_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            2,
            ReanalysisReason::QueueNotification,
            &ready_analysis_result,
            semaphore.clone(),
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            iteration,
            ReanalysisReason::QueueNotification,
            &ready_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            1,
            ReanalysisReason::QueueNotification,
            &ready_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            2,
            1,
            ReanalysisReason::QueueNotification,
            &ready_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            1,
            ReanalysisReason::QueueNotification,
            &blocked_analysis_result,
            semaphore.clone(),
            &mut join_set,
            &mut cleanup_guard,
        )
        .await
        .or_fail("unexpected error");

    assert!(!should_break);
    assert_eq!(iteration, 1, "dispatch 0件では iteration は進まない");
    assert_eq!(queued.len(), 1, "dispatchできない change はキューに残る");
    assert!(in_flight.is_empty());

    let (should_break, iteration) = executor
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            iteration,
            ReanalysisReason::ResolveCompletion,
            &ready_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
        .perform_reanalysis_and_dispatch(
            &mut queued,
            &mut in_flight,
            1,
            2,
            ReanalysisReason::ResolveCompletion,
            &ready_analysis_result,
            semaphore,
            &mut join_set,
            &mut cleanup_guard,
        )
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
async fn test_handle_merge_result_keeps_pending_counter_non_negative() {
    use crate::parallel::MergeResult;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.store(2, Ordering::Relaxed);
    executor
        .handle_merge_result(MergeResult {
            change_id: "change-ok".to_string(),
            workspace_name: "ws-change-ok".to_string(),
            outcome: Ok(()),
        })
        .await;
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 1);

    executor
        .handle_merge_result(MergeResult {
            change_id: "change-err".to_string(),
            workspace_name: "ws-change-err".to_string(),
            outcome: Err("merge failed".to_string()),
        })
        .await;
    assert_eq!(executor.pending_merge_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn fix_scheduler_premature_exit_decrements_pending_merge_counter_on_merge_completion() {
    use crate::parallel::MergeResult;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let (tx, _rx) = mpsc::channel(32);
    let mut executor = ParallelExecutor::new(repo_dir.path().to_path_buf(), config, Some(tx));

    executor.pending_merge_count.fetch_add(1, Ordering::Relaxed);

    executor
        .handle_merge_result(MergeResult {
            change_id: "change-ok".to_string(),
            workspace_name: "ws-change-ok".to_string(),
            outcome: Ok(()),
        })
        .await;

    assert_eq!(
        executor.pending_merge_count.load(Ordering::Relaxed),
        0,
        "scheduler must clear pending merge counter after merge result is handled"
    );
}

#[tokio::test]
async fn test_scheduler_lifetime_controls_idle_exit_behavior() {
    use tempfile::TempDir;

    let repo_dir = TempDir::new().or_fail("unexpected error");
    init_git_repo(repo_dir.path()).await;

    let config = create_test_config();
    let mut finite_executor =
        ParallelExecutor::new(repo_dir.path().to_path_buf(), config.clone(), None);

    assert!(
        finite_executor.should_exit_when_idle(true, true, true),
        "finite scheduler must exit when all work is drained"
    );

    finite_executor.set_persistent_lifetime();
    assert!(
        !finite_executor.should_exit_when_idle(true, true, true),
        "persistent scheduler must remain alive while idle"
    );

    assert!(
        !finite_executor.should_exit_when_idle(false, true, true),
        "scheduler must not exit when active join tasks remain"
    );
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
    use tempfile::TempDir;

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

    assert!(
        repo_root.path().join("ACCEPTANCE_REPORT.json").exists(),
        "verdict-finalized PASS must produce workspace-local acceptance evidence"
    );
}

/// Regression: malformed trailing-text verdicts (e.g. `ACCEPTANCE: PASSAll ...`)
/// MUST NOT be treated as canonical PASS by the parallel executor. The legacy
/// `starts_with` check accepted these and could lock in a bogus pass; the
/// strict canonical contract falls through to CONTINUE so the loop can retry.
#[tokio::test]
async fn test_acceptance_trailing_text_pass_is_not_canonical() {
    use tempfile::TempDir;

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
    )
    .await
    .or_fail("unexpected error");

    assert!(
        matches!(result, crate::orchestration::AcceptanceResult::Continue),
        "trailing-text PASS must NOT satisfy canonical verdict; expected CONTINUE fallback, got {:?}",
        result
    );

    assert!(
        !repo_root.path().join("ACCEPTANCE_REPORT.json").exists(),
        "malformed trailing-text verdict must not produce workspace-local acceptance evidence"
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
    use tempfile::TempDir;

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
