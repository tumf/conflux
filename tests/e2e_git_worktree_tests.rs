#![cfg(feature = "heavy-tests")]

//! Git worktree / real-boundary E2E integration tests.
//!
//! These tests intentionally use real git repositories, worktree commands, and
//! filesystem/process boundaries. They are integration/e2e coverage, not unit tests.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use conflux::orchestration::execute_rejection_flow;

#[path = "support/shared_test_support.rs"]
mod shared_test_support;

static SCRIPT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Initialize a Git repository with initial commit for testing.
/// Returns true if git is available and repo was initialized successfully.
async fn init_git_repo(path: &Path) -> bool {
    use tokio::process::Command as TokioCommand;

    let init_result = TokioCommand::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .await;

    match init_result {
        Ok(output) if output.status.success() => {}
        _ => return false,
    }

    let _ = TokioCommand::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(path)
        .output()
        .await;

    let _ = TokioCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(path)
        .output()
        .await;

    std::fs::write(path.join("README.md"), "# Test Project\n").unwrap();
    let _ = TokioCommand::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .await;

    let commit_result = TokioCommand::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(path)
        .output()
        .await;

    matches!(commit_result, Ok(output) if output.status.success())
}

#[tokio::test]
async fn test_git_worktree_create_and_cleanup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    let worktree_path = temp_path.join("worktrees").join("test-worktree");
    let branch_name = "test-branch";

    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let head = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

    let create_output = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            "-b",
            branch_name,
            &head,
        ])
        .current_dir(temp_path)
        .output()
        .unwrap();

    assert!(
        create_output.status.success(),
        "Worktree creation should succeed: {}",
        String::from_utf8_lossy(&create_output.stderr)
    );
    assert!(worktree_path.exists(), "Worktree directory should exist");

    let list_output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let list = String::from_utf8(list_output.stdout).unwrap();
    assert!(
        list.contains("test-worktree"),
        "Worktree should appear in list"
    );

    let remove_output = Command::new("git")
        .args([
            "worktree",
            "remove",
            worktree_path.to_str().unwrap(),
            "--force",
        ])
        .current_dir(temp_path)
        .output()
        .unwrap();

    assert!(
        remove_output.status.success(),
        "Worktree removal should succeed"
    );
    assert!(
        !worktree_path.exists(),
        "Worktree directory should be removed"
    );

    let branch_delete = Command::new("git")
        .args(["branch", "-D", branch_name])
        .current_dir(temp_path)
        .output()
        .unwrap();

    assert!(
        branch_delete.status.success(),
        "Branch deletion should succeed"
    );
}

#[tokio::test]
async fn test_git_worktree_parallel_execution_flow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    let worktrees_dir = temp_path.join("worktrees");
    std::fs::create_dir_all(&worktrees_dir).unwrap();

    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    let change_ids = ["change-1", "change-2"];
    let mut branch_names = Vec::new();

    for change_id in &change_ids {
        let branch_name = format!("ws-{}", change_id);
        let worktree_path = worktrees_dir.join(&branch_name);

        let create_output = Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                &branch_name,
                &base_commit,
            ])
            .current_dir(temp_path)
            .output()
            .unwrap();

        assert!(
            create_output.status.success(),
            "Worktree creation for {} should succeed: {}",
            change_id,
            String::from_utf8_lossy(&create_output.stderr)
        );

        branch_names.push(branch_name);
    }

    for (i, change_id) in change_ids.iter().enumerate() {
        let branch_name = &branch_names[i];
        let worktree_path = worktrees_dir.join(branch_name);

        let file_name = format!("{}.txt", change_id);
        std::fs::write(
            worktree_path.join(&file_name),
            format!("Content for {}", change_id),
        )
        .unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(&worktree_path)
            .output()
            .unwrap();

        let commit_output = Command::new("git")
            .args(["commit", "-m", &format!("Apply: {}", change_id)])
            .current_dir(&worktree_path)
            .output()
            .unwrap();

        assert!(
            commit_output.status.success(),
            "Commit in {} should succeed",
            change_id
        );
    }

    for branch_name in &branch_names {
        let merge_output = Command::new("git")
            .args(["merge", branch_name, "--no-edit"])
            .current_dir(temp_path)
            .output()
            .unwrap();

        assert!(
            merge_output.status.success(),
            "Merge of {} should succeed: {}",
            branch_name,
            String::from_utf8_lossy(&merge_output.stderr)
        );
    }

    assert!(
        temp_path.join("change-1.txt").exists(),
        "change-1.txt should be merged"
    );
    assert!(
        temp_path.join("change-2.txt").exists(),
        "change-2.txt should be merged"
    );

    for branch_name in &branch_names {
        let worktree_path = worktrees_dir.join(branch_name);

        let _ = Command::new("git")
            .args([
                "worktree",
                "remove",
                worktree_path.to_str().unwrap(),
                "--force",
            ])
            .current_dir(temp_path)
            .output()
            .unwrap();

        let _ = Command::new("git")
            .args(["branch", "-D", branch_name])
            .current_dir(temp_path)
            .output()
            .unwrap();
    }

    let final_list = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let list = String::from_utf8(final_list.stdout).unwrap();
    assert!(
        !list.contains("ws-change"),
        "Worktrees should be cleaned up"
    );
}

