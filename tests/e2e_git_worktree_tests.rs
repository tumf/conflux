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
    pub use conflux::upstream::coordinator::{FinalizeOutcome, UpstreamStepOutcome};

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

    /// Verification command that always succeeds, executed as a real process.
    pub const PASSING_COMMAND: &str = "exit 0";

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

    add_cumulative_change_merge(&fx.repo, "local-change");
    let local_head = git(&fx.repo, &["rev-parse", "HEAD"]);

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
    add_cumulative_change_merge(&fx.repo, "local-change");
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

    add_cumulative_change_merge(&fx.repo, "local-change");

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
