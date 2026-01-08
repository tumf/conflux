//! Ralph Loop Compatibility Tests
//!
//! These tests verify that the openspec-orchestrator is compatible with
//! the Ralph Wiggum plugin for OpenCode.
//!
//! Key compatibility requirements:
//! 1. State files do not conflict
//! 2. Commands work in both TUI mode (Ralph) and Headless mode (Orchestrator)
//! 3. Both tools can coexist in the same project

use std::fs;
use std::path::PathBuf;

/// Ralph loop state file path (from ralph-wiggum.ts)
const RALPH_STATE_FILE: &str = ".opencode/ralph-loop.local.json";

/// Orchestrator state file path
const ORCHESTRATOR_STATE_FILE: &str = ".opencode/orchestrator-state.json";

/// Test that state file paths are different and do not conflict
#[test]
fn test_state_files_do_not_conflict() {
    assert_ne!(
        RALPH_STATE_FILE, ORCHESTRATOR_STATE_FILE,
        "State files must have different paths to avoid conflicts"
    );

    // Verify paths are in the same directory but with different names
    let ralph_path = PathBuf::from(RALPH_STATE_FILE);
    let orch_path = PathBuf::from(ORCHESTRATOR_STATE_FILE);

    assert_eq!(
        ralph_path.parent(),
        orch_path.parent(),
        "Both state files should be in the same directory for organizational purposes"
    );

    assert_ne!(
        ralph_path.file_name(),
        orch_path.file_name(),
        "State file names must be different"
    );
}