#[tokio::test]
async fn test_git_worktree_conflict_detection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    std::fs::write(temp_path.join("shared.txt"), "original content\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "Add shared file"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let worktrees_dir = temp_path.join("worktrees");
    std::fs::create_dir_all(&worktrees_dir).unwrap();

    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    let worktree1 = worktrees_dir.join("ws-conflict-1");
    let worktree2 = worktrees_dir.join("ws-conflict-2");

    let _ = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree1.to_str().unwrap(),
            "-b",
            "ws-conflict-1",
            &base_commit,
        ])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args([
            "worktree",
            "add",
            worktree2.to_str().unwrap(),
            "-b",
            "ws-conflict-2",
            &base_commit,
        ])
        .current_dir(temp_path)
        .output()
        .unwrap();

    std::fs::write(worktree1.join("shared.txt"), "content from worktree 1\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree1)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "Change from worktree 1"])
        .current_dir(&worktree1)
        .output()
        .unwrap();

    std::fs::write(worktree2.join("shared.txt"), "content from worktree 2\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&worktree2)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["commit", "-m", "Change from worktree 2"])
        .current_dir(&worktree2)
        .output()
        .unwrap();

    let merge1 = Command::new("git")
        .args(["merge", "ws-conflict-1", "--no-edit"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    assert!(merge1.status.success(), "First merge should succeed");

    let merge2 = Command::new("git")
        .args(["merge", "ws-conflict-2", "--no-edit"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    assert!(
        !merge2.status.success(),
        "Second merge should fail with conflict"
    );

    let stderr = String::from_utf8_lossy(&merge2.stderr);
    let stdout = String::from_utf8_lossy(&merge2.stdout);
    let combined = format!("{}\n{}", stdout, stderr);

    assert!(
        combined.contains("CONFLICT")
            || combined.contains("conflict")
            || combined.contains("Merge conflict"),
        "Output should indicate conflict: {}",
        combined
    );

    let _ = Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["worktree", "remove", worktree1.to_str().unwrap(), "--force"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["worktree", "remove", worktree2.to_str().unwrap(), "--force"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["branch", "-D", "ws-conflict-1"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let _ = Command::new("git")
        .args(["branch", "-D", "ws-conflict-2"])
        .current_dir(temp_path)
        .output()
        .unwrap();
}

#[tokio::test]
async fn test_vcs_backend_auto_detection_git() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    assert!(
        temp_path.join(".git").exists(),
        ".git directory should exist"
    );
}

#[tokio::test]
async fn test_git_worktree_staged_changes_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    std::fs::write(temp_path.join("staged.txt"), "staged content").unwrap();
    let _ = Command::new("git")
        .args(["add", "staged.txt"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let status = String::from_utf8(status_output.stdout).unwrap();
    assert!(
        status.contains("A"),
        "Staged file should be detected with 'A' status"
    );
    assert!(!status.is_empty(), "Repo should have staged changes");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_blocked_rejection_flow_end_to_end_creates_marker_and_removes_worktree() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_root = temp_dir.path();

    if !init_git_repo(repo_root).await {
        println!("Skipping test: git not available");
        return;
    }

    let change_id = "blocked-e2e";
    let change_dir = repo_root.join("openspec/changes").join(change_id);
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("proposal.md"), "# proposal\n").unwrap();
    fs::write(change_dir.join("tasks.md"), "- [ ] task\n").unwrap();

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mock_bin = repo_root.join(format!("mock_bin_{}", script_id));
    fs::create_dir_all(&mock_bin).unwrap();
    let mock_openspec = mock_bin.join("openspec");

    use std::os::unix::fs::OpenOptionsExt;
    let mut openspec_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&mock_openspec)
        .unwrap();
    openspec_file
        .write_all(
            b"#!/bin/bash\nif [ \"$1\" = \"resolve\" ]; then\n  exit 0\nfi\necho \"unexpected openspec command\" >&2\nexit 1\n",
        )
        .unwrap();
    openspec_file.sync_all().unwrap();
    drop(openspec_file);

    let _env_guard = shared_test_support::env_lock();
    let original_path = std::env::var("PATH").unwrap_or_default();
    unsafe {
        std::env::set_var("PATH", format!("{}:{}", mock_bin.display(), original_path));
    }

    let base_branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()
        .unwrap();
    assert!(base_branch.status.success());
    let base_branch = String::from_utf8(base_branch.stdout)
        .unwrap()
        .trim()
        .to_string();

    let worktree_path = repo_root.join(".worktrees").join(change_id);
    fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

    let add_output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &format!("wt/{}", change_id),
            worktree_path.to_str().unwrap(),
            &base_branch,
        ])
        .current_dir(repo_root)
        .output()
        .unwrap();
    assert!(add_output.status.success());

    let result = execute_rejection_flow(
        change_id,
        "E2E acceptance blocked",
        &worktree_path,
        &base_branch,
        repo_root,
    )
    .await;

    unsafe {
        std::env::set_var("PATH", original_path);
    }

    assert!(
        result.is_ok(),
        "rejection flow should succeed in e2e: {:?}",
        result
    );

    let rejected_marker = change_dir.join("REJECTED.md");
    assert!(
        rejected_marker.exists(),
        "REJECTED.md must exist after rejection"
    );
    let content = fs::read_to_string(rejected_marker).unwrap();
    assert!(content.contains("change_id: blocked-e2e"));
    assert!(content.contains("reason: E2E acceptance blocked"));

    let list_output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .unwrap();
    assert!(list_output.status.success());
    let list_text = String::from_utf8(list_output.stdout).unwrap();
    assert!(
        !list_text.contains(worktree_path.to_str().unwrap()),
        "rejected worktree should be removed"
    );
}

// ── Upstream integration (real Git / bare remote / process boundaries) ──────
//
// These cases deliberately exercise real repositories, a real local bare
// remote, real `git` subprocesses, and real verification-command processes.
// They are integration/e2e coverage, not unit tests, and stay behind
// `heavy-tests` so the default suite is unaffected.

mod upstream_integration_support {
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;

    use conflux::upstream::checkpoint::BaseLaneState;
    use conflux::upstream::coordinator::{SchedulerOutcome, UpstreamCoordinator};
    use conflux::upstream::git_ops::GitUpstreamOps;
    use conflux::upstream::options::UpstreamIntegrationConfig;
    use conflux::upstream::ports::{
        NoopUpstreamObserver, PortResult, RepairAttemptResult, RepairRequest, UpstreamRepairAgent,
    };
    use conflux::upstream::verify::CommandVerifier;

    pub use conflux::upstream::checkpoint::CheckpointTrigger;
    pub use conflux::upstream::coordinator::{
        scan_pending_publications, FinalizeOutcome, PublicationOutcome, UpstreamStepOutcome,
    };

    pub fn git(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .unwrap_or_else(|e| panic!("git {:?} failed to start: {}", args, e));
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    pub fn git_allow_failure(cwd: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn configure_identity(path: &Path) {
        git(path, &["config", "user.email", "test@example.com"]);
        git(path, &["config", "user.name", "Test User"]);
        git(path, &["config", "commit.gpgsign", "false"]);
    }

    pub fn write_and_commit(path: &Path, file: &str, contents: &str, message: &str) -> String {
        let target = path.join(file);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&target, contents).unwrap();
        git(path, &["add", "."]);
        git(path, &["commit", "-m", message]);
        git(path, &["rev-parse", "HEAD"])
    }

    /// A base repository on branch `main` with a local bare remote `origin`.
    pub struct Fixture {
        pub _dir: tempfile::TempDir,
        pub repo: std::path::PathBuf,
        pub remote: std::path::PathBuf,
        pub clone2: std::path::PathBuf,
    }

    /// Build the fixture, or `None` when git is unavailable.
    pub fn fixture() -> Option<Fixture> {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote.git");
        let repo = dir.path().join("repo");
        let clone2 = dir.path().join("other");
        std::fs::create_dir_all(&repo).unwrap();

        if !git_allow_failure(dir.path(), &["init", "--bare", "-b", "main", "remote.git"]) {
            return None;
        }
        if !git_allow_failure(&repo, &["init", "-b", "main"]) {
            return None;
        }
        configure_identity(&repo);
        write_and_commit(&repo, "README.md", "# base\n", "Initial commit");
        git(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&repo, &["push", "-u", "origin", "main"]);

        // A second clone stands in for another actor advancing the remote.
        git(
            dir.path(),
            &["clone", remote.to_str().unwrap(), clone2.to_str().unwrap()],
        );
        configure_identity(&clone2);

        Some(Fixture {
            _dir: dir,
            repo,
            remote,
            clone2,
        })
    }

    /// Add a `Merge change:` integration commit with real archive tree evidence.
    pub fn add_cumulative_change_merge(repo: &Path, change_id: &str) -> String {
        let base = git(repo, &["rev-parse", "HEAD"]);
        git(repo, &["checkout", "-b", &format!("wt-{}", change_id)]);
        // Active change directory, then archived: exactly what a real archive does.
        write_and_commit(
            repo,
            &format!("openspec/changes/archive/{}/proposal.md", change_id),
            "# archived\n",
            &format!("archive {}", change_id),
        );
        git(repo, &["checkout", "main"]);
        git(repo, &["reset", "--hard", &base]);
        git(
            repo,
            &[
                "merge",
                "--no-ff",
                "-m",
                &format!("Merge change: {}", change_id),
                &format!("wt-{}", change_id),
            ],
        );
        git(repo, &["rev-parse", "HEAD"])
    }

    /// Record the publication-required marker the base lane writes after an
    /// opted-in change integrates into cumulative base.
    ///
    /// This is a real commit in a real repository, so everything derived from it
    /// (restart refusal, retry routing, recovery) is exercised against genuine
    /// Git evidence rather than a fixture flag.
    pub fn mark_publication_required(repo: &Path, change_id: &str) -> String {
        let message = conflux::upstream::publication::format_publication_marker_message(
            change_id, "origin", "main",
        );
        git(repo, &["commit", "--allow-empty", "-m", &message]);
        git(repo, &["rev-parse", "HEAD"])
    }

    /// Integrate a change into cumulative base exactly as an opted-in run does:
    /// the `Merge change:` integration commit, then its publication marker.
    pub fn integrate_and_mark(repo: &Path, change_id: &str) -> String {
        add_cumulative_change_merge(repo, change_id);
        mark_publication_required(repo, change_id)
    }

    pub fn ops(repo: &Path) -> GitUpstreamOps {
        GitUpstreamOps::new(repo)
    }

    /// Verification command that always succeeds, executed as a real process.
    pub const PASSING_COMMAND: &str = "exit 0";

    /// Verification command that always fails, executed as a real process.
    pub const FAILING_COMMAND: &str = "exit 1";

    /// A repair agent that runs a real shell script in the repository.
    pub struct ScriptRepairAgent {
        pub repo: std::path::PathBuf,
        pub script: String,
        pub max_attempts: u32,
        pub calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl UpstreamRepairAgent for ScriptRepairAgent {
        fn max_attempts(&self) -> u32 {
            self.max_attempts
        }

        async fn repair(&self, _request: &RepairRequest) -> PortResult<RepairAttemptResult> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let status = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&self.script)
                .current_dir(&self.repo)
                .status()
                .await
                .unwrap();
            Ok(RepairAttemptResult {
                command_success: status.success(),
            })
        }
    }

    /// A repair agent that must never be invoked.
    ///
    /// `budget` is the bounded retry budget Conflux owns; a zero budget proves a
    /// failure stalls without ever reaching the agent.
    pub struct ForbiddenRepairAgent;

    pub struct ZeroBudgetRepairAgent;

    #[async_trait::async_trait]
    impl UpstreamRepairAgent for ZeroBudgetRepairAgent {
        fn max_attempts(&self) -> u32 {
            0
        }

        async fn repair(&self, request: &RepairRequest) -> PortResult<RepairAttemptResult> {
            panic!(
                "zero-budget repair agent must not be invoked for cause {:?}",
                request.cause
            );
        }
    }

    #[async_trait::async_trait]
    impl UpstreamRepairAgent for ForbiddenRepairAgent {
        fn max_attempts(&self) -> u32 {
            2
        }

        async fn repair(&self, request: &RepairRequest) -> PortResult<RepairAttemptResult> {
            panic!(
                "repair agent must not be invoked for cause {:?}",
                request.cause
            );
        }
    }

    pub fn coordinator(
        repo: &Path,
        verify_command: &str,
        repair: Arc<dyn UpstreamRepairAgent>,
    ) -> UpstreamCoordinator {
        UpstreamCoordinator::new(
            UpstreamIntegrationConfig::new("origin", verify_command),
            "main",
            Arc::new(GitUpstreamOps::new(repo)),
            Arc::new(CommandVerifier::new(verify_command, repo)),
            repair,
            Arc::new(NoopUpstreamObserver),
        )
    }

    pub fn clean_lane() -> BaseLaneState {
        BaseLaneState::clean()
    }

    pub fn after_drain() -> CheckpointTrigger {
        CheckpointTrigger::AfterDrain
    }

    pub fn drained() -> SchedulerOutcome {
        SchedulerOutcome::DrainedSuccessfully
    }
}

#[tokio::test]
async fn upstream_integration_e2e_noop_when_remote_already_integrated() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);

    let outcome = coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();

    assert!(
        matches!(outcome, UpstreamStepOutcome::NoOp { .. }),
        "outcome: {:?}",
        outcome
    );
    assert_eq!(
        git(&fx.repo, &["rev-parse", "HEAD"]),
        head_before,
        "an ancestry-proven no-op must not create a commit"
    );
}

#[tokio::test]
async fn upstream_integration_e2e_remote_advance_creates_trailer_bearing_merge() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // Local cumulative work.
    add_cumulative_change_merge(&fx.repo, "local-change");
    // Another actor advances the remote.
    let remote_sha = write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();

    let merge_sha = match outcome {
        UpstreamStepOutcome::Integrated { merge_sha } => merge_sha,
        other => panic!("expected integration, got {:?}", other),
    };

    let message = git(&fx.repo, &["log", "-1", "--format=%B", &merge_sha]);
    assert!(
        message.contains("Cflx-Upstream-Remote: origin"),
        "{}",
        message
    );
    assert!(
        message.contains("Cflx-Upstream-Branch: main"),
        "{}",
        message
    );
    assert!(
        message.contains(&format!("Cflx-Upstream-SHA: {}", remote_sha)),
        "{}",
        message
    );

    let parents = git(&fx.repo, &["log", "-1", "--format=%P", &merge_sha]);
    assert_eq!(
        parents.split_whitespace().count(),
        2,
        "must be a --no-ff merge"
    );
    assert!(parents.split_whitespace().any(|p| p == remote_sha));
}

#[tokio::test]
async fn upstream_integration_e2e_strictly_remote_ahead_still_merges() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // No local work at all: the remote is strictly ahead.
    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();

    assert!(
        matches!(outcome, UpstreamStepOutcome::Integrated { .. }),
        "strictly remote-ahead history is still integrated with --no-ff: {:?}",
        outcome
    );
    let parents = git(&fx.repo, &["log", "-1", "--format=%P"]);
    assert_eq!(parents.split_whitespace().count(), 2);
}

