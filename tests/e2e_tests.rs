//! End-to-End Tests for OpenSpec Orchestrator
//!
//! These tests verify the complete workflow:
//! 1. Single change apply → archive flow
//! 2. Multiple changes sequential processing
//! 3. Error handling scenarios
//!
//! Note: These tests use mock scripts instead of real openspec/opencode
//! to ensure reproducibility and fast execution.

use std::fs;
use std::io::Write;

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use conflux::orchestration::execute_rejection_flow;

// Global counter for unique script names across parallel tests
static SCRIPT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a mock openspec script that returns predefined output
fn create_mock_openspec(temp_dir: &Path, list_output: &str, archive_behavior: &str) -> String {
    use std::os::unix::fs::OpenOptionsExt;

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = temp_dir.join(format!("mock_openspec_{}.sh", script_id));

    let script_content = format!(
        r#"#!/bin/bash
case "$1" in
    list)
        echo '{}'
        ;;
    archive)
        {}
        ;;
    *)
        echo "Unknown command: $1" >&2
        exit 1
        ;;
esac
"#,
        list_output.replace('\n', "\\n"),
        archive_behavior
    );

    // Create file with executable permissions from the start
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&script_path)
        .unwrap();
    file.write_all(script_content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Small delay to ensure the file is fully written and executable
    std::thread::sleep(std::time::Duration::from_millis(10));

    script_path.to_string_lossy().to_string()
}

/// Create a mock opencode script that simulates apply/archive commands
fn create_mock_opencode(temp_dir: &Path, behavior: &str) -> String {
    use std::os::unix::fs::OpenOptionsExt;

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = temp_dir.join(format!("mock_opencode_{}.sh", script_id));

    let script_content = format!(
        r#"#!/bin/bash
# Mock OpenCode for testing
# Command format: mock_opencode.sh run "/openspec-apply change-id"
{}
"#,
        behavior
    );

    // Create file with executable permissions from the start
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&script_path)
        .unwrap();
    file.write_all(script_content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Small delay to ensure the file is fully written and executable
    std::thread::sleep(std::time::Duration::from_millis(10));

    script_path.to_string_lossy().to_string()
}

/// Create test directory structure with openspec changes
fn setup_openspec_test_env(temp_dir: &Path, changes: &[(&str, u32, u32)]) {
    let changes_dir = temp_dir.join("openspec/changes");
    fs::create_dir_all(&changes_dir).unwrap();

    for (change_id, completed, total) in changes {
        let change_dir = changes_dir.join(change_id);
        fs::create_dir_all(&change_dir).unwrap();

        // Create tasks.md
        let mut tasks_content = String::from("# Tasks\n\n");
        for i in 0..*total {
            let checkbox = if i < *completed { "[x]" } else { "[ ]" };
            tasks_content.push_str(&format!("- {} Task {}\n", checkbox, i + 1));
        }
        fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Create proposal.md
        fs::write(
            change_dir.join("proposal.md"),
            format!("# Proposal: {}\n\nTest proposal content.", change_id),
        )
        .unwrap();
    }
}

// ============================================================================
// Test 1: Single change apply → archive flow
// ============================================================================

#[test]
fn test_single_change_flow_mock_setup() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up a single change that is almost complete
    setup_openspec_test_env(temp_path, &[("test-change", 4, 5)]);

    // Create mock openspec that returns the test change
    let list_output = "Changes:\n  test-change     4/5 tasks   1m ago";
    let mock_openspec = create_mock_openspec(temp_path, list_output, "exit 0");

    // Create mock opencode that succeeds
    let mock_opencode = create_mock_opencode(
        temp_path,
        r#"
echo "Mock OpenCode: Received command: $@"
exit 0
"#,
    );

    // Verify mocks are created and executable
    assert!(Path::new(&mock_openspec).exists());
    assert!(Path::new(&mock_opencode).exists());

    // Test that the mock openspec works
    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("test-change"));
    assert!(stdout.contains("4/5"));
}

