//! Tests for OpenCode command file content validation
//!
//! These tests verify that the OpenCode command files contain the expected
//! external dependency policy phrases and instructions.

use std::fs;
use std::path::PathBuf;

/// Get path to OpenCode command directory
fn opencode_command_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".config").join("opencode").join("command"))
}

#[test]
fn test_cflx_proposal_contains_external_dependency_policy() {
    let Some(cmd_dir) = opencode_command_dir() else {
        println!("Skipping test: home directory not available");
        return;
    };

    let proposal_path = cmd_dir.join("cflx-proposal.md");
    if !proposal_path.exists() {
        println!("Skipping test: cflx-proposal.md does not exist");
        return;
    }

    let content = fs::read_to_string(&proposal_path).expect("Failed to read cflx-proposal.md");

    // Verify external dependency policy section exists
    assert!(
        content.contains("External dependency policy"),
        "cflx-proposal.md should contain 'External dependency policy' section"
    );

    // Verify mock-first approach
    assert!(
        content.contains("mock-first"),
        "cflx-proposal.md should contain 'mock-first'"
    );

    // Verify definition of external dependency
    assert!(
        content.contains("AI cannot resolve or verify autonomously"),
        "cflx-proposal.md should define external dependencies"
    );

    // Verify mock/stub/fixture prioritization
    assert!(
        content.contains("mock/stub/fixture"),
        "cflx-proposal.md should mention mock/stub/fixture implementations"
    );

    // Verify non-mockable handling
    assert!(
        content.contains("non-mockable"),
        "cflx-proposal.md should mention non-mockable dependencies"
    );
}

#[test]
fn test_cflx_apply_contains_external_dependency_policy() {
    let Some(cmd_dir) = opencode_command_dir() else {
        println!("Skipping test: home directory not available");
        return;
    };

    let apply_path = cmd_dir.join("cflx-apply.md");
    if !apply_path.exists() {
        println!("Skipping test: cflx-apply.md does not exist");
        return;
    }

    let content = fs::read_to_string(&apply_path).expect("Failed to read cflx-apply.md");

    // Verify external dependency policy section exists
    assert!(
        content.contains("External dependency policy"),
        "cflx-apply.md should contain 'External dependency policy' section"
    );

    // Verify mock-first approach
    assert!(
        content.contains("mock-first"),
        "cflx-apply.md should contain 'mock-first'"
    );

    // Verify missing secrets MUST NOT cause CONTINUE
    assert!(
        content.contains("Missing secrets") && content.contains("MUST NOT"),
        "cflx-apply.md should specify that missing secrets MUST NOT be used as a reason to CONTINUE"
    );

    // Verify mockable dependencies must be mocked
    assert!(
        content.contains("CAN be mocked") && content.contains("MUST be mocked"),
        "cflx-apply.md should specify that mockable dependencies MUST be mocked"
    );

    // Verify non-mockable handling
    assert!(
        content.contains("non-mockable") && content.contains("Out of Scope"),
        "cflx-apply.md should specify handling of non-mockable dependencies"
    );
}