#[tokio::test]
async fn upstream_integration_e2e_first_parent_history_classification() {
    use conflux::upstream::coordinator::validate_initial_fetch;
    use conflux::upstream::git_ops::GitUpstreamOps;
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let git_ops = GitUpstreamOps::new(&fx.repo);

    // A cumulative change integration with real archive tree evidence is accepted.
    add_cumulative_change_merge(&fx.repo, "accepted-change");
    let validation = validate_initial_fetch(&git_ops, "origin", "main")
        .await
        .unwrap()
        .expect("recognized cumulative history is publishable");
    assert_eq!(
        validation.spine.integrated_change_ids,
        vec!["accepted-change".to_string()]
    );

    // An unrelated first-parent commit is rejected before any mutation.
    write_and_commit(&fx.repo, "hotfix.md", "hotfix\n", "hotfix: local only");
    let rejected = validate_initial_fetch(&git_ops, "origin", "main")
        .await
        .unwrap()
        .expect_err("unrelated local-only history must be rejected");
    assert!(
        rejected.to_string().contains("unrelated commit"),
        "diagnostic: {}",
        rejected
    );
}

#[tokio::test]
async fn upstream_integration_e2e_restart_recovery_identifies_unpushed_merge() {
    use conflux::upstream::coordinator::{
        scan_unpushed_upstream_merges, upstream_recovery_refusal,
    };
    use conflux::upstream::git_ops::GitUpstreamOps;
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    add_cumulative_change_merge(&fx.repo, "local-change");
    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();

    // Simulate a crash: a new process observes only repository state.
    let git_ops = GitUpstreamOps::new(&fx.repo);
    let evidence = scan_unpushed_upstream_merges(&git_ops).await.unwrap();
    assert_eq!(
        evidence.len(),
        1,
        "the unpushed upstream merge is recovered"
    );
    assert_eq!(evidence[0].trailers.remote, "origin");
    assert_eq!(evidence[0].trailers.branch, "main");

    // A run without `-u` must refuse to continue.
    let refusal = upstream_recovery_refusal(&evidence).expect("option-less run is refused");
    assert!(refusal.to_string().contains("--integrate-upstream=origin"));

    // Deleting runtime projections cannot change the safe next action.
    let _ = std::fs::remove_dir_all(fx.repo.join(".cflx"));
    let after = scan_unpushed_upstream_merges(&git_ops).await.unwrap();
    assert_eq!(after.len(), 1);

    // After the push, the same scan reports nothing to recover.
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert!(
        matches!(outcome, FinalizeOutcome::Completed { .. }),
        "{:?}",
        outcome
    );
    let published = scan_unpushed_upstream_merges(&git_ops).await.unwrap();
    assert!(
        published.is_empty(),
        "published merges are not recovery evidence"
    );
}

#[tokio::test]
async fn upstream_integration_e2e_semantic_repair_then_reverification() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    // Real verification process: fails until `fixed` exists in the worktree.
    let verify_command = "test -f fixed";
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let repair = std::sync::Arc::new(ScriptRepairAgent {
        repo: fx.repo.clone(),
        script: "touch fixed && git add fixed && git commit -m 'repair: satisfy verification'"
            .to_string(),
        max_attempts: 2,
        calls: calls.clone(),
    });

    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);
    let mut coordinator = coordinator(&fx.repo, verify_command, repair);
    let outcome = coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();

    assert!(
        matches!(outcome, UpstreamStepOutcome::Integrated { .. }),
        "outcome: {:?}",
        outcome
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    // Repair is forward-only: the pre-repair HEAD is still an ancestor.
    assert!(git_allow_failure(
        &fx.repo,
        &["merge-base", "--is-ancestor", &head_before, "HEAD"]
    ));
}

#[tokio::test]
async fn upstream_integration_e2e_blocked_and_cancelled_outcomes_never_push() {
    use conflux::upstream::coordinator::SchedulerOutcome;
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    add_cumulative_change_merge(&fx.repo, "local-change");
    let remote_before = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);

    for outcome in [
        SchedulerOutcome::BlockedOrStalled,
        SchedulerOutcome::Cancelled,
    ] {
        let mut coordinator = coordinator(
            &fx.repo,
            PASSING_COMMAND,
            std::sync::Arc::new(ForbiddenRepairAgent),
        );
        let result = coordinator.finalize(outcome).await.unwrap();
        assert!(
            matches!(result, FinalizeOutcome::Skipped { .. }),
            "{:?}",
            result
        );
    }

    assert_eq!(
        git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before,
        "a non-drained outcome must not advance the remote"
    );
}

#[tokio::test]
async fn upstream_integration_e2e_successful_drain_pushes_once_and_confirms() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let local_head = integrate_and_mark(&fx.repo, "local-change");

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert_eq!(
        outcome,
        FinalizeOutcome::Completed {
            pushed_head: local_head.clone()
        }
    );

    let remote_sha = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(
        remote_sha.starts_with(&local_head),
        "remote: {}",
        remote_sha
    );

    // At most one successful push: a repeat finalization does not push again.
    let repeat = coordinator.finalize(drained()).await.unwrap();
    assert_eq!(
        repeat,
        FinalizeOutcome::Completed {
            pushed_head: local_head
        }
    );
}

#[tokio::test]
async fn upstream_integration_e2e_zero_change_run_manufactures_no_history() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);
    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert_eq!(outcome, FinalizeOutcome::NoWork);
    assert_eq!(git(&fx.repo, &["rev-parse", "HEAD"]), head_before);

    // A remote-only advance updates observation only.
    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert_eq!(outcome, FinalizeOutcome::NoWork);
    assert_eq!(
        git(&fx.repo, &["rev-parse", "HEAD"]),
        head_before,
        "a remote-only advance must not create a synthetic local merge"
    );
}

#[tokio::test]
async fn upstream_integration_e2e_push_race_returns_to_integration_then_succeeds() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // Local work plus a concurrent remote advance: the pre-push check must
    // suppress the stale push, integrate, reverify, and only then publish.
    integrate_and_mark(&fx.repo, "local-change");
    let racing_sha = write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.finalize(drained()).await.unwrap();

    let pushed = match outcome {
        FinalizeOutcome::Completed { pushed_head } => pushed_head,
        other => panic!("expected completion, got {:?}", other),
    };

    // The racing revision was integrated rather than force-pushed over.
    assert!(git_allow_failure(
        &fx.repo,
        &["merge-base", "--is-ancestor", &racing_sha, &pushed]
    ));
    let remote_sha = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(remote_sha.starts_with(&pushed));
}

#[tokio::test]
async fn upstream_integration_e2e_hook_rejected_push_stalls_without_agent() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    integrate_and_mark(&fx.repo, "local-change");

    // A real pre-receive hook rejection: not a race, and the local tree is clean,
    // so this must stall with no agent invocation.
    let hook = fx.remote.join("hooks").join("pre-receive");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert!(
        matches!(outcome, FinalizeOutcome::Stalled { .. }),
        "outcome: {:?}",
        outcome
    );
    assert!(coordinator.pushed_head().is_none());
}

