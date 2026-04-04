//! Regression guard: no backup-extension files may be git-tracked.
//!
//! Detects `.bak`, `.backup`, and `.bak2` files so that accidental
//! re-introduction is caught by `cargo test` in CI.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn no_tracked_backup_files() {
    let output = Command::new("git")
        .args(["ls-files", "*.bak", "*.backup", "*.bak2"])
        .output()
        .expect("failed to run git ls-files");

    let found = String::from_utf8_lossy(&output.stdout);
    assert!(
        found.trim().is_empty(),
        "Tracked backup files detected — remove them with `git rm`:\n{}",
        found
    );
}

#[test]
fn heavy_real_boundary_suites_stay_feature_gated() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let file_gated = [
        "tests/e2e_tests.rs",
        "tests/e2e_proposal_session.rs",
        "tests/e2e_git_worktree_tests.rs",
        "tests/process_cleanup_test.rs",
        "tests/merge_conflict_check_tests.rs",
    ];

    for relative_path in file_gated {
        let content = fs::read_to_string(repo_root.join(relative_path))
            .unwrap_or_else(|e| panic!("failed to read {relative_path}: {e}"));
        assert!(
            content.contains("#![cfg(feature = \"heavy-tests\")]"),
            "{relative_path} must remain file-gated with #![cfg(feature = \"heavy-tests\")]"
        );
    }

    let function_gated: [(&str, &[&str]); 6] = [
        (
            "src/parallel/tests/executor.rs",
            &[
                "test_attempt_merge_succeeds_when_change_archived",
                "test_merge_proceeds_when_archive_complete",
            ],
        ),
        (
            "src/server/api/projects.rs",
            &[
                "test_add_project_setup_failure_returns_422_and_rolls_back_registry",
                "test_add_project_without_repo_root_setup_succeeds_without_marker",
                "test_app_state_resolve_command_comes_from_top_level_config",
            ],
        ),
        (
            "src/ai_command_runner.rs",
            &[
                "test_inactivity_timeout_streaming_pipeline",
                "test_inactivity_timeout_retry",
            ],
        ),
        (
            "src/orchestration/archive.rs",
            &["test_archive_change_retries_until_verified"],
        ),
        (
            "src/command_queue.rs",
            &[
                "test_inactivity_timeout_triggers",
                "test_inactivity_timeout_error_message_format",
            ],
        ),
        (
            "src/hooks.rs",
            &["test_hook_runner_timeout", "test_index_lock_wait_timeout"],
        ),
    ];

    for (relative_path, names) in function_gated {
        let content = fs::read_to_string(repo_root.join(relative_path))
            .unwrap_or_else(|e| panic!("failed to read {relative_path}: {e}"));
        let lines: Vec<&str> = content.lines().collect();
        for name in names {
            let fn_line = lines
                .iter()
                .position(|l| l.contains(&format!("fn {name}(")))
                .unwrap_or_else(|| panic!("{relative_path}: function {name} not found"));
            let window_start = fn_line.saturating_sub(5);
            let has_gate = lines[window_start..fn_line]
                .iter()
                .any(|l| l.contains("#[cfg(feature = \"heavy-tests\")]"));
            assert!(
                has_gate,
                "{relative_path} must gate {name} with #[cfg(feature = \"heavy-tests\")] within 5 lines above"
            );
        }
    }
}