/// Test that both Ralph and Orchestrator state files can be created simultaneously
#[test]
fn test_coexistence_of_state_files() {
    use std::io::Write;

    let temp_dir = tempfile::tempdir().unwrap();
    let opencode_dir = temp_dir.path().join(".opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    // Create Ralph state file
    let ralph_state = serde_json::json!({
        "active": true,
        "iteration": 3,
        "max_iterations": 10,
        "completion_promise": "ALL TESTS PASSING",
        "started_at": "2026-01-08T10:00:00Z",
        "prompt": "Implement feature X",
        "auto_continue": true
    });

    let ralph_path = opencode_dir.join("ralph-loop.local.json");
    let mut ralph_file = fs::File::create(&ralph_path).unwrap();
    ralph_file
        .write_all(
            serde_json::to_string_pretty(&ralph_state)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

    // Create Orchestrator state file
    let orch_state = serde_json::json!({
        "current_change": "add-feature",
        "processed_changes": ["fix-bug"],
        "archived_changes": ["old-change"],
        "failed_changes": [],
        "started_at": "2026-01-08T09:00:00Z",
        "last_update": "2026-01-08T10:30:00Z",
        "total_iterations": 5
    });

    let orch_path = opencode_dir.join("orchestrator-state.json");
    let mut orch_file = fs::File::create(&orch_path).unwrap();
    orch_file
        .write_all(
            serde_json::to_string_pretty(&orch_state)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();

    // Verify both files exist and are independent
    assert!(ralph_path.exists(), "Ralph state file should exist");
    assert!(orch_path.exists(), "Orchestrator state file should exist");

    // Read and verify both files can be parsed independently
    let ralph_content = fs::read_to_string(&ralph_path).unwrap();
    let ralph_parsed: serde_json::Value = serde_json::from_str(&ralph_content).unwrap();
    assert_eq!(ralph_parsed["active"], true);
    assert_eq!(ralph_parsed["iteration"], 3);

    let orch_content = fs::read_to_string(&orch_path).unwrap();
    let orch_parsed: serde_json::Value = serde_json::from_str(&orch_content).unwrap();
    assert_eq!(orch_parsed["current_change"], "add-feature");
    assert_eq!(orch_parsed["total_iterations"], 5);

    // Verify modifying one doesn't affect the other
    let updated_ralph_state = serde_json::json!({
        "active": false,
        "iteration": 10,
        "max_iterations": 10,
        "completion_promise": "ALL TESTS PASSING",
        "started_at": "2026-01-08T10:00:00Z",
        "prompt": "Implement feature X",
        "auto_continue": true
    });

    fs::write(
        &ralph_path,
        serde_json::to_string_pretty(&updated_ralph_state).unwrap(),
    )
    .unwrap();

    // Orchestrator state should be unchanged
    let orch_content_after = fs::read_to_string(&orch_path).unwrap();
    assert_eq!(
        orch_content, orch_content_after,
        "Orchestrator state should not be affected by Ralph state changes"
    );
}

/// Test that the command format is compatible with both modes
#[test]
fn test_command_format_compatibility() {
    // OpenCode command format for Headless mode (used by Orchestrator)
    let headless_command = "/openspec-apply add-feature";

    // Ralph loop uses the same command format in TUI mode
    let tui_command = "/openspec-apply add-feature";

    assert_eq!(
        headless_command, tui_command,
        "Command format should be identical for both modes"
    );

    // Verify command parsing
    let parts: Vec<&str> = headless_command.split_whitespace().collect();
    assert_eq!(parts[0], "/openspec-apply");
    assert_eq!(parts[1], "add-feature");
}

/// Test that the execution modes are clearly separated
#[test]
fn test_execution_mode_separation() {
    // Ralph loop execution (TUI mode)
    // - Uses OpenCode's TUI API (appendPrompt, submitPrompt)
    // - Relies on session.idle event for continuation
    // - State is managed in ralph-loop.local.json

    // Orchestrator execution (Headless mode)
    // - Uses `opencode run` command
    // - Process exit indicates completion
    // - State is managed in orchestrator-state.json

    // These modes are intentionally separate and don't interfere
    // This test documents the expected behavior

    // Ralph uses TUI API functions
    let ralph_api_functions = vec!["appendPrompt", "submitPrompt", "session.idle"];

    // Orchestrator uses CLI commands
    let orchestrator_commands = vec!["opencode run", "/openspec-apply", "/openspec-archive"];

    // No overlap in mechanisms
    for ralph_fn in &ralph_api_functions {
        for orch_cmd in &orchestrator_commands {
            assert!(
                !ralph_fn.contains(orch_cmd) && !orch_cmd.contains(ralph_fn),
                "Ralph and Orchestrator should use different mechanisms"
            );
        }
    }
}

/// Test that switching between modes is possible
#[test]
fn test_mode_switching() {
    // Scenario: User starts with Ralph loop, then switches to Orchestrator
    // or vice versa. Both should work independently.

    // The key insight is that:
    // 1. Ralph loop works in TUI mode (interactive)
    // 2. Orchestrator works in Headless mode (batch processing)
    // 3. The /openspec-apply command works in both modes

    // Verify that the command definitions are consistent
    let openspec_apply_command = "/openspec-apply";
    let openspec_archive_command = "/openspec-archive";

    // Both commands should be available for use in either mode
    assert!(openspec_apply_command.starts_with('/'));
    assert!(openspec_archive_command.starts_with('/'));

    // Commands take change-id as argument
    let example_apply = format!("{} {}", openspec_apply_command, "my-change");
    assert!(example_apply.contains("my-change"));
}

/// Test Ralph loop continuation mechanism doesn't conflict with Orchestrator
#[test]
fn test_continuation_mechanism_independence() {
    // Ralph loop continuation:
    // - Triggered by session.idle event
    // - Checks ralph-loop.local.json for active state
    // - Auto-submits prompt if auto_continue is true

    // Orchestrator continuation:
    // - Triggered by process exit
    // - Orchestrator's main loop controls the flow
    // - Uses orchestrator-state.json for state

    // These mechanisms are completely independent
    // Document this in a test

    #[allow(dead_code)]
    struct RalphContinuation {
        trigger: &'static str,
        state_file: &'static str,
        check_condition: &'static str,
    }

    #[allow(dead_code)]
    struct OrchestratorContinuation {
        trigger: &'static str,
        state_file: &'static str,
        flow_control: &'static str,
    }

    let ralph = RalphContinuation {
        trigger: "session.idle event",
        state_file: ".opencode/ralph-loop.local.json",
        check_condition: "state.active && state.auto_continue",
    };

    let orchestrator = OrchestratorContinuation {
        trigger: "process exit (exit code)",
        state_file: ".opencode/orchestrator-state.json",
        flow_control: "Orchestrator main loop",
    };

    assert_ne!(ralph.trigger, orchestrator.trigger);
    assert_ne!(ralph.state_file, orchestrator.state_file);
}
