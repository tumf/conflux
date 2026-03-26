//! Regression tests for parallel workspace reuse and state detection.
//!
//! These tests verify that when an existing worktree is reused via `--parallel`
//! (resume mode), the detected workspace state correctly gates which pipeline
//! stages run.  Before the fix, a workspace in `Archived` state would be treated
//! as a fresh start and re-enter the apply loop, sometimes silently returning
//! "already complete" due to the archive-fallback in `check_task_progress`, and
//! then creating a spurious duplicate "Apply: <change_id>" commit.

use crate::execution::apply::check_task_progress;
use crate::execution::state::{detect_workspace_state, WorkspaceState};
use crate::parallel::workspace::get_or_create_workspace;
use crate::parallel::ParallelEvent;
use crate::vcs::{
    VcsBackend, VcsResult, VcsWarning, Workspace, WorkspaceInfo, WorkspaceManager, WorkspaceStatus,
};
use async_trait::async_trait;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::SystemTime;
use tempfile::TempDir;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn init_git_repo(repo_root: &Path) {
    StdCommand::new("git")
        .args(["init", "-b", "main"])
        .current_dir(repo_root)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_root)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_root)
        .output()
        .unwrap();
}

fn git_commit(repo_root: &Path, message: &str) {
    fs::write(repo_root.join("test.txt"), message).unwrap();
    StdCommand::new("git")
        .args(["add", "-A"])
        .current_dir(repo_root)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_root)
        .output()
        .unwrap();
}

// ---------------------------------------------------------------------------
// Minimal WorkspaceManager mock for get_or_create_workspace tests
// ---------------------------------------------------------------------------

struct ResumeTestManager {
    /// When Some, `find_existing_workspace` returns this workspace info.
    existing: Option<WorkspaceInfo>,
    /// The workspace path returned for all workspace operations.
    workspace_path: PathBuf,
}

#[async_trait]
impl WorkspaceManager for ResumeTestManager {
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
        Ok("abc123".to_string())
    }
    async fn create_workspace(
        &mut self,
        change_id: &str,
        _base_revision: Option<&str>,
    ) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: format!("ws-{}", change_id),
            path: self.workspace_path.clone(),
            change_id: change_id.to_string(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Created,
        })
    }
    fn update_workspace_status(&mut self, _workspace_name: &str, _status: WorkspaceStatus) {}
    async fn find_existing_workspace(
        &mut self,
        _change_id: &str,
    ) -> VcsResult<Option<WorkspaceInfo>> {
        Ok(self.existing.clone())
    }
    async fn reuse_workspace(&mut self, info: &WorkspaceInfo) -> VcsResult<Workspace> {
        Ok(Workspace {
            name: info.workspace_name.clone(),
            path: self.workspace_path.clone(),
            change_id: info.change_id.clone(),
            base_revision: "base".to_string(),
            status: WorkspaceStatus::Applying,
        })
    }
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
        vec![]
    }
    async fn list_worktree_change_ids(&self) -> VcsResult<HashSet<String>> {
        Ok(HashSet::new())
    }
    fn conflict_resolution_prompt(&self) -> &'static str {
        ""
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
        Ok("workspace-rev".to_string())
    }
    async fn get_status(&self) -> VcsResult<String> {
        Ok(String::new())
    }
    async fn get_log_for_revisions(&self, _revisions: &[String]) -> VcsResult<String> {
        Ok(String::new())
    }
    async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
        Ok(vec![])
    }
    fn forget_workspace_sync(&self, _workspace_name: &str) {}
    fn repo_root(&self) -> &Path {
        &self.workspace_path
    }
    fn original_branch(&self) -> Option<String> {
        Some("main".to_string())
    }
}

// ---------------------------------------------------------------------------
// get_or_create_workspace: was_resumed flag
// ---------------------------------------------------------------------------

