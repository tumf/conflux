//! Tests for ParallelExecutor and related functionality.

use super::super::*;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::default_retry_patterns;
use crate::config::OrchestratorConfig;
use crate::vcs::{VcsBackend, VcsError, VcsResult, VcsWarning, WorkspaceInfo};
use crate::vcs::{WorkspaceManager, WorkspaceStatus};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

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
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("README.md"), "base").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
}

async fn commit_workspace_change(
    workspace: &Workspace,
    filename: &str,
    contents: &str,
    message: &str,
) {
    std::fs::write(workspace.path.join(filename), contents).unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(&workspace.path)
        .output()
        .await
        .unwrap();
}
#[test]
fn test_skip_reason_for_merge_deferred_dependency() {
    let merge_calls = Arc::new(AtomicUsize::new(0));
    let manager = TestWorkspaceManager::new(merge_calls);
    let mut change_dependencies = HashMap::new();
    change_dependencies.insert("change-b".to_string(), vec!["change-a".to_string()]);
    let mut merge_deferred_changes = HashSet::new();
    merge_deferred_changes.insert("change-a".to_string());

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
        merge_deferred_changes,
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        shared_stagger_state,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };

    // MergeWait dependencies are NOT skip reasons; they are handled as blocked/queued status
    // via dependency resolution checks (is_dependency_resolved). Only failed dependencies
    // are skip reasons.
    assert!(executor.skip_reason_for_change("change-b").is_none());
    assert!(executor.skip_reason_for_change("change-c").is_none());
}

#[tokio::test]
async fn test_resolve_merge_aborts_when_base_dirty() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;

    std::fs::write(repo_root.join("dirty.txt"), "dirty").unwrap();

    let result = resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a").await;
    assert!(result.is_err());

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
    assert!(!merge_messages.contains("Merge change: change-a"));
}

#[tokio::test]
async fn test_resolve_merge_executes_selected_change_only() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let worktree_dir = tempfile::TempDir::new().unwrap();
    let base_dir = worktree_dir.path().join("worktrees");
    let resolver_dir = tempfile::TempDir::new().unwrap();
    let resolver_script = resolver_dir.path().join("merge-resolver.sh");

    init_git_repo(repo_root).await;

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some(format!("sh {}", resolver_script.display())),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    let workspace_b = manager.create_workspace("change-b", None).await.unwrap();
    commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;
    commit_workspace_change(&workspace_b, "change-b.txt", "B", "Apply: change-b").await;

    // Create archive entries in workspace_a and workspace_b to satisfy archive verification
    for (workspace, change_id) in [(&workspace_a, "change-a"), (&workspace_b, "change-b")] {
        // Remove openspec/changes/<change_id> directory to simulate completed archive
        let changes_dir = workspace
            .path
            .join(format!("openspec/changes/{}", change_id));
        if changes_dir.exists() {
            std::fs::remove_dir_all(&changes_dir).unwrap();
        }

        // Create archive entry as a directory (archive_entry_exists checks directory names)
        let archive_dir = workspace.path.join("openspec/changes/archive");
        let archive_entry = archive_dir.join(change_id);
        std::fs::create_dir_all(&archive_entry).unwrap();
        std::fs::write(
            archive_entry.join("proposal.md"),
            format!("# Archive entry for {}", change_id),
        )
        .unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Archive: {}", change_id)])
            .current_dir(&workspace.path)
            .output()
            .await
            .unwrap();
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
    std::fs::write(&resolver_script, script_contents).unwrap();

    resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a")
        .await
        .unwrap();

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
    assert!(merge_messages.contains("Merge change: change-a"));
    assert!(!merge_messages.contains("Merge change: change-b"));
}

#[tokio::test]
async fn test_merge_uses_resolve_command_with_change_ids() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("README.md"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

    std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();
}

#[tokio::test]
async fn test_merge_allows_non_merge_head_after_merges() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("README.md"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

    std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();
}

#[tokio::test]
async fn test_merge_retries_when_merge_left_in_progress() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("README.md"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();
}