#[tokio::test]
async fn upstream_integration_e2e_checkpoint_batches_results_behind_one_fetch() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );

    // First completed result starts the checkpoint and integrates.
    let first = coordinator
        .checkpoint(
            CheckpointTrigger::BeforeBaseIntegration,
            &clean_lane(),
            Some("change-a"),
        )
        .await
        .unwrap();
    assert!(matches!(first, UpstreamStepOutcome::Integrated { .. }));

    // A second result arriving after release is a deterministic no-op, not a
    // second merge: the remote has not advanced again.
    let second = coordinator
        .checkpoint(
            CheckpointTrigger::BeforeBaseIntegration,
            &clean_lane(),
            Some("change-b"),
        )
        .await
        .unwrap();
    assert!(
        matches!(second, UpstreamStepOutcome::NoOp { .. }),
        "{:?}",
        second
    );

    let merges = git(&fx.repo, &["log", "--merges", "--format=%s", "HEAD"]);
    assert_eq!(
        merges
            .lines()
            .filter(|line| line.starts_with("Merge upstream:"))
            .count(),
        1,
        "one upstream advance produces exactly one merge"
    );
}

#[tokio::test]
async fn upstream_integration_e2e_dirty_base_defers_entire_checkpoint() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);
    let dirty = conflux::upstream::checkpoint::BaseLaneState {
        base_dirty_reason: Some("uncommitted changes".to_string()),
        lane_busy_reason: None,
    };

    let outcome = coordinator
        .checkpoint(
            CheckpointTrigger::BeforeBaseIntegration,
            &dirty,
            Some("change-a"),
        )
        .await
        .unwrap();
    assert!(
        matches!(outcome, UpstreamStepOutcome::Deferred { .. }),
        "{:?}",
        outcome
    );
    assert_eq!(git(&fx.repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(coordinator.queued_results(), vec!["change-a".to_string()]);
}

#[tokio::test]
async fn upstream_integration_e2e_stale_result_verification_runs_against_current_base() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // A worktree accepted against an older base is represented by a file the
    // upstream advance is incompatible with. The complete verification command
    // is a real process reading the current tree.
    write_and_commit(&fx.clone2, "upstream.md", "conflicting\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        "test ! -f upstream.md",
        std::sync::Arc::new(ZeroBudgetRepairAgent),
    );
    let outcome = coordinator
        .verify_base_result("stale-change")
        .await
        .unwrap();
    assert!(
        matches!(outcome, UpstreamStepOutcome::Integrated { .. }),
        "the current base still satisfies the command before integration"
    );

    // After integrating upstream, the same completed result fails verification
    // and the base lane must stay closed.
    let integrated = coordinator
        .checkpoint(after_drain(), &clean_lane(), None)
        .await
        .unwrap();
    assert!(
        matches!(integrated, UpstreamStepOutcome::Stalled { .. }),
        "{:?}",
        integrated
    );

    let blocked = coordinator
        .verify_base_result("stale-change")
        .await
        .unwrap();
    match blocked {
        UpstreamStepOutcome::Stalled { reason } => assert!(reason.contains("stale-change")),
        other => panic!("expected stall, got {:?}", other),
    }
    assert!(coordinator.pushed_head().is_none());
}

// ── Per-change upstream publication (real Git / bare remote / processes) ────
//
// These cases prove the change-scoped publication contract against genuine
// repositories, a real local bare remote, real `git` subprocesses, and real
// verification-command processes. A stubbed publication, run-only wiring,
// missing crash evidence, a premature `merged`, or an unconfirmed push all fail
// here.

#[tokio::test]
async fn per_change_upstream_e2e_publishes_one_change_and_confirms_remotely() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let head = integrate_and_mark(&fx.repo, "alpha");
    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );

    let outcome = coordinator.publish_change("alpha").await.unwrap();

    assert_eq!(
        outcome,
        PublicationOutcome::Published { head: head.clone() }
    );
    let observed = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(
        observed.starts_with(&head),
        "remote must actually contain the published revision: {}",
        observed
    );

    // Once published, the marker is no longer outstanding work.
    git(&fx.repo, &["fetch", "origin", "main"]);
    let pending = scan_pending_publications(&ops(&fx.repo)).await.unwrap();
    assert!(pending.is_empty(), "pending: {:?}", pending);
}

#[tokio::test]
async fn per_change_upstream_e2e_persistent_process_publishes_multiple_changes() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // One long-lived coordinator, as a persistent local TUI keeps: publication
    // happens at each change boundary and never waits for scheduler drain.
    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );

    let alpha_head = integrate_and_mark(&fx.repo, "alpha");
    let first = coordinator.publish_change("alpha").await.unwrap();
    assert_eq!(
        first,
        PublicationOutcome::Published {
            head: alpha_head.clone()
        }
    );

    let beta_head = integrate_and_mark(&fx.repo, "beta");
    let second = coordinator.publish_change("beta").await.unwrap();
    assert_eq!(
        second,
        PublicationOutcome::Published {
            head: beta_head.clone()
        }
    );

    assert_ne!(alpha_head, beta_head);
    let observed = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(observed.starts_with(&beta_head), "remote: {}", observed);
    assert!(git_allow_failure(
        &fx.repo,
        &["merge-base", "--is-ancestor", &alpha_head, &beta_head]
    ));
}

#[tokio::test]
async fn per_change_upstream_e2e_records_durable_identity_without_an_upstream_merge() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // The remote never advances, so no upstream merge commit is ever created.
    // The publication marker is therefore the only crash evidence available.
    let marker = integrate_and_mark(&fx.repo, "alpha");

    let log = git(&fx.repo, &["log", "--format=%s"]);
    assert!(!log.contains("Merge upstream:"), "log: {}", log);

    let pending = scan_pending_publications(&ops(&fx.repo)).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].commit, marker);
    assert_eq!(pending[0].trailers.change_id, "alpha");
    assert_eq!(pending[0].trailers.remote, "origin");
    assert_eq!(pending[0].trailers.branch, "main");
}

#[tokio::test]
async fn per_change_upstream_e2e_option_less_restart_refuses_marked_unpublished_integration() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    integrate_and_mark(&fx.repo, "alpha");
    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);
    let remote_before = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let refusal = conflux::upstream::ensure_no_unpushed_upstream_recovery(&fx.repo)
        .await
        .expect_err("an option-less restart must refuse marked unpublished history");

    let message = refusal.to_string();
    assert!(message.contains("alpha"), "{}", message);
    assert!(
        message.contains("--integrate-upstream=origin"),
        "{}",
        message
    );
    assert!(message.contains("--upstream-verify-command"), "{}", message);
    assert!(
        message.contains("it is not merged"),
        "the change must not be reported as terminal merged: {}",
        message
    );

    // Refusal happens before any orchestration mutation.
    assert_eq!(git(&fx.repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before
    );
}

#[tokio::test]
async fn per_change_upstream_e2e_enabled_restart_resumes_publication() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let head = integrate_and_mark(&fx.repo, "alpha");

    // A brand-new process: nothing but repository and remote evidence exists.
    let mut restarted = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let pending = scan_pending_publications(&ops(&fx.repo)).await.unwrap();
    assert_eq!(pending.len(), 1);

    let outcome = restarted
        .publish_change(&pending[0].trailers.change_id)
        .await
        .unwrap();

    assert_eq!(
        outcome,
        PublicationOutcome::Published { head: head.clone() }
    );
    let observed = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(observed.starts_with(&head), "remote: {}", observed);
}

#[tokio::test]
async fn per_change_upstream_e2e_failed_verification_suppresses_push_and_stays_resumable() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let head = integrate_and_mark(&fx.repo, "alpha");
    let remote_before = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let mut failing = coordinator(
        &fx.repo,
        FAILING_COMMAND,
        std::sync::Arc::new(ZeroBudgetRepairAgent),
    );
    let outcome = failing.publish_change("alpha").await.unwrap();

    assert!(
        matches!(outcome, PublicationOutcome::Stalled { .. }),
        "outcome: {:?}",
        outcome
    );
    assert!(failing.pushed_head().is_none());
    assert_eq!(
        git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before,
        "a failed verification must publish nothing"
    );

    // The change is still resumable from repository evidence.
    let pending = scan_pending_publications(&ops(&fx.repo)).await.unwrap();
    assert_eq!(pending.len(), 1);

    // Explicit retry with a working verification command completes it.
    let mut retried = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let retry = retried.publish_change("alpha").await.unwrap();
    assert_eq!(retry, PublicationOutcome::Published { head: head.clone() });
    let observed = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(observed.starts_with(&head), "remote: {}", observed);
}

