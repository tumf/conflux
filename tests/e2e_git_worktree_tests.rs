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