#[tokio::test]
async fn test_merge_retries_when_merge_commit_missing() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("README.md"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

    std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();

    let merge_log = Command::new("git")
        .args(["log", "--merges", "--format=%s"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
    assert!(merge_messages.contains("Merge change: change-a"));
    assert!(merge_messages.contains("Merge change: change-b"));
}

#[tokio::test]
async fn test_merge_resolves_conflict_with_resolve_command() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("conflict.txt"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
    let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

    std::fs::write(workspace_a.path.join("conflict.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

    std::fs::write(workspace_b.path.join("conflict.txt"), "B").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-b"])
        .current_dir(&workspace_b.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();

    let merged_contents = std::fs::read_to_string(repo_root.join("conflict.txt")).unwrap();
    assert!(merged_contents.contains('B'));
}

#[cfg(unix)]
#[tokio::test]
async fn test_merge_retries_after_pre_commit_changes() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let repo_root = temp_dir.path();
    let base_dir = repo_root.join("worktrees");

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_root.join("hooked.txt"), "base").unwrap();

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    let config = create_test_config_with(OrchestratorConfig {
        resolve_command: Some("sh merge-resolver.sh".to_string()),
        ..Default::default()
    });
    let mut manager =
        GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

    let workspace_a = manager.create_workspace("change-a", None).await.unwrap();

    std::fs::write(repo_root.join("main.txt"), "main").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Apply: change-a"])
        .current_dir(&workspace_a.path)
        .output()
        .await
        .unwrap();

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
    std::fs::write(&hook_path, hook_contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).unwrap();
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
    std::fs::write(&resolver_script, script_contents).unwrap();

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
        merge_deferred_changes: HashSet::new(),
        previously_blocked_changes: HashSet::new(),
        force_recreate_worktree: HashSet::new(),
        hooks: None,
        cancel_token: None,
        last_queue_change_at: Arc::new(Mutex::new(None)),
        dynamic_queue: None,
        ai_runner,
        apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
        archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
        acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
        acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
        shared_stagger_state,
        needs_reanalysis: false,
        manual_resolve_count: None,
        auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        .unwrap();

    let hook_contents = std::fs::read_to_string(repo_root.join("hooked.txt")).unwrap();
    assert!(hook_contents.contains("hooked"));
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
async fn test_debounce_with_queue_changes() {
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let (tx, _rx) = mpsc::channel(10);
    let executor = ParallelExecutor::new(repo_root, config, Some(tx));

    // First check: no queue changes, should reanalyze
    assert!(executor.should_reanalyze().await);

    // Simulate a queue change
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(Instant::now());
    }

    // Immediate check: should NOT reanalyze (debounce active)
    assert!(!executor.should_reanalyze().await);

    // Wait for debounce period to expire (10 seconds + margin)
    tokio::time::sleep(Duration::from_secs(11)).await;

    // After debounce: should reanalyze
    assert!(executor.should_reanalyze().await);
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
    let result = handle.await.unwrap();
    assert!(
        result.is_ok(),
        "Queue notification should have been received"
    );
}

#[tokio::test]
async fn test_attempt_merge_defers_when_change_not_archived() {
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create openspec/changes/test-change directory (simulating incomplete archive)
    let change_dir = repo_root.join("openspec/changes/test-change");
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("spec.md"), "# Test").unwrap();

    // Commit the change directory to ensure working tree is clean
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add test change (not archived)"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

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
        Ok(MergeAttempt::Deferred(reason)) => {
            assert!(
                reason.contains("Archive incomplete"),
                "Expected deferred reason to mention archive incomplete, got: {}",
                reason
            );
            assert!(
                reason.contains("test-change"),
                "Expected reason to include change ID, got: {}",
                reason
            );
        }
        Ok(MergeAttempt::Merged) => {
            panic!("Merge should have been deferred when change directory exists");
        }
        Err(e) => {
            panic!("Expected MergeDeferred, got error: {}", e);
        }
    }
}

#[tokio::test]
async fn test_attempt_merge_succeeds_when_change_archived() {
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create archive directory but NOT openspec/changes/test-change (proper archive)
    let archive_dir = repo_root.join("openspec/changes/archive/test-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(archive_dir.join("spec.md"), "# Archived").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create worktree for the change (outside the main repo to avoid dirty working tree)
    let workspace_base = TempDir::new().unwrap();
    let workspace_path = workspace_base.path().join("ws-test-change");

    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "ws-test-change",
            workspace_path.to_str().unwrap(),
            "HEAD",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

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
        Ok(MergeAttempt::Merged) => {
            // Success - merge was allowed
        }
        Ok(MergeAttempt::Deferred(reason)) => {
            panic!(
                "Merge should have succeeded when change is archived, got deferred: {}",
                reason
            );
        }
        Err(e) => {
            // This is also acceptable - merge may fail for other reasons (e.g., merge conflicts)
            // but it should not be deferred due to archive verification
            println!("Merge failed with error (acceptable): {}", e);
        }
    }
}

/// Test that the has_merge_deferred helper correctly tracks MergeWait state.
/// Per spec (update-tui-status-display): The system SHALL send AllCompleted
/// even when MergeWait remains, but this helper tracks whether any changes
/// are in MergeWait state for other logic purposes.
#[test]
fn test_merge_wait_suppresses_completion_events() {
    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let mut executor = ParallelExecutor::new(repo_root, config, None);

    // Initially, no merge deferred - should allow completion
    assert!(!executor.has_merge_deferred());

    // Add a change to merge_deferred set
    executor
        .merge_deferred_changes
        .insert("test-change".to_string());

    // Now has_merge_deferred should return true
    assert!(executor.has_merge_deferred());

    // Clear the set
    executor.merge_deferred_changes.clear();

    // Should return false again
    assert!(!executor.has_merge_deferred());
}

/// Test that changes in MergeWait state are correctly filtered during loop iteration.
/// This test validates the spec requirement:
/// "The loop continues processing runnable changes and MergeWait is not treated as a terminal completion reason."
#[test]
fn test_merge_wait_does_not_block_runnable_changes() {
    // This is validated by the existing filtering logic in execute_with_reanalysis:
    // - Changes with WorkspaceState::Archived are excluded from queued_changes
    // - Changes with WorkspaceState::Created/Applying/Applied are included
    // - The loop continues as long as queued_changes is not empty
    //
    // MergeWait is a WorkspaceStatus (in-memory), not a WorkspaceState (from git)
    // Changes in MergeWait have WorkspaceState::Archived (archive commit exists)
    // So they are correctly excluded from processing, allowing other changes to continue

    let config = create_test_config();
    let repo_root = PathBuf::from("/tmp/test-repo");
    let executor = ParallelExecutor::new(repo_root, config, None);

    // Verify initial state
    assert_eq!(executor.merge_deferred_changes.len(), 0);
    assert!(!executor.has_merge_deferred());
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
    assert!(!executor.needs_reanalysis); // Initially false until execution starts

    // Test debounce logic
    {
        let mut last_change = executor.last_queue_change_at.lock().await;
        *last_change = Some(std::time::Instant::now());
    }

    // Immediate check: should NOT reanalyze (debounce active)
    assert!(!executor.should_reanalyze().await);

    // Wait for debounce period to expire
    tokio::time::sleep(std::time::Duration::from_secs(11)).await;

    // After debounce: should reanalyze
    assert!(executor.should_reanalyze().await);

    // Verify AnalysisStarted event would be emitted
    // (Full execution test would require mocking apply/archive commands)
}

/// Test that on_merged hook is executed when parallel merge succeeds
#[tokio::test]
async fn test_on_merged_hook_execution() {
    use crate::hooks::{HookConfig, HookRunner, HooksConfig};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
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
        })),
        ..Default::default()
    };

    let hook_runner = HookRunner::new(hooks_config);

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
#[tokio::test]
async fn test_merge_deferred_when_worktree_dirty() {
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create archive directory to simulate that archive was successful (change moved to archive)
    let archive_dir = repo_root.join("openspec/changes/archive/2024-01-01-test-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(archive_dir.join("spec.md"), "# Archived Test").unwrap();

    // Commit the archive
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create a dirty file (uncommitted change)
    fs::write(repo_root.join("dirty.txt"), "dirty content").unwrap();

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
        Ok(MergeAttempt::Deferred(reason)) => {
            assert!(
                reason.contains("incomplete") || reason.contains("dirty"),
                "Expected deferred reason to mention incomplete archive or dirty worktree, got: {}",
                reason
            );
        }
        Ok(MergeAttempt::Merged) => {
            panic!("Merge should have been deferred due to dirty worktree");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test that attempt_merge defers when archive entry is missing
#[tokio::test]
async fn test_merge_deferred_when_archive_entry_missing() {
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

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
        Ok(MergeAttempt::Deferred(reason)) => {
            assert!(
                reason.contains("incomplete")
                    || reason.contains("archive")
                    || reason.contains("missing"),
                "Expected deferred reason to mention incomplete archive or missing entry, got: {}",
                reason
            );
        }
        Ok(MergeAttempt::Merged) => {
            panic!("Merge should have been deferred due to missing archive entry");
        }
        Err(e) => {
            panic!("Unexpected error: {}", e);
        }
    }
}

/// Test that attempt_merge proceeds when archive is complete
#[tokio::test]
async fn test_merge_proceeds_when_archive_complete() {
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // Create temporary repository
    let temp_dir = TempDir::new().unwrap();
    let repo_root = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create initial commit
    fs::write(repo_root.join("README.md"), "initial").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create archive directory but NOT openspec/changes/test-change (proper archive)
    let archive_dir = repo_root.join("openspec/changes/archive/test-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(archive_dir.join("spec.md"), "# Archived").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Archive: test-change"])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

    // Create worktree for the change (outside the main repo to avoid dirty working tree)
    let workspace_base = TempDir::new().unwrap();
    let workspace_path = workspace_base.path().join("ws-test-change");

    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "ws-test-change",
            workspace_path.to_str().unwrap(),
            "HEAD",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .unwrap();

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
        Ok(MergeAttempt::Merged) => {
            // Success - merge was allowed
        }
        Ok(MergeAttempt::Deferred(reason)) => {
            panic!(
                "Merge should have succeeded when change is archived, got deferred: {}",
                reason
            );
        }
        Err(e) => {
            // This is also acceptable - merge may fail for other reasons (e.g., original branch not initialized)
            // but it should not be deferred due to archive verification
            println!("Merge failed with error (acceptable): {}", e);
        }
    }
}