#[tokio::test]
async fn per_change_upstream_e2e_remote_advance_is_integrated_before_publication() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    integrate_and_mark(&fx.repo, "alpha");
    let racing = write_and_commit(&fx.clone2, "upstream.md", "upstream\n", "upstream work");
    git(&fx.clone2, &["push", "origin", "main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.publish_change("alpha").await.unwrap();

    let published = match outcome {
        PublicationOutcome::Published { head } => head,
        other => panic!("expected publication, got {:?}", other),
    };
    assert!(
        git_allow_failure(
            &fx.repo,
            &["merge-base", "--is-ancestor", &racing, &published]
        ),
        "the remote advance must be integrated, never force-pushed over"
    );
    let observed = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    assert!(observed.starts_with(&published), "remote: {}", observed);
}

#[tokio::test]
async fn per_change_upstream_e2e_push_rejection_stalls_without_agent() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    integrate_and_mark(&fx.repo, "alpha");

    let hook = fx.remote.join("hooks").join("pre-receive");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let remote_before = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);
    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.publish_change("alpha").await.unwrap();

    assert!(
        matches!(outcome, PublicationOutcome::Stalled { .. }),
        "outcome: {:?}",
        outcome
    );
    assert!(coordinator.pushed_head().is_none());
    assert_eq!(
        git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before
    );
    assert!(!scan_pending_publications(&ops(&fx.repo))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn per_change_upstream_e2e_interrupted_push_is_confirmed_without_a_second_push() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    let head = integrate_and_mark(&fx.repo, "alpha");
    // The prior process pushed but stopped before recording confirmation.
    git(&fx.repo, &["push", "origin", "main"]);

    // A deny-everything hook proves no second push is attempted: if publication
    // tried to push again it would fail instead of confirming.
    let hook = fx.remote.join("hooks").join("pre-receive");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.publish_change("alpha").await.unwrap();

    assert_eq!(outcome, PublicationOutcome::AlreadyConfirmed { head });
}

#[tokio::test]
async fn per_change_upstream_e2e_zero_change_recovery_requires_explicit_evidence() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // Unmarked cumulative history is not recovery work.
    add_cumulative_change_merge(&fx.repo, "disabled-mode-change");
    let head_before = git(&fx.repo, &["rev-parse", "HEAD"]);
    let remote_before = git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]);

    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    assert_eq!(
        coordinator.finalize(drained()).await.unwrap(),
        FinalizeOutcome::NoWork
    );
    assert_eq!(git(&fx.repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(&fx.repo, &["ls-remote", "origin", "refs/heads/main"]),
        remote_before
    );

    // An explicit marker is recovery work, and publishing it does not
    // retroactively make the earlier disabled-mode change a published change.
    let marked = mark_publication_required(&fx.repo, "opted-in-change");
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert_eq!(
        outcome,
        FinalizeOutcome::Completed {
            pushed_head: marked
        }
    );
}

#[tokio::test]
async fn per_change_upstream_e2e_default_off_history_never_blocks_startup() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    add_cumulative_change_merge(&fx.repo, "disabled-mode-change");

    // A disabled run starts normally: ordinary terminal `merged` history is not
    // publication evidence and is never promoted.
    conflux::upstream::ensure_no_unpushed_upstream_recovery(&fx.repo)
        .await
        .expect("disabled-mode history must not block an option-less start");
    assert!(scan_pending_publications(&ops(&fx.repo))
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn per_change_upstream_e2e_run_and_local_tui_construct_one_runtime() {
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // Both frontends normalize the same raw option values, then run the same
    // startup validation against the same repository.
    let from_run =
        conflux::upstream::resolve_frontend_upstream_config(Some("origin"), Some("exit 0"), None)
            .expect("run options")
            .expect("enabled");
    let from_tui =
        conflux::upstream::resolve_frontend_upstream_config(Some("origin"), Some("exit 0"), None)
            .expect("tui options")
            .expect("enabled");
    assert_eq!(from_run, from_tui);

    let run_runtime = conflux::upstream::prepare_upstream_integration(
        from_run, &fx.repo, true, None, true, false,
    )
    .await
    .expect("run startup validation");
    let tui_runtime = conflux::upstream::prepare_upstream_integration(
        from_tui, &fx.repo, true, None, true, false,
    )
    .await
    .expect("local TUI startup validation");

    assert_eq!(run_runtime, tui_runtime);
    assert_eq!(run_runtime.branch, "main");
}

/// Real-repository fixtures for explicit-target resume classification.
///
/// These cases use real Git repositories, real worktrees, and real archive
/// trees. They are integration/e2e evidence for `explicit-target-resume-tests`,
/// not unit coverage of the resolver decision table.
mod explicit_target_resume_support {
    use std::path::{Path, PathBuf};

    pub use conflux::orchestration::target_resolution::{
        classify_base_completion, workspace_resume_evidence, BaseCompletionEvidence,
        BaseEvidenceErrorKind, ExplicitTargetPlan, TargetClassification, TargetResolution,
        TargetResolutionOptions, WorkspaceResumeEvidence,
    };

    pub use super::upstream_integration_support::{configure_identity, git, git_allow_failure};

    pub struct Repo {
        pub _dir: tempfile::TempDir,
        pub root: PathBuf,
    }

    /// A repository on branch `main` with one commit, or `None` without git.
    pub fn repo() -> Option<Repo> {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        if !git_allow_failure(&root, &["init", "-b", "main"]) {
            return None;
        }
        configure_identity(&root);
        std::fs::write(root.join("README.md"), "# base\n").unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "Initial commit"]);
        Some(Repo { _dir: dir, root })
    }

    /// Write an active OpenSpec change directory (working copy only).
    pub fn write_active_change(root: &Path, change_id: &str) {
        let dir = root.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("proposal.md"), format!("# {change_id}\n")).unwrap();
        std::fs::write(
            dir.join("tasks.md"),
            "## Implementation Tasks\n\n- [ ] Do the work\n",
        )
        .unwrap();
    }

    /// Write an archive entry (working copy only). `entry_name` may be date-prefixed.
    pub fn write_archive_entry(root: &Path, entry_name: &str, change_id: &str) {
        let dir = root.join("openspec/changes/archive").join(entry_name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("proposal.md"), format!("# {change_id}\n")).unwrap();
        std::fs::write(
            dir.join("tasks.md"),
            "## Implementation Tasks\n\n- [x] Do the work\n",
        )
        .unwrap();
    }

    pub fn commit_all(root: &Path, message: &str) {
        git(root, &["add", "-A"]);
        git(root, &["commit", "-m", message]);
    }

    /// Add a cflx-managed worktree whose branch is the change ID.
    pub fn add_worktree(root: &Path, change_id: &str) -> PathBuf {
        let path = root.join(".openspec-worktrees").join(change_id);
        git(
            root,
            &[
                "worktree",
                "add",
                "-b",
                change_id,
                path.to_str().unwrap(),
                "HEAD",
            ],
        );
        path
    }

    /// Classify a requested target set against the repository's `main` base.
    pub async fn resolve(root: &Path, requested: &[&str], no_resume: bool) -> TargetResolution {
        let plan = ExplicitTargetPlan::new(
            requested.iter().map(|s| s.to_string()).collect(),
            "main".to_string(),
            TargetResolutionOptions { no_resume },
        );
        plan.resolve(root).await
    }

    /// Snapshot every path under the repository, for side-effect assertions.
    pub fn snapshot_tree(root: &Path) -> Vec<String> {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                out.push(
                    path.strip_prefix(base)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
                if path.is_dir() {
                    walk(&path, base, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }
}

#[tokio::test]
async fn explicit_target_resume_base_evidence_completed_for_exact_and_date_prefixed_archive() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_archive_entry(&fx.root, "exact-change", "exact-change");
    write_archive_entry(&fx.root, "2026-07-30-dated-change", "dated-change");
    commit_all(&fx.root, "Archive both changes");

    assert_eq!(
        classify_base_completion("exact-change", &fx.root, "main").await,
        BaseCompletionEvidence::Completed
    );
    assert_eq!(
        classify_base_completion("dated-change", &fx.root, "main").await,
        BaseCompletionEvidence::Completed,
        "a date-prefixed archive entry proves the same completion"
    );
}

#[tokio::test]
async fn explicit_target_resume_base_evidence_not_completed_without_archive() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    // No archive directory at all.
    assert_eq!(
        classify_base_completion("missing-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted
    );

    // An archive directory holding some other change is still NotCompleted.
    write_archive_entry(&fx.root, "other-change", "other-change");
    commit_all(&fx.root, "Archive other change");
    assert_eq!(
        classify_base_completion("missing-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted
    );
}

#[tokio::test]
async fn explicit_target_resume_base_evidence_contradictory_when_active_dir_remains() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_archive_entry(&fx.root, "2026-07-30-half-change", "half-change");
    write_active_change(&fx.root, "half-change");
    commit_all(
        &fx.root,
        "Archive half-change without removing the active directory",
    );

    let evidence = classify_base_completion("half-change", &fx.root, "main").await;
    assert!(
        matches!(evidence, BaseCompletionEvidence::Contradictory { .. }),
        "archive plus active directory must not read as completed: {evidence:?}"
    );

    // An active change directory always outranks a candidate contradiction, so
    // this target remains ordinary work rather than a failure.
    let resolution = resolve(&fx.root, &["half-change"], false).await;
    assert_eq!(
        resolution.classification_of("half-change"),
        Some(TargetClassification::Active)
    );

    // When the leftover base directory is not an active change (here: no
    // proposal.md), the contradiction reaches classification as a safe failure
    // rather than a completion skip.
    write_archive_entry(&fx.root, "2026-07-30-stub-change", "stub-change");
    let stub_dir = fx.root.join("openspec/changes/stub-change");
    std::fs::create_dir_all(&stub_dir).unwrap();
    std::fs::write(stub_dir.join("tasks.md"), "## Implementation Tasks\n").unwrap();
    commit_all(
        &fx.root,
        "Archive stub-change leaving a stub directory behind",
    );

    let resolution = resolve(&fx.root, &["stub-change"], false).await;
    assert_eq!(
        resolution.classification_of("stub-change"),
        Some(TargetClassification::EvidenceError)
    );
    assert!(resolution.already_completed_ids().is_empty());
    let message = resolution.failure_error().unwrap().to_string();
    assert!(
        message.contains("unusable change evidence: stub-change"),
        "the contradiction is reported with actionable detail: {message}"
    );
}

#[tokio::test]
async fn explicit_target_resume_base_evidence_error_on_missing_branch() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_archive_entry(&fx.root, "some-change", "some-change");
    commit_all(&fx.root, "Archive some-change");

    let evidence = classify_base_completion("some-change", &fx.root, "no-such-branch").await;
    assert!(
        matches!(
            evidence,
            BaseCompletionEvidence::EvidenceError {
                kind: BaseEvidenceErrorKind::MissingBranch,
                ..
            }
        ),
        "an unreadable base branch is an evidence error, not 'not completed': {evidence:?}"
    );

    // A Git command failure is also an evidence error: run outside any repository.
    let non_repo = tempfile::tempdir().unwrap();
    let outside = classify_base_completion("some-change", non_repo.path(), "main").await;
    assert!(
        matches!(outside, BaseCompletionEvidence::EvidenceError { .. }),
        "{outside:?}"
    );
}