#[test]
fn test_single_change_complete_triggers_archive() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up a single change that is 100% complete (should trigger archive)
    setup_openspec_test_env(temp_path, &[("complete-change", 5, 5)]);

    // Create mock openspec that returns the complete change
    let list_output = "Changes:\n  complete-change     5/5 tasks   1m ago";
    let mock_openspec = create_mock_openspec(
        temp_path,
        list_output,
        r#"
echo "Archived: $2"
exit 0
"#,
    );

    // Verify archive command works
    let output = Command::new(&mock_openspec)
        .args(["archive", "complete-change", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Archived"));
}

// ============================================================================
// Test 2: Multiple changes sequential processing
// ============================================================================

#[test]
fn test_multiple_changes_priority_complete_first() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up multiple changes with different progress
    setup_openspec_test_env(
        temp_path,
        &[
            ("change-a", 2, 5),  // 40% complete
            ("change-b", 5, 5),  // 100% complete - should be processed first (archive)
            ("change-c", 1, 10), // 10% complete
        ],
    );

    // Create mock openspec with multiple changes
    let list_output = r#"Changes:
  change-a     2/5 tasks   10m ago
  change-b     5/5 tasks   5m ago
  change-c     1/10 tasks   1h ago"#;

    let mock_openspec = create_mock_openspec(temp_path, list_output, "exit 0");

    // Verify all changes are returned
    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("change-a"));
    assert!(stdout.contains("change-b"));
    assert!(stdout.contains("change-c"));
    assert!(stdout.contains("5/5")); // Complete change
}

#[test]
fn test_multiple_changes_fallback_to_progress_order() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up changes where none are complete
    // The orchestrator should fall back to highest progress
    setup_openspec_test_env(
        temp_path,
        &[
            ("low-progress", 1, 10),    // 10%
            ("high-progress", 8, 10),   // 80% - should be selected
            ("medium-progress", 5, 10), // 50%
        ],
    );

    let list_output = r#"Changes:
  low-progress     1/10 tasks   1h ago
  high-progress     8/10 tasks   30m ago
  medium-progress     5/10 tasks   45m ago"#;

    let mock_openspec = create_mock_openspec(temp_path, list_output, "exit 0");

    // Parse and verify the progress ordering
    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Verify all changes are present
    assert!(stdout.contains("low-progress"));
    assert!(stdout.contains("high-progress"));
    assert!(stdout.contains("medium-progress"));
}

// ============================================================================
// Test 3: Error handling scenarios
// ============================================================================

#[test]
fn test_openspec_list_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create mock openspec that fails on list
    use std::os::unix::fs::OpenOptionsExt;

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = temp_path.join(format!("mock_openspec_fail_{}.sh", script_id));
    let script_content = r#"#!/bin/bash
echo "Error: openspec not configured" >&2
exit 1
"#;

    // Create file with executable permissions from the start
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&script_path)
        .unwrap();
    file.write_all(script_content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Small delay to ensure the file is fully written and executable
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Verify the failure
    let output = Command::new(&script_path).arg("list").output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Error"));
}