/// Creating a new workspace must return was_resumed=false.
#[tokio::test]
async fn test_get_or_create_workspace_new_returns_not_resumed() {
    let tmp = TempDir::new().unwrap();
    let mut manager = ResumeTestManager {
        existing: None,
        workspace_path: tmp.path().to_path_buf(),
    };
    let (tx, _rx) = mpsc::channel::<ParallelEvent>(16);
    let event_tx = Some(tx);

    let (_ws, was_resumed) = get_or_create_workspace(
        &mut manager,
        "my-change",
        "base-rev",
        false,
        &HashSet::new(),
        &event_tx,
    )
    .await
    .expect("get_or_create_workspace should succeed");

    assert!(!was_resumed, "new workspace must report was_resumed=false");
}

/// Reusing an existing workspace must return was_resumed=true.
#[tokio::test]
async fn test_get_or_create_workspace_reuse_returns_resumed() {
    let tmp = TempDir::new().unwrap();
    let workspace_info = WorkspaceInfo {
        workspace_name: "ws-my-change".to_string(),
        path: tmp.path().to_path_buf(),
        change_id: "my-change".to_string(),
        last_modified: SystemTime::UNIX_EPOCH,
    };
    let mut manager = ResumeTestManager {
        existing: Some(workspace_info),
        workspace_path: tmp.path().to_path_buf(),
    };
    let (tx, _rx) = mpsc::channel::<ParallelEvent>(16);
    let event_tx = Some(tx);

    let (_ws, was_resumed) = get_or_create_workspace(
        &mut manager,
        "my-change",
        "base-rev",
        false,
        &HashSet::new(),
        &event_tx,
    )
    .await
    .expect("get_or_create_workspace should succeed");

    assert!(
        was_resumed,
        "existing workspace must report was_resumed=true"
    );
}

/// When no_resume=true, a new workspace is always created (was_resumed=false),
/// even when an existing workspace is present.
#[tokio::test]
async fn test_get_or_create_workspace_no_resume_creates_fresh() {
    let tmp = TempDir::new().unwrap();
    let workspace_info = WorkspaceInfo {
        workspace_name: "ws-my-change".to_string(),
        path: tmp.path().to_path_buf(),
        change_id: "my-change".to_string(),
        last_modified: SystemTime::UNIX_EPOCH,
    };
    let mut manager = ResumeTestManager {
        existing: Some(workspace_info),
        workspace_path: tmp.path().to_path_buf(),
    };
    let (tx, _rx) = mpsc::channel::<ParallelEvent>(16);
    let event_tx = Some(tx);

    let (_ws, was_resumed) = get_or_create_workspace(
        &mut manager,
        "my-change",
        "base-rev",
        true, // no_resume
        &HashSet::new(),
        &event_tx,
    )
    .await
    .expect("get_or_create_workspace should succeed");

    assert!(
        !was_resumed,
        "no_resume=true must bypass existing workspace and report was_resumed=false"
    );
}

// ---------------------------------------------------------------------------
// detect_workspace_state: regression for the "Archived silently treated as
// fresh start" scenario described in the proposal.
// ---------------------------------------------------------------------------

/// A workspace that contains an archive entry for the change and has a clean
/// working tree must be detected as Archived, NOT Created.
///
/// Before the fix, code that consumed `detect_workspace_state` could miss the
/// Archived state and proceed into the apply loop, relying on the
/// `check_task_progress` archive fallback to return "all tasks complete" and
/// exit the loop immediately — creating a spurious duplicate Apply commit.
#[tokio::test]
async fn test_detect_workspace_state_archived_not_treated_as_created() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();
    init_git_repo(repo_root);
    git_commit(repo_root, "Initial commit");

    // Simulate a workspace branch
    StdCommand::new("git")
        .args(["checkout", "-b", "workspace-my-change"])
        .current_dir(repo_root)
        .output()
        .unwrap();

    // Move change files into archive (simulating a completed archive step)
    let archive_dir = repo_root.join("openspec/changes/archive/my-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("tasks.md"),
        "## Tasks\n- [x] Do something\n",
    )
    .unwrap();
    git_commit(repo_root, "Archive: my-change");

    let state = detect_workspace_state("my-change", repo_root, "main")
        .await
        .expect("detect_workspace_state must not fail");

    // The state must be Archived, not Created.  If it were Created, dispatch
    // would run the full apply pipeline on an already-archived workspace.
    assert_eq!(
        state,
        WorkspaceState::Archived,
        "workspace with committed archive entry must be detected as Archived, not Created"
    );
}