#[tokio::test]
async fn explicit_target_resume_base_evidence_rejects_uncommitted_and_subject_only_archives() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    // 1. A commit subject that looks like an archive, with no tree evidence.
    std::fs::write(fx.root.join("notes.md"), "note\n").unwrap();
    commit_all(&fx.root, "Archive: subject-only-change");
    assert_eq!(
        classify_base_completion("subject-only-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted,
        "a commit subject is never accepted as completion proof"
    );

    // 2. An archive entry that exists only in the uncommitted working copy.
    write_archive_entry(
        &fx.root,
        "2026-07-30-uncommitted-change",
        "uncommitted-change",
    );
    assert_eq!(
        classify_base_completion("uncommitted-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted,
        "an uncommitted working-copy archive is not in the base tree"
    );

    // 3. Committed on another branch only: still not in base.
    git(&fx.root, &["checkout", "-b", "side"]);
    commit_all(&fx.root, "Archive uncommitted-change on side branch");
    assert_eq!(
        classify_base_completion("uncommitted-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted
    );
    assert_eq!(
        classify_base_completion("uncommitted-change", &fx.root, "side").await,
        BaseCompletionEvidence::Completed
    );
}

#[tokio::test]
async fn explicit_target_resume_workspace_evidence_requires_more_than_a_name() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "applied-change");
    commit_all(&fx.root, "Add applied-change");

    // Applied: the worktree carries the active change directory.
    let applied_ws = add_worktree(&fx.root, "applied-change");
    let evidence = workspace_resume_evidence("applied-change", &applied_ws, "main").await;
    assert!(
        matches!(evidence, WorkspaceResumeEvidence::Resumable { .. }),
        "{evidence:?}"
    );

    // Name-only: a worktree that carries no proposal for the change.
    let bare_ws = add_worktree(&fx.root, "name-only-change");
    let evidence = workspace_resume_evidence("name-only-change", &bare_ws, "main").await;
    assert!(
        matches!(evidence, WorkspaceResumeEvidence::NotResumable { .. }),
        "a matching workspace name alone is not resume evidence: {evidence:?}"
    );

    // Missing path: the workspace was removed from disk.
    let missing = fx.root.join(".openspec-worktrees/never-created");
    let evidence = workspace_resume_evidence("never-created", &missing, "main").await;
    assert!(
        matches!(evidence, WorkspaceResumeEvidence::NotResumable { .. }),
        "{evidence:?}"
    );

    // Malformed: a change directory with no proposal.md.
    let malformed_ws = add_worktree(&fx.root, "malformed-change");
    std::fs::create_dir_all(malformed_ws.join("openspec/changes/malformed-change")).unwrap();
    let evidence = workspace_resume_evidence("malformed-change", &malformed_ws, "main").await;
    assert!(
        matches!(evidence, WorkspaceResumeEvidence::NotResumable { .. }),
        "a change directory without a proposal is not resume evidence: {evidence:?}"
    );
}

#[tokio::test]
async fn explicit_target_resume_workspace_evidence_accepts_archived_not_integrated() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "archiving-change");
    commit_all(&fx.root, "Add archiving-change");

    // The worktree archived the change but the archive never reached base.
    let ws = add_worktree(&fx.root, "archiving-change");
    std::fs::remove_dir_all(ws.join("openspec/changes/archiving-change")).unwrap();
    write_archive_entry(&ws, "2026-07-30-archiving-change", "archiving-change");
    git(&ws, &["add", "-A"]);
    git(&ws, &["commit", "-m", "Archive: archiving-change"]);

    assert_eq!(
        classify_base_completion("archiving-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted,
        "base has not integrated the archive yet"
    );

    let evidence = workspace_resume_evidence("archiving-change", &ws, "main").await;
    let WorkspaceResumeEvidence::Resumable { change, .. } = evidence else {
        panic!("archived-not-integrated workspace must be resumable: {evidence:?}");
    };
    assert_eq!(change.id, "archiving-change");

    // Base no longer lists the change as active and never received the archive,
    // so only the worktree can identify the requested target.
    std::fs::remove_dir_all(fx.root.join("openspec/changes/archiving-change")).unwrap();
    commit_all(
        &fx.root,
        "Drop archiving-change from base without archiving it",
    );
    assert_eq!(
        classify_base_completion("archiving-change", &fx.root, "main").await,
        BaseCompletionEvidence::NotCompleted
    );

    let resolution = resolve(&fx.root, &["archiving-change"], false).await;
    assert_eq!(
        resolution.classification_of("archiving-change"),
        Some(TargetClassification::ResumableWorkspace)
    );
    assert_eq!(resolution.processed_ids(), vec!["archiving-change"]);
    assert!(resolution.failure_error().is_none());
}

#[tokio::test]
async fn explicit_target_resume_repeated_target_set_skips_integrated_target() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "change-one");
    write_active_change(&fx.root, "change-two");
    commit_all(&fx.root, "Add change-one and change-two");

    // First invocation: both are ordinary active work.
    let first = resolve(&fx.root, &["change-one", "change-two"], false).await;
    assert!(first.failure_error().is_none());
    assert_eq!(first.processed_ids(), vec!["change-one", "change-two"]);

    // change-one is archived and integrated into base.
    std::fs::remove_dir_all(fx.root.join("openspec/changes/change-one")).unwrap();
    write_archive_entry(&fx.root, "2026-07-30-change-one", "change-one");
    commit_all(&fx.root, "Archive: change-one");

    // Repeating the identical target set skips it instead of failing.
    let second = resolve(&fx.root, &["change-one", "change-two"], false).await;
    assert!(
        second.failure_error().is_none(),
        "a repeated completed target must not be an unknown-ID error: {:?}",
        second.failure_error().map(|e| e.to_string())
    );
    assert_eq!(second.already_completed_ids(), vec!["change-one"]);
    assert_eq!(second.processed_ids(), vec!["change-two"]);
    assert_eq!(
        second.requested_ids(),
        vec!["change-one", "change-two"],
        "request order is retained for terminal reporting"
    );
}

#[tokio::test]
async fn explicit_target_resume_active_target_wins_over_candidate_workspace() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "dual-change");
    commit_all(&fx.root, "Add dual-change");
    let ws = add_worktree(&fx.root, "dual-change");
    assert!(ws.is_dir());

    let resolution = resolve(&fx.root, &["dual-change"], false).await;
    assert_eq!(
        resolution.classification_of("dual-change"),
        Some(TargetClassification::Active),
        "an active change is ordinary work even when a managed worktree exists"
    );
    assert!(resolution.resumable_ids().is_empty());
    assert!(ws.is_dir(), "classification must not remove the workspace");
}

#[tokio::test]
async fn explicit_target_resume_reports_unknown_and_duplicate_ids_together() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "known-change");
    commit_all(&fx.root, "Add known-change");

    let before = snapshot_tree(&fx.root);
    let resolution = resolve(
        &fx.root,
        &["known-change", "ghost-a", "known-change", "ghost-b"],
        false,
    )
    .await;

    let message = resolution.failure_error().unwrap().to_string();
    assert!(
        message.contains("duplicate change IDs: known-change"),
        "{message}"
    );
    assert!(
        message.contains("unknown change IDs: ghost-a, ghost-b"),
        "all unknown IDs are reported together: {message}"
    );
    assert_eq!(
        snapshot_tree(&fx.root),
        before,
        "rejection happens before any workspace is created, deleted, or mutated"
    );
}

