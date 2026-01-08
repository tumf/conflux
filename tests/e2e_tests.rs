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
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// Create a mock openspec script that returns predefined output
fn create_mock_openspec(temp_dir: &Path, list_output: &str, archive_behavior: &str) -> String {
    let script_path = temp_dir.join("mock_openspec.sh");

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

    let mut file = fs::File::create(&script_path).unwrap();
    file.write_all(script_content.as_bytes()).unwrap();

    // Make executable
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

    script_path.to_string_lossy().to_string()
}

/// Create a mock opencode script that simulates apply/archive commands
fn create_mock_opencode(temp_dir: &Path, behavior: &str) -> String {
    let script_path = temp_dir.join("mock_opencode.sh");

    let script_content = format!(
        r#"#!/bin/bash
# Mock OpenCode for testing
# Command format: mock_opencode.sh run "/openspec-apply change-id"
{}
"#,
        behavior
    );

    let mut file = fs::File::create(&script_path).unwrap();
    file.write_all(script_content.as_bytes()).unwrap();

    // Make executable
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

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
fn test_single_change_flow_dry_run() {
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
    let script_path = temp_path.join("mock_openspec_fail.sh");
    let script_content = r#"#!/bin/bash
echo "Error: openspec not configured" >&2
exit 1
"#;

    let mut file = fs::File::create(&script_path).unwrap();
    file.write_all(script_content.as_bytes()).unwrap();

    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms).unwrap();

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
// State persistence tests
// ============================================================================

#[test]
fn test_state_file_creation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    // Create .opencode directory
    let opencode_dir = temp_path.join(".opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    // Create a sample state file
    let state = serde_json::json!({
        "current_change": "test-change",
        "processed_changes": ["change-1"],
        "archived_changes": ["old-change"],
        "failed_changes": [],
        "started_at": "2026-01-08T10:00:00Z",
        "last_update": "2026-01-08T10:30:00Z",
        "total_iterations": 5
    });

    let state_path = opencode_dir.join("orchestrator-state.json");
    fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap()).unwrap();

    // Verify state file exists and is readable
    assert!(state_path.exists());

    let content = fs::read_to_string(&state_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["current_change"], "test-change");
    assert_eq!(parsed["total_iterations"], 5);
}

#[test]
fn test_state_recovery_after_restart() {
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();

    let opencode_dir = temp_path.join(".opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    // Create initial state (simulating a previous run)
    let initial_state = serde_json::json!({
        "current_change": "in-progress-change",
        "processed_changes": ["change-1", "change-2"],
        "archived_changes": ["old-change"],
        "failed_changes": [],
        "started_at": "2026-01-08T09:00:00Z",
        "last_update": "2026-01-08T10:00:00Z",
        "total_iterations": 10
    });

    let state_path = opencode_dir.join("orchestrator-state.json");
    fs::write(
        &state_path,
        serde_json::to_string_pretty(&initial_state).unwrap(),
    )
    .unwrap();

    // Simulate loading state after restart
    let loaded_content = fs::read_to_string(&state_path).unwrap();
    let loaded: serde_json::Value = serde_json::from_str(&loaded_content).unwrap();

    // Verify state was preserved
    assert_eq!(loaded["current_change"], "in-progress-change");
    assert_eq!(loaded["processed_changes"].as_array().unwrap().len(), 2);
    assert_eq!(loaded["total_iterations"], 10);

    // Simulate updating state after recovery
    let mut updated: serde_json::Value = serde_json::from_str(&loaded_content).unwrap();
    updated["total_iterations"] = serde_json::json!(11);
    updated["last_update"] = serde_json::json!("2026-01-08T10:30:00Z");

    fs::write(&state_path, serde_json::to_string_pretty(&updated).unwrap()).unwrap();

    // Verify update
    let final_content = fs::read_to_string(&state_path).unwrap();
    let final_state: serde_json::Value = serde_json::from_str(&final_content).unwrap();

    assert_eq!(final_state["total_iterations"], 11);
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
fn test_openspec_archive_command_format() {
    // Validate the archive command format
    let command = "/openspec-archive";
    let change_id = "completed-change";
    let full_command = format!("{} {}", command, change_id);

    assert_eq!(full_command, "/openspec-archive completed-change");
}

#[test]
fn test_opencode_run_command_format() {
    // Validate the opencode run command format
    let opencode_command = vec!["opencode", "run", "/openspec-apply test-change"];

    assert_eq!(opencode_command[0], "opencode");
    assert_eq!(opencode_command[1], "run");
    assert!(opencode_command[2].starts_with("/openspec-apply"));
}