/// A workspace whose archive entry lives under a date-prefixed directory must
/// also be detected as Archived.
#[tokio::test]
async fn test_detect_workspace_state_archived_date_prefixed_not_treated_as_created() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();
    init_git_repo(repo_root);
    git_commit(repo_root, "Initial commit");

    StdCommand::new("git")
        .args(["checkout", "-b", "workspace-dated-change"])
        .current_dir(repo_root)
        .output()
        .unwrap();

    let archive_dir = repo_root.join("openspec/changes/archive/2024-03-15-dated-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(archive_dir.join("tasks.md"), "## Tasks\n- [x] Done\n").unwrap();
    git_commit(repo_root, "Archive: dated-change");

    let state = detect_workspace_state("dated-change", repo_root, "main")
        .await
        .expect("detect_workspace_state must not fail");

    assert_eq!(
        state,
        WorkspaceState::Archived,
        "workspace with date-prefixed archive entry must be detected as Archived"
    );
}

// ---------------------------------------------------------------------------
// check_task_progress: archive fallback regression
// ---------------------------------------------------------------------------

/// When tasks.md is only available in the archive directory (Archiving or
/// Archived state), `check_task_progress` must still return the progress so
/// the apply loop can exit cleanly for the Archiving state.
///
/// After the fix, callers in Archived/Merged state are short-circuited before
/// `check_task_progress` is ever called, so reaching the archive fallback
/// signals an Archiving (mid-archive) scenario rather than an inadvertent
/// "fresh start" detection.
#[test]
fn test_check_task_progress_archive_fallback_returns_progress() {
    let tmp = TempDir::new().unwrap();
    let workspace_path = tmp.path();

    // Place tasks.md in the archive location (change dir does not exist)
    let archive_dir = workspace_path.join("openspec/changes/archive/my-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("tasks.md"),
        "## Implementation Tasks\n- [x] Task one\n- [x] Task two\n",
    )
    .unwrap();

    let progress = check_task_progress(workspace_path, "my-change")
        .expect("check_task_progress must succeed when tasks.md is in archive dir");

    assert_eq!(progress.completed, 2);
    assert_eq!(progress.total, 2);
}

/// When neither the active change directory nor the archive directory contains
/// tasks.md, `check_task_progress` must return an error, not a false "complete"
/// result.
#[test]
fn test_check_task_progress_missing_returns_error() {
    let tmp = TempDir::new().unwrap();
    let workspace_path = tmp.path();

    // No tasks.md anywhere
    let result = check_task_progress(workspace_path, "no-such-change");
    assert!(
        result.is_err(),
        "missing tasks.md must produce an error, not a false complete result"
    );
}

// ---------------------------------------------------------------------------
// Regression: mixed Archiving + Archived restart
//
// These tests guard the fix for the bug where a parallel restart with one
// workspace in `Archiving` state and another already in `Archived` state
// would leave the `Archived` workspace as `not queued` instead of advancing
// it to merge handling.
// ---------------------------------------------------------------------------

