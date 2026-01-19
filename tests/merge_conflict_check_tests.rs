//! Integration tests for git merge-tree conflict detection
//!
//! These tests verify that git merge-tree behaves as expected:
//! 1. Exit code 1 indicates conflicts
//! 2. Exit code 0 indicates clean merge
//! 3. Stdout contains tree OID and conflict info for conflicts
//! 4. Stdout contains only tree OID for clean merges

use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tokio::process::Command;

async fn run_git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| format!("Failed to execute git: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tokio::test]
async fn test_git_merge_tree_conflict_detection() {
    // This test verifies that git merge-tree behaves as expected
    // and that our implementation correctly interprets its output

    // Setup repo with conflict
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    run_git(&["init"], repo_path).await.unwrap();
    run_git(&["config", "user.name", "Test User"], repo_path)
        .await
        .unwrap();
    run_git(&["config", "user.email", "test@example.com"], repo_path)
        .await
        .unwrap();

    // Create initial file and commit
    fs::write(repo_path.join("file.txt"), "initial content\n").unwrap();
    run_git(&["add", "file.txt"], repo_path).await.unwrap();
    run_git(&["commit", "-m", "Initial commit"], repo_path)
        .await
        .unwrap();

    // Create branch and modify file
    run_git(&["checkout", "-b", "feature"], repo_path)
        .await
        .unwrap();
    fs::write(repo_path.join("file.txt"), "feature content\n").unwrap();
    run_git(&["add", "file.txt"], repo_path).await.unwrap();
    run_git(&["commit", "-m", "Feature change"], repo_path)
        .await
        .unwrap();

    // Go back to main and make conflicting change
    run_git(&["checkout", "main"], repo_path).await.unwrap();
    fs::write(repo_path.join("file.txt"), "main content\n").unwrap();
    run_git(&["add", "file.txt"], repo_path).await.unwrap();
    run_git(&["commit", "-m", "Main change"], repo_path)
        .await
        .unwrap();

    // Get commits
    let head_commit = run_git(&["rev-parse", "HEAD"], repo_path).await.unwrap();
    let head_commit = head_commit.trim();
    let feature_commit = run_git(&["rev-parse", "feature"], repo_path).await.unwrap();
    let feature_commit = feature_commit.trim();
    let merge_base = run_git(&["merge-base", head_commit, feature_commit], repo_path)
        .await
        .unwrap();
    let merge_base = merge_base.trim();

    // Test git merge-tree with conflict
    let output = Command::new("git")
        .args([
            "merge-tree",
            "--write-tree",
            "--merge-base",
            merge_base,
            head_commit,
            feature_commit,
        ])
        .current_dir(repo_path)
        .output()
        .await
        .unwrap();

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("=== Git merge-tree output (with conflict) ===");
    println!("Exit code: {}", exit_code);
    println!("Stdout:\n{}", stdout);
    println!("Stderr:\n{}", stderr);

    // Verify exit code 1 indicates conflict
    assert_eq!(exit_code, 1, "Expected exit code 1 for conflict");

    // Verify stdout contains tree OID and conflict info
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "Expected stdout to contain tree OID");

    // First line should be tree OID (40 hex chars)
    let tree_oid = lines[0].trim();
    assert_eq!(tree_oid.len(), 40, "Expected 40-char tree OID");
    assert!(
        tree_oid.chars().all(|c| c.is_ascii_hexdigit()),
        "Expected hex OID"
    );

    // Should have conflict file info or messages
    assert!(lines.len() > 1, "Expected conflict info after tree OID");
}

#[tokio::test]
async fn test_git_merge_tree_clean_merge() {
    // Test that clean merges return exit code 0

    // Setup repo without conflict
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    run_git(&["init"], repo_path).await.unwrap();
    run_git(&["config", "user.name", "Test User"], repo_path)
        .await
        .unwrap();
    run_git(&["config", "user.email", "test@example.com"], repo_path)
        .await
        .unwrap();

    // Create initial file and commit
    fs::write(repo_path.join("file.txt"), "initial content\n").unwrap();
    run_git(&["add", "file.txt"], repo_path).await.unwrap();
    run_git(&["commit", "-m", "Initial commit"], repo_path)
        .await
        .unwrap();

    // Create branch and add new file (no conflict)
    run_git(&["checkout", "-b", "feature"], repo_path)
        .await
        .unwrap();
    fs::write(repo_path.join("new_file.txt"), "new content\n").unwrap();
    run_git(&["add", "new_file.txt"], repo_path).await.unwrap();
    run_git(&["commit", "-m", "Add new file"], repo_path)
        .await
        .unwrap();

    // Go back to main
    run_git(&["checkout", "main"], repo_path).await.unwrap();

    // Get commits
    let head_commit = run_git(&["rev-parse", "HEAD"], repo_path).await.unwrap();
    let head_commit = head_commit.trim();
    let feature_commit = run_git(&["rev-parse", "feature"], repo_path).await.unwrap();
    let feature_commit = feature_commit.trim();
    let merge_base = run_git(&["merge-base", head_commit, feature_commit], repo_path)
        .await
        .unwrap();
    let merge_base = merge_base.trim();

    // Test git merge-tree without conflict
    let output = Command::new("git")
        .args([
            "merge-tree",
            "--write-tree",
            "--merge-base",
            merge_base,
            head_commit,
            feature_commit,
        ])
        .current_dir(repo_path)
        .output()
        .await
        .unwrap();

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("=== Git merge-tree output (clean merge) ===");
    println!("Exit code: {}", exit_code);
    println!("Stdout:\n{}", stdout);
    println!("Stderr:\n{}", stderr);

    // Verify exit code 0 indicates clean merge
    assert_eq!(exit_code, 0, "Expected exit code 0 for clean merge");

    // Verify stdout contains only tree OID (single line)
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "Expected single line (tree OID) for clean merge"
    );

    let tree_oid = lines[0].trim();
    assert_eq!(tree_oid.len(), 40, "Expected 40-char tree OID");
    assert!(
        tree_oid.chars().all(|c| c.is_ascii_hexdigit()),
        "Expected hex OID"
    );
}
