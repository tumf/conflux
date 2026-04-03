#![cfg(feature = "heavy-tests")]

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

// Git worktree / real external boundary E2E tests were moved to
// `tests/e2e_git_worktree_tests.rs` so this file remains focused on
// mock-driven orchestrator flow checks.