#[tokio::test]
async fn explicit_target_resume_no_resume_keeps_completion_and_preserves_refused_workspace() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    // A base-integrated change plus a workspace-only recoverable change.
    write_active_change(&fx.root, "done-change");
    write_active_change(&fx.root, "workspace-only");
    commit_all(&fx.root, "Add both changes");

    let ws = add_worktree(&fx.root, "workspace-only");

    std::fs::remove_dir_all(fx.root.join("openspec/changes/done-change")).unwrap();
    write_archive_entry(&fx.root, "2026-07-30-done-change", "done-change");
    // Remove the active copy of workspace-only from base so only the worktree has it.
    std::fs::remove_dir_all(fx.root.join("openspec/changes/workspace-only")).unwrap();
    commit_all(
        &fx.root,
        "Archive done-change and drop workspace-only from base",
    );

    let resolution = resolve(&fx.root, &["done-change", "workspace-only"], true).await;

    assert_eq!(
        resolution.already_completed_ids(),
        vec!["done-change"],
        "--no-resume does not erase base-integrated completion"
    );
    assert_eq!(resolution.resume_refused_ids(), vec!["workspace-only"]);
    let message = resolution.failure_error().unwrap().to_string();
    assert!(message.contains("refused by --no-resume"), "{message}");
    assert!(
        ws.is_dir(),
        "a refused worktree is never implicitly deleted or replaced"
    );
}

#[tokio::test]
async fn explicit_target_resume_dry_run_classification_has_no_side_effects() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "active-change");
    write_active_change(&fx.root, "resumable-change");
    commit_all(&fx.root, "Add changes");

    let ws = add_worktree(&fx.root, "resumable-change");
    std::fs::remove_dir_all(fx.root.join("openspec/changes/resumable-change")).unwrap();
    write_archive_entry(&fx.root, "2026-07-30-completed-change", "completed-change");
    commit_all(
        &fx.root,
        "Archive completed-change, drop resumable-change from base",
    );

    let before = snapshot_tree(&fx.root);
    let head_before = git(&fx.root, &["rev-parse", "HEAD"]);

    let resolution = resolve(
        &fx.root,
        &[
            "active-change",
            "completed-change",
            "resumable-change",
            "ghost-change",
        ],
        false,
    )
    .await;

    assert_eq!(resolution.active_ids(), vec!["active-change"]);
    assert_eq!(resolution.already_completed_ids(), vec!["completed-change"]);
    assert_eq!(resolution.resumable_ids(), vec!["resumable-change"]);
    assert_eq!(resolution.unknown_ids(), vec!["ghost-change"]);

    let lines = resolution.report_lines();
    assert!(lines
        .iter()
        .any(|l| l.contains("already completed (skipped): completed-change")));
    assert!(lines
        .iter()
        .any(|l| l.contains("resumable workspaces: resumable-change")));

    assert_eq!(
        snapshot_tree(&fx.root),
        before,
        "read-only classification mutates nothing"
    );
    assert_eq!(git(&fx.root, &["rev-parse", "HEAD"]), head_before);
    assert!(
        ws.is_dir(),
        "no workspace cleanup happens during classification"
    );
}

#[tokio::test]
async fn explicit_target_resume_classifies_against_post_checkpoint_cumulative_base() {
    use explicit_target_resume_support::*;
    let Some(fx) = repo() else {
        println!("Skipping test: git not available");
        return;
    };

    write_active_change(&fx.root, "remote-completed");
    write_active_change(&fx.root, "local-change");
    commit_all(&fx.root, "Add both changes");

    let plan = ExplicitTargetPlan::new(
        vec!["remote-completed".to_string(), "local-change".to_string()],
        "main".to_string(),
        TargetResolutionOptions::default(),
    );

    // Before the checkpoint both targets are ordinary active work.
    let before = plan.resolve(&fx.root).await;
    assert_eq!(
        before.processed_ids(),
        vec!["remote-completed", "local-change"]
    );

    // The initial upstream checkpoint integrates a branch that archived one target.
    git(&fx.root, &["checkout", "-b", "upstream-work"]);
    std::fs::remove_dir_all(fx.root.join("openspec/changes/remote-completed")).unwrap();
    write_archive_entry(&fx.root, "2026-07-30-remote-completed", "remote-completed");
    commit_all(&fx.root, "Archive: remote-completed");
    git(&fx.root, &["checkout", "main"]);
    git(
        &fx.root,
        &["merge", "--no-ff", "-m", "Merge upstream", "upstream-work"],
    );

    // Classifying after the checkpoint sees the newly integrated archive.
    let after = plan.resolve(&fx.root).await;
    assert!(after.failure_error().is_none());
    assert_eq!(after.already_completed_ids(), vec!["remote-completed"]);
    assert_eq!(after.processed_ids(), vec!["local-change"]);
    assert_eq!(
        plan.resolved().await.unwrap().already_completed_ids(),
        vec!["remote-completed"],
        "the recorded resolution is available to terminal consumers"
    );
}

#[tokio::test]
async fn explicit_target_resume_all_completed_run_still_publishes_unpushed_history() {
    use explicit_target_resume_support::{
        resolve, write_active_change, write_archive_entry, TargetClassification,
    };
    use upstream_integration_support::*;
    let Some(fx) = fixture() else {
        println!("Skipping test: git not available");
        return;
    };

    // Every requested target is already archived and integrated into base, and
    // that integration is opted-in but not yet published: the marker is what
    // makes it recognized recovery work rather than terminal `merged` history.
    write_active_change(&fx.repo, "done-one");
    write_archive_entry(&fx.repo, "2026-07-30-done-one", "done-one");
    std::fs::remove_dir_all(fx.repo.join("openspec/changes/done-one")).unwrap();
    git(&fx.repo, &["add", "-A"]);
    git(&fx.repo, &["commit", "-m", "Archive: done-one"]);
    integrate_and_mark(&fx.repo, "done-two");

    let resolution = resolve(&fx.repo, &["done-one", "done-two"], false).await;
    assert!(resolution.failure_error().is_none());
    assert_eq!(
        resolution.classification_of("done-one"),
        Some(TargetClassification::AlreadyCompleted)
    );
    assert_eq!(
        resolution.classification_of("done-two"),
        Some(TargetClassification::AlreadyCompleted)
    );
    assert!(
        resolution.processed_ids().is_empty(),
        "there is no change work left to dispatch"
    );

    // A skip-only run must still verify, push, and confirm the unpublished base.
    let mut coordinator = coordinator(
        &fx.repo,
        PASSING_COMMAND,
        std::sync::Arc::new(ForbiddenRepairAgent),
    );
    let outcome = coordinator.finalize(drained()).await.unwrap();
    assert!(
        matches!(outcome, FinalizeOutcome::Completed { .. }),
        "an all-already-completed run still finalizes recognized unpublished history: {outcome:?}"
    );
    assert_eq!(
        git(&fx.repo, &["rev-parse", "HEAD"]),
        git(&fx.repo, &["rev-parse", "origin/main"]),
        "the cumulative base is published"
    );
}

// ── Remote worktree operations (real Git worktrees / teardown / merge state) ──
//
// These cases prove the parts of `secure-remote-worktree-operations` that are
// only decidable against a real repository: mandatory teardown, dirty refusal,
// conflict preservation, `on_merged` ordering, and identity retirement across a
// real remove-and-recreate. They drive the production port
// (`RemoteWorktreeOperations` over `GitWorktreeBackend`), not a fake.

#[cfg(feature = "web-monitoring")]
mod remote_worktree_support {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use conflux::config::OrchestratorConfig;
    use conflux::web::remote_control_api::worktrees::{
        RemoteWorktreeOperations, WorktreeOperations, WorktreeRegistry,
    };
    use conflux::worktree_ops::git_backend::GitWorktreeBackend;
    use conflux::worktree_ops::service::{NullEventSink, WorktreeService};

    pub use super::upstream_integration_support::{configure_identity, git, git_allow_failure};

    /// A real repository plus the production remote worktree port over it.
    pub struct RemoteFixture {
        pub _dir: tempfile::TempDir,
        pub repo: PathBuf,
        pub workspaces: PathBuf,
        pub registry: Arc<WorktreeRegistry>,
        pub port: RemoteWorktreeOperations,
    }

    /// Build the fixture, or `None` when git is unavailable.
    pub fn remote_fixture() -> Option<RemoteFixture> {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let workspaces = dir.path().join("workspaces");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&workspaces).unwrap();

        if !git_allow_failure(&repo, &["init", "-b", "main"]) {
            return None;
        }
        configure_identity(&repo);
        std::fs::write(repo.join("README.md"), "# base\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "Initial commit"]);

        let registry = Arc::new(WorktreeRegistry::new());
        let service = Arc::new(WorktreeService::new(
            Arc::new(GitWorktreeBackend::new(
                repo.clone(),
                Arc::new(OrchestratorConfig::default()),
            )),
            Arc::new(NullEventSink),
            workspaces.clone(),
        ));
        let port = RemoteWorktreeOperations::new(service, registry.clone(), repo.clone());

        Some(RemoteFixture {
            _dir: dir,
            repo,
            workspaces,
            registry,
            port,
        })
    }