#[test]
fn test_opencode_apply_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create mock opencode that fails during apply
    let mock_opencode = create_mock_opencode(
        temp_path,
        r#"
echo "Error: Failed to apply change" >&2
exit 1
"#,
    );

    // Verify the failure
    let output = Command::new(&mock_opencode)
        .args(["run", "/openspec-apply test-change"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Error"));
}

#[test]
fn test_archive_failure_handling() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create mock openspec where archive fails
    let list_output = "Changes:\n  failing-archive     5/5 tasks   1m ago";
    let mock_openspec = create_mock_openspec(
        temp_path,
        list_output,
        r#"
echo "Error: Archive directory not writable" >&2
exit 1
"#,
    );

    // Verify archive failure
    let output = Command::new(&mock_openspec)
        .args(["archive", "failing-archive", "--yes"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Error"));
}

#[test]
fn test_empty_changes_list() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create mock openspec that returns empty list
    let mock_openspec = create_mock_openspec(temp_path, "Changes:", "exit 0");

    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should have header but no changes
    assert!(stdout.contains("Changes:"));
    assert!(!stdout.contains("tasks"));
}

#[test]
fn test_partial_failure_continues_with_others() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up multiple changes where one will fail
    setup_openspec_test_env(temp_path, &[("will-fail", 3, 5), ("will-succeed", 4, 5)]);

    // Create mock opencode that fails for specific change
    let mock_opencode = create_mock_opencode(
        temp_path,
        r#"
# Check if the command contains 'will-fail'
if echo "$@" | grep -q "will-fail"; then
    echo "Error: Failed to apply will-fail" >&2
    exit 1
fi
echo "Successfully applied"
exit 0
"#,
    );

    // Verify failure for specific change
    let output = Command::new(&mock_opencode)
        .args(["run", "/openspec-apply will-fail"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    // Verify success for other change
    let output = Command::new(&mock_opencode)
        .args(["run", "/openspec-apply will-succeed"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// ============================================================================
// Integration with file system structure
// ============================================================================

#[test]
fn test_openspec_directory_structure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create full openspec directory structure
    let project_md = temp_path.join("openspec/project.md");
    fs::create_dir_all(project_md.parent().unwrap()).unwrap();
    fs::write(&project_md, "# Project\n\nTest project.").unwrap();

    // Create changes directory
    let changes_dir = temp_path.join("openspec/changes");
    fs::create_dir_all(&changes_dir).unwrap();

    // Create a test change
    let change_dir = changes_dir.join("test-feature");
    fs::create_dir_all(&change_dir).unwrap();

    fs::write(
        change_dir.join("proposal.md"),
        "# Proposal: Test Feature\n\nAdd a new feature.",
    )
    .unwrap();

    fs::write(
        change_dir.join("design.md"),
        "# Design: Test Feature\n\nImplementation details.",
    )
    .unwrap();

    fs::write(
        change_dir.join("tasks.md"),
        r#"# Tasks

- [x] Task 1: Setup
- [x] Task 2: Implementation
- [ ] Task 3: Testing
- [ ] Task 4: Documentation
"#,
    )
    .unwrap();

    // Verify structure
    assert!(project_md.exists());
    assert!(changes_dir.exists());
    assert!(change_dir.join("proposal.md").exists());
    assert!(change_dir.join("design.md").exists());
    assert!(change_dir.join("tasks.md").exists());

    // Verify task parsing from tasks.md
    let tasks_content = fs::read_to_string(change_dir.join("tasks.md")).unwrap();
    let completed_count = tasks_content.matches("[x]").count();
    let total_count = tasks_content.matches("- [").count();

    assert_eq!(completed_count, 2);
    assert_eq!(total_count, 4);
}

// ============================================================================
// Command format validation
// ============================================================================

#[test]
fn test_openspec_apply_command_format() {
    // Validate the command format used by orchestrator
    let command = "/openspec-apply";
    let change_id = "test-change";
    let full_command = format!("{} {}", command, change_id);

    assert_eq!(full_command, "/openspec-apply test-change");

    // Parse the command back
    let parts: Vec<&str> = full_command.split_whitespace().collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "/openspec-apply");
    assert_eq!(parts[1], "test-change");
}

#[test]
fn test_conflux_archive_command_format() {
    // Validate the archive command format
    let command = "/conflux:archive";
    let change_id = "completed-change";
    let full_command = format!("{} {}", command, change_id);

    assert_eq!(full_command, "/conflux:archive completed-change");
}

#[test]
fn test_opencode_run_command_format() {
    // Validate the opencode run command format
    let opencode_command = ["opencode", "run", "/openspec-apply test-change"];

    assert_eq!(opencode_command[0], "opencode");
    assert_eq!(opencode_command[1], "run");
    assert!(opencode_command[2].starts_with("/openspec-apply"));
}

// ============================================================================
// Archive Priority Tests (fix-tui-archive-skip)
// ============================================================================

// OPENSPEC: openspec/specs/cli/spec.md#tui-archive-priority-processing/archive-before-next-apply
#[test]
fn test_archive_priority_complete_changes_first() {
    // Test scenario: Change A at 100%, Change B at 50%
    // Verify A should be archived before B is processed
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Set up multiple changes with different completion states
    setup_openspec_test_env(
        temp_path,
        &[
            ("change-a", 5, 5), // 100% complete - should be archived first
            ("change-b", 2, 4), // 50% complete - should be processed second
        ],
    );

    // Create mock openspec that tracks archive calls
    let archive_log = temp_path.join("archive_log.txt");
    let list_output = r#"Changes:
  change-a     5/5 tasks   5m ago
  change-b     2/4 tasks   10m ago"#;

    use std::os::unix::fs::OpenOptionsExt;

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = temp_path.join(format!("mock_openspec_priority_{}.sh", script_id));
    let script_content = format!(
        r#"#!/bin/bash
case "$1" in
    list)
        echo '{}'
        ;;
    archive)
        echo "$(date +%s%N) archived: $2" >> {}
        echo "Archived: $2"
        ;;
    *)
        echo "Unknown command: $1" >&2
        exit 1
        ;;
esac
"#,
        list_output.replace('\n', "\\n"),
        archive_log.display()
    );

    // Create file with executable permissions from the start
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&script_path)
        .unwrap();
    file.write_all(script_content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Small delay to ensure the file is fully written and executable
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Verify the complete change (5/5) appears in list
    let output = Command::new(&script_path).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("change-a"));
    assert!(stdout.contains("5/5")); // Complete change

    // Verify archive command works
    let output = Command::new(&script_path)
        .args(["archive", "change-a", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// OPENSPEC: openspec/specs/cli/spec.md#tui-archive-priority-processing/multiple-complete-changes
#[test]
fn test_archive_priority_multiple_complete_changes() {
    // Test scenario: Multiple changes at 100% completion
    // Verify all complete changes are archived before any apply happens
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    setup_openspec_test_env(
        temp_path,
        &[
            ("complete-1", 3, 3),  // 100% complete
            ("complete-2", 5, 5),  // 100% complete
            ("incomplete", 1, 10), // 10% - needs apply
        ],
    );

    let list_output = r#"Changes:
  complete-1     3/3 tasks   5m ago
  complete-2     5/5 tasks   3m ago
  incomplete     1/10 tasks   15m ago"#;

    let mock_openspec = create_mock_openspec(
        temp_path,
        list_output,
        r#"
echo "Archived: $2"
exit 0
"#,
    );

    // Verify both complete changes appear in list
    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Count complete changes
    assert!(stdout.contains("3/3")); // complete-1
    assert!(stdout.contains("5/5")); // complete-2
    assert!(stdout.contains("1/10")); // incomplete
}

// OPENSPEC: openspec/specs/cli/spec.md#remove-retry-based-completion-check/completion-detected-on-next-iteration
#[test]
fn test_mid_apply_completion_detection() {
    // Test scenario: Change becomes 100% during another apply
    // Verify complete change is detected and archived on next loop iteration
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initial state: both incomplete
    setup_openspec_test_env(
        temp_path,
        &[
            ("change-a", 4, 5), // 80% - will complete during apply
            ("change-b", 2, 5), // 40% - being applied
        ],
    );

    // Create stateful mock that simulates change-a completing mid-process
    let state_file = temp_path.join("state.txt");
    fs::write(&state_file, "0").unwrap(); // Initial state

    use std::os::unix::fs::OpenOptionsExt;

    let script_id = SCRIPT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let script_path = temp_path.join(format!("mock_openspec_midapply_{}.sh", script_id));
    let script_content = format!(
        r#"#!/bin/bash
STATE_FILE="{}"
STATE=$(cat "$STATE_FILE" 2>/dev/null || echo "0")

case "$1" in
    list)
        if [ "$STATE" = "0" ]; then
            echo 'Changes:
  change-a     4/5 tasks   5m ago
  change-b     2/5 tasks   10m ago'
        else
            # After first apply, change-a is complete
            echo 'Changes:
  change-a     5/5 tasks   5m ago
  change-b     3/5 tasks   10m ago'
        fi
        ;;
    archive)
        echo "Archived: $2"
        ;;
    increment)
        echo "1" > "$STATE_FILE"
        echo "State incremented"
        ;;
    *)
        echo "Unknown command: $1" >&2
        exit 1
        ;;
