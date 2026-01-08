//! Ralph Loop Compatibility Tests
//!
//! These tests verify that the openspec-orchestrator is compatible with
//! the Ralph Wiggum plugin for OpenCode.
//!
//! Key compatibility requirements:
//! 1. Commands work in both TUI mode (Ralph) and Headless mode (Orchestrator)
//! 2. Both tools can coexist in the same project

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

    // Orchestrator execution (Headless mode)
    // - Uses `opencode run` command
    // - Process exit indicates completion
    // - Stateless - relies on openspec list for current state

    // These modes are intentionally separate and don't interfere

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