    /// Register a managed, non-archived change so create eligibility passes.
    ///
    /// Committed, not just written: a base merge requires a clean working tree,
    /// so leaving the declaration untracked would make every merge case fail for
    /// a reason that has nothing to do with what is under test.
    pub fn declare_change(repo: &Path, change_id: &str) {
        let dir = repo.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("proposal.md"), "# proposal\n").unwrap();
        std::fs::write(dir.join("tasks.md"), "- [x] done\n").unwrap();
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-m", &format!("Declare {change_id}")]);
    }

    /// True when the base repository is sitting on an unresolved merge.
    pub fn merge_in_progress(repo: &Path) -> bool {
        repo.join(".git/MERGE_HEAD").exists()
    }

    /// The opaque ID currently bound to the worktree on `branch`, if any.
    pub async fn id_for_branch(port: &RemoteWorktreeOperations, branch: &str) -> Option<String> {
        port.list()
            .await
            .ok()?
            .worktrees
            .into_iter()
            .find(|worktree| worktree.branch == branch)
            .map(|worktree| worktree.worktree_id)
    }
}

#[cfg(feature = "web-monitoring")]
mod remote_worktree_tests {
    use super::remote_worktree_support::*;
    use conflux::web::remote_control_api::dto::ErrorCode;
    use conflux::web::remote_control_api::worktrees::WorktreeOperations;

    #[tokio::test]
    async fn remote_worktree_create_uses_current_base_head_and_allocates_an_id() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");
        std::fs::write(fx.repo.join("second.txt"), "second\n").unwrap();
        git(&fx.repo, &["add", "."]);
        git(&fx.repo, &["commit", "-m", "Advance base"]);
        let head = git(&fx.repo, &["rev-parse", "HEAD"]);

        fx.port.create("change-a").await.expect("create succeeds");

        let worktree_path = fx.workspaces.join("change-a");
        assert!(worktree_path.is_dir(), "the worktree directory exists");
        assert_eq!(
            git(&worktree_path, &["rev-parse", "HEAD"]),
            head,
            "the worktree is cut from current managed base HEAD"
        );

        let listed = fx.port.list().await.expect("list succeeds");
        let created = listed
            .worktrees
            .iter()
            .find(|worktree| worktree.branch == "change-a")
            .expect("the created worktree is observable");
        assert_eq!(created.worktree_id.len(), 32);
        assert_eq!(created.dirty, Some(false));
        // Repository-relative, and never the absolute root in any spelling.
        let raw = serde_json::to_string(&listed.worktrees).unwrap();
        let canonical = std::fs::canonicalize(&fx.repo).unwrap();
        assert!(
            !raw.contains(fx.repo.to_str().unwrap()),
            "leaked root: {raw}"
        );
        assert!(
            !raw.contains(canonical.to_str().unwrap()),
            "leaked canonical root: {raw}"
        );
        assert_eq!(created.path, "../workspaces/change-a");
    }

    #[tokio::test]
    async fn remote_worktree_create_refuses_an_existing_worktree_and_an_unmanaged_change() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");
        fx.port.create("change-a").await.expect("first create");

        let failure = fx
            .port
            .create("change-a")
            .await
            .expect_err("a second create must conflict");
        assert_eq!(failure.error_code, ErrorCode::WorktreeExists);

        let failure = fx
            .port
            .create("never-proposed")
            .await
            .expect_err("an unmanaged change must be refused");
        assert_eq!(failure.error_code, ErrorCode::WorktreeNotFound);
    }

    #[tokio::test]
    async fn remote_worktree_delete_runs_teardown_and_retires_the_identity() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");

        // A real `.wt/teardown` proves the delete path is not bypassing it.
        let wt_dir = fx.repo.join(".wt");
        std::fs::create_dir_all(&wt_dir).unwrap();
        let marker = fx.repo.join("teardown-ran");
        std::fs::write(
            wt_dir.join("teardown"),
            format!("#!/bin/sh\ntouch {}\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                wt_dir.join("teardown"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        git(&fx.repo, &["add", "-A"]);
        git(&fx.repo, &["commit", "-m", "Add teardown script"]);

        fx.port.create("change-a").await.expect("create succeeds");
        let id = id_for_branch(&fx.port, "change-a")
            .await
            .expect("an ID was allocated");

        fx.port.delete(&id).await.expect("delete succeeds");

        assert!(marker.exists(), "managed teardown must have run");
        assert!(!fx.workspaces.join("change-a").exists());
        assert!(fx.registry.is_retired(&id), "the ID must be retired");
        assert!(fx.registry.resolve(&id).is_none());

        let failure = fx
            .port
            .delete(&id)
            .await
            .expect_err("a retired ID addresses nothing");
        assert_eq!(failure.error_code, ErrorCode::WorktreeNotFound);
    }

    #[tokio::test]
    async fn remote_worktree_dirty_worktree_is_not_deletable() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");
        fx.port.create("change-a").await.expect("create succeeds");
        let id = id_for_branch(&fx.port, "change-a").await.unwrap();

        std::fs::write(fx.workspaces.join("change-a/dirty.txt"), "uncommitted\n").unwrap();

        let failure = fx
            .port
            .delete(&id)
            .await
            .expect_err("a dirty worktree must not be deleted");
        assert_eq!(failure.error_code, ErrorCode::WorktreeDirty);
        assert!(
            fx.workspaces.join("change-a").exists(),
            "a refused delete retains the resource"
        );
        assert!(
            !fx.registry.is_retired(&id),
            "a refused delete retains the identity binding"
        );
    }

    #[tokio::test]
    async fn remote_worktree_successful_merge_lands_on_base_and_runs_on_merged() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");
        fx.port.create("change-a").await.expect("create succeeds");

        let worktree = fx.workspaces.join("change-a");
        configure_identity(&worktree);
        std::fs::write(worktree.join("feature.txt"), "feature\n").unwrap();
        git(&worktree, &["add", "."]);
        git(&worktree, &["commit", "-m", "Add feature"]);

        let id = id_for_branch(&fx.port, "change-a").await.unwrap();
        fx.port.merge(&id).await.expect("merge succeeds");

        assert!(
            fx.repo.join("feature.txt").exists(),
            "the base contains the worktree result"
        );
        assert!(
            !merge_in_progress(&fx.repo),
            "a clean merge leaves no intermediate state"
        );
    }

    #[tokio::test]
    async fn remote_worktree_merge_conflict_preserves_state_and_blocks_further_mutation() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");
        std::fs::write(fx.repo.join("shared.txt"), "base\n").unwrap();
        git(&fx.repo, &["add", "."]);
        git(&fx.repo, &["commit", "-m", "Add shared file"]);

        fx.port.create("change-a").await.expect("create succeeds");
        let worktree = fx.workspaces.join("change-a");
        configure_identity(&worktree);
        std::fs::write(worktree.join("shared.txt"), "from worktree\n").unwrap();
        git(&worktree, &["add", "."]);
        git(&worktree, &["commit", "-m", "Worktree edit"]);

        // Diverge base so the merge genuinely conflicts.
        std::fs::write(fx.repo.join("shared.txt"), "from base\n").unwrap();
        git(&fx.repo, &["add", "."]);
        git(&fx.repo, &["commit", "-m", "Base edit"]);

        let id = id_for_branch(&fx.port, "change-a").await.unwrap();
        let failure = fx
            .port
            .merge(&id)
            .await
            .expect_err("a conflicting merge must fail the command");

        assert_eq!(failure.error_code, ErrorCode::MergeConflict);
        assert!(failure.message.contains("shared.txt"));
        assert!(failure.message.contains("local_or_tui_required"));
        assert!(
            merge_in_progress(&fx.repo),
            "the intermediate merge state must be preserved, not aborted"
        );

        // Until a local resolve or abort happens, further mutation is refused.
        let busy = fx
            .port
            .merge(&id)
            .await
            .expect_err("the root stays busy after a preserved conflict");
        assert_eq!(busy.error_code, ErrorCode::RootBusy);
        let busy_delete = fx
            .port
            .delete(&id)
            .await
            .expect_err("the root stays busy for deletion too");
        assert_eq!(busy_delete.error_code, ErrorCode::RootBusy);
        assert!(
            merge_in_progress(&fx.repo),
            "the retained conflict state is not discarded by the refusals"
        );
    }

    #[tokio::test]
    async fn remote_worktree_recreated_worktree_receives_a_new_identity() {
        let Some(fx) = remote_fixture() else {
            println!("Skipping test: git not available");
            return;
        };
        declare_change(&fx.repo, "change-a");

        fx.port.create("change-a").await.expect("first create");
        let first = id_for_branch(&fx.port, "change-a").await.unwrap();
        fx.port.delete(&first).await.expect("delete succeeds");

        fx.port.create("change-a").await.expect("second create");
        let second = id_for_branch(&fx.port, "change-a").await.unwrap();

        assert_ne!(
            first, second,
            "a worktree recreated at the same path must not inherit the retired ID"
        );
        assert!(fx.registry.is_retired(&first));
        assert!(fx.registry.resolve(&second).is_some());
    }
}