/// A workspace with uncommitted archive files (Archiving) and a workspace with
/// a committed archive entry (Archived) must be detected as distinct states.
///
/// Before the dispatch fix, both could have been handled the same way (silent
/// no-op return), but the regression relies on the states being correctly
/// distinguished first.
#[tokio::test]
async fn test_mixed_restart_archiving_and_archived_states_are_distinct() {
    // ── Workspace A: Archiving (archive files moved but not committed) ────────
    let tmp_archiving = TempDir::new().unwrap();
    let path_archiving = tmp_archiving.path();
    init_git_repo(path_archiving);
    git_commit(path_archiving, "Initial commit");

    StdCommand::new("git")
        .args(["checkout", "-b", "workspace-change-archiving"])
        .current_dir(path_archiving)
        .output()
        .unwrap();

    // Simulate archive files moved into place but NOT yet committed (dirty tree).
    let archive_dir_a = path_archiving.join("openspec/changes/archive/change-archiving");
    fs::create_dir_all(&archive_dir_a).unwrap();
    fs::write(archive_dir_a.join("proposal.md"), "# Archiving change").unwrap();
    // Do NOT commit — leave tree dirty so this looks like Archiving, not Archived.

    let state_archiving = detect_workspace_state("change-archiving", path_archiving, "main")
        .await
        .expect("detect_workspace_state must not fail for Archiving workspace");

    // ── Workspace B: Archived (archive commit is present and tree is clean) ───
    let tmp_archived = TempDir::new().unwrap();
    let path_archived = tmp_archived.path();
    init_git_repo(path_archived);
    git_commit(path_archived, "Initial commit");

    StdCommand::new("git")
        .args(["checkout", "-b", "workspace-change-archived"])
        .current_dir(path_archived)
        .output()
        .unwrap();

    let archive_dir_b = path_archived.join("openspec/changes/archive/change-archived");
    fs::create_dir_all(&archive_dir_b).unwrap();
    fs::write(archive_dir_b.join("tasks.md"), "## Tasks\n- [x] Done\n").unwrap();
    git_commit(path_archived, "Archive: change-archived");

    let state_archived = detect_workspace_state("change-archived", path_archived, "main")
        .await
        .expect("detect_workspace_state must not fail for Archived workspace");

    // The two states must be distinct.
    assert_eq!(
        state_archiving,
        WorkspaceState::Archiving,
        "workspace with uncommitted archive files must be Archiving"
    );
    assert_eq!(
        state_archived,
        WorkspaceState::Archived,
        "workspace with committed archive entry must be Archived"
    );
    assert_ne!(
        state_archiving, state_archived,
        "Archiving and Archived states must be distinguishable on restart"
    );
}

/// An `Archived` workspace must yield a readable HEAD revision so the dispatch
/// path can return `final_revision: Some(rev)` and hand off to merge handling.
///
/// This guards the fix in `dispatch_change_to_workspace`: the archived resume
/// branch now calls `get_current_commit` and returns its result as `final_revision`.
/// If `get_current_commit` were to fail the change would surface as an error
/// rather than silently disappearing from the queue.
#[tokio::test]
async fn test_archived_workspace_head_revision_is_readable() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();
    init_git_repo(repo_root);
    git_commit(repo_root, "Initial commit");

    StdCommand::new("git")
        .args(["checkout", "-b", "workspace-archive-rev"])
        .current_dir(repo_root)
        .output()
        .unwrap();

    let archive_dir = repo_root.join("openspec/changes/archive/archive-rev-change");
    fs::create_dir_all(&archive_dir).unwrap();
    fs::write(archive_dir.join("tasks.md"), "## Tasks\n- [x] Done\n").unwrap();
    git_commit(repo_root, "Archive: archive-rev-change");

    // Verify workspace state is Archived (precondition for the dispatch fix path).
    let state = detect_workspace_state("archive-rev-change", repo_root, "main")
        .await
        .expect("detect_workspace_state must succeed");
    assert_eq!(state, WorkspaceState::Archived);

    // The fix calls `get_current_commit` to obtain the revision for `final_revision`.
    // Verify it succeeds and returns a non-empty SHA so the merge handoff has a
    // valid revision.
    let rev = crate::vcs::git::commands::get_current_commit(repo_root)
        .await
        .expect("get_current_commit must succeed on a clean archived workspace");

    assert!(
        !rev.is_empty(),
        "archived workspace HEAD revision must be non-empty"
    );
    // A git SHA is 40 hex chars (full) or at least 7 (short) — the command
    // returns the full SHA, so check for a reasonable minimum length.
    assert!(
        rev.len() >= 7,
        "archived workspace HEAD revision must look like a git SHA, got: {}",
        rev
    );
}