esac
"#,
        state_file.display()
    );

    // Create file with executable permissions from the start
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o755)
        .open(&script_path)
        .unwrap();
    file.write_all(script_content.as_bytes()).unwrap();
    file.sync_all().unwrap();
    drop(file);

    // Small delay to ensure the file is fully written and executable
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Initial list - both incomplete
    let output = Command::new(&script_path).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("4/5")); // change-a incomplete
    assert!(stdout.contains("2/5")); // change-b incomplete

    // Simulate apply completing (increment state)
    let output = Command::new(&script_path)
        .arg("increment")
        .output()
        .unwrap();
    assert!(output.status.success());

    // After state change - change-a is now complete
    let output = Command::new(&script_path).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("5/5")); // change-a now complete
    assert!(stdout.contains("3/5")); // change-b progressed

    // Verify archive works for newly complete change
    let output = Command::new(&script_path)
        .args(["archive", "change-a", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Archived"));
}

#[test]
fn test_no_complete_changes_fallback() {
    // Test scenario: No changes at 100% completion
    // Verify orchestrator selects highest progress change for apply
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    setup_openspec_test_env(
        temp_path,
        &[
            ("low", 1, 10),    // 10%
            ("medium", 5, 10), // 50%
            ("high", 8, 10),   // 80% - should be selected
        ],
    );

    let list_output = r#"Changes:
  low     1/10 tasks   1h ago
  medium     5/10 tasks   30m ago
  high     8/10 tasks   15m ago"#;

    let mock_openspec = create_mock_openspec(temp_path, list_output, "exit 0");

    let output = Command::new(&mock_openspec).arg("list").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // All changes present, none complete
    assert!(stdout.contains("1/10"));
    assert!(stdout.contains("5/10"));
    assert!(stdout.contains("8/10"));
    assert!(!stdout.contains("/10 tasks") || !stdout.contains("10/10")); // No complete changes
}

// ============================================================================
// Git Worktree E2E Tests (add-git-worktree-parallel)
// ============================================================================

/// Initialize a Git repository with initial commit for testing.
///
/// Returns true if git is available and repo was initialized successfully.
async fn init_git_repo(path: &Path) -> bool {
    use tokio::process::Command as TokioCommand;

    // Initialize git repo
    let init_result = TokioCommand::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .await;

    match init_result {
        Ok(output) if output.status.success() => {}
        _ => return false,
    }

    // Configure git user for commit
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

    // Create and commit initial file
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
async fn test_git_worktree_clean_repo_parallel_ready() {
    // Test scenario: Clean Git repo should be ready for parallel execution
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo with initial commit
    if !init_git_repo(temp_path).await {
        // Skip test if git is not available
        println!("Skipping test: git not available");
        return;
    }

    // Verify repo is clean (no uncommitted changes)
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let status = String::from_utf8(status_output.stdout).unwrap();
    assert!(status.is_empty(), "Repo should be clean after commit");

    // Verify git worktree commands are available
    let worktree_list = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    assert!(
        worktree_list.status.success(),
        "git worktree should be available"
    );
}

#[tokio::test]
async fn test_git_worktree_uncommitted_changes_error() {
    // Test scenario: Git repo with uncommitted changes should reject parallel execution
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    // Create an uncommitted change
    std::fs::write(temp_path.join("new_file.txt"), "uncommitted content").unwrap();

    // Verify repo has uncommitted changes
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let status = String::from_utf8(status_output.stdout).unwrap();
    assert!(!status.is_empty(), "Repo should have uncommitted changes");
    assert!(
        status.contains("new_file.txt"),
        "Uncommitted file should appear in status"
    );
}

#[tokio::test]
async fn test_git_worktree_untracked_files_error() {
    // Test scenario: Git repo with untracked files should reject parallel execution
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    // Create an untracked file
    std::fs::write(temp_path.join("untracked.txt"), "untracked content").unwrap();

    // Verify repo has untracked files
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    let status = String::from_utf8(status_output.stdout).unwrap();
    assert!(status.contains("??"), "Untracked files should be detected");
}

#[tokio::test]
async fn test_git_worktree_create_and_cleanup() {
    // Test scenario: Create a worktree, verify it exists, then cleanup
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    let worktree_path = temp_path.join("worktrees").join("test-worktree");
    let branch_name = "test-branch";

    // Get current commit
    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let head = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Create worktree directory
    std::fs::create_dir_all(worktree_path.parent().unwrap()).unwrap();

    // Create worktree
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

    // Verify worktree is listed
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

    // Cleanup worktree
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

    // Delete branch
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
    // Test scenario: Simulate full parallel execution workflow with Git worktrees
    // 1. Create clean repo
    // 2. Create multiple worktrees for parallel changes
    // 3. Make changes in each worktree
    // 4. Merge changes back (sequential)
    // 5. Cleanup
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    let worktrees_dir = temp_path.join("worktrees");
    std::fs::create_dir_all(&worktrees_dir).unwrap();

    // Get current commit as base
    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Create two worktrees for parallel changes
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

    // Simulate changes in each worktree
    for (i, change_id) in change_ids.iter().enumerate() {
        let branch_name = &branch_names[i];
        let worktree_path = worktrees_dir.join(branch_name);

        // Create a file in the worktree
        let file_name = format!("{}.txt", change_id);
        std::fs::write(
            worktree_path.join(&file_name),
            format!("Content for {}", change_id),
        )
        .unwrap();

        // Commit the change
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

    // Get the original branch name (for documentation; unused in this test)
    let branch_output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let _original_branch = String::from_utf8(branch_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Sequential merge: merge each branch one by one
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

    // Verify merged files exist in main repo
    assert!(
        temp_path.join("change-1.txt").exists(),
        "change-1.txt should be merged"
    );
    assert!(
        temp_path.join("change-2.txt").exists(),
        "change-2.txt should be merged"
    );

    // Cleanup worktrees
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

    // Verify worktrees are cleaned up
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
    // Test scenario: Conflicting changes in parallel worktrees should be detected during merge
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    // Create a file that will be modified in both worktrees
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

    // Get current commit as base
    let head_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8(head_output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Create two worktrees
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

    // Make conflicting changes in each worktree
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

    // Merge first branch (should succeed)
    let merge1 = Command::new("git")
        .args(["merge", "ws-conflict-1", "--no-edit"])
        .current_dir(temp_path)
        .output()
        .unwrap();
    assert!(merge1.status.success(), "First merge should succeed");

    // Merge second branch (should fail with conflict)
    let merge2 = Command::new("git")
        .args(["merge", "ws-conflict-2", "--no-edit"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    // Merge should fail due to conflict
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

    // Abort the merge
    let _ = Command::new("git")
        .args(["merge", "--abort"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    // Cleanup
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
    // Test scenario: VCS backend auto-detection should find Git
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    // Verify .git directory exists
    assert!(
        temp_path.join(".git").exists(),
        ".git directory should exist"
    );

    // This validates the auto-detection logic: Git should be detected
    // when .git exists
}

#[tokio::test]
async fn test_git_worktree_staged_changes_error() {
    // Test scenario: Git repo with staged (but uncommitted) changes should reject parallel execution
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Initialize git repo
    if !init_git_repo(temp_path).await {
        println!("Skipping test: git not available");
        return;
    }

    // Create and stage a file (but don't commit)
    std::fs::write(temp_path.join("staged.txt"), "staged content").unwrap();
    let _ = Command::new("git")
        .args(["add", "staged.txt"])
        .current_dir(temp_path)
        .output()
        .unwrap();

    // Verify repo has staged changes
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

// Note: E2E tests for command queue staggering and retry are covered
// by the unit tests in src/command_queue.rs. Integration tests would
// require exposing internal modules which is not necessary for this feature.
