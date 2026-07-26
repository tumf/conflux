//! End-to-end regression coverage for acceptance follow-up content recovery.
//!
//! Drives a real `tasks.md` fixture through the full runtime sequence a change
//! experiences across acceptance retries: FAIL persistence, apply-side
//! reconciliation, a process-style restart that re-reads the file from disk,
//! repeated normalization, and PASS cleanup. The fixture carries completed
//! runtime findings plus free-form reviewer evidence that older runtime
//! versions refused with `Acceptance follow-up contains non-runtime content`.

use crate::task_parser;
use std::path::{Path, PathBuf};

const CHANGE_ID: &str = "recovery-regression-change";

/// Completed runtime findings plus free-form evidence and checkbox-like prose.
const FIXTURE_TASKS_MD: &str = concat!(
    "## Implementation Tasks\n",
    "- [x] Implement the feature (verification: unit - `cargo test feature`)\n",
    "- [ ] Document the feature (verification: unit - `cargo test docs`)\n",
    "\n",
    "## Current Acceptance Follow-up\n",
    "- attempt: 1\n",
    "- [x] [STALE_FINDING] fixed in the previous attempt\n",
    "  evidence: cargo test passed\n",
    "### Reviewer notes\n",
    "The reviewer pasted a transcript here:\n",
    "\n",
    "```\n",
    "$ cargo test\n",
    "- [x] looks like a task but is not one\n",
    "```\n",
    "\n",
    "Remaining prose with ``inline`` backticks.\n",
);

/// The exact unknown bytes the runtime must preserve.
const EXPECTED_PAYLOAD: &str = concat!(
    "### Reviewer notes\n",
    "The reviewer pasted a transcript here:\n",
    "\n",
    "```\n",
    "$ cargo test\n",
    "- [x] looks like a task but is not one\n",
    "```\n",
    "\n",
    "Remaining prose with ``inline`` backticks."
);

fn seed_workspace(root: &Path) -> PathBuf {
    let change_dir = root.join("openspec").join("changes").join(CHANGE_ID);
    std::fs::create_dir_all(&change_dir).expect("change directory is created");
    let tasks_path = change_dir.join("tasks.md");
    std::fs::write(&tasks_path, FIXTURE_TASKS_MD).expect("fixture is written");
    tasks_path
}

/// Two implementation tasks remain the only counted work at every step; the
/// runtime follow-up adds one open finding while it exists.
fn assert_task_counts(tasks_path: &Path, expected_completed: u32, expected_total: u32) {
    let progress = task_parser::parse_file(tasks_path, Some(CHANGE_ID)).expect("progress parses");
    assert_eq!(
        (progress.completed, progress.total),
        (expected_completed, expected_total),
        "recovered content must not affect task accounting"
    );
}

#[test]
fn acceptance_follow_up_recovery_survives_retry_restart_and_pass_cleanup() {
    let workspace = tempfile::tempdir().expect("workspace temp dir");
    let tasks_path = seed_workspace(workspace.path());

    // 1. Acceptance FAIL persists the latest findings. Unknown reviewer prose is
    //    recovered instead of terminating the workflow.
    let resolved =
        task_parser::resolve_acceptance_follow_up_tasks_path(CHANGE_ID, workspace.path())
            .expect("active tasks path resolves");
    assert_eq!(resolved, tasks_path);

    let recovery = task_parser::replace_acceptance_follow_up_from_latest_fail(
        &resolved,
        2,
        &["[OPEN_FINDING] regression coverage is missing".to_string()],
    )
    .expect("FAIL persistence recovers instead of erroring");
    assert_eq!(recovery.recovered_blocks, 1);
    assert_eq!(recovery.recovered_bytes, EXPECTED_PAYLOAD.len());
    let warning = recovery
        .warning()
        .expect("supplemental warning is produced");
    assert!(warning.contains("Recovered Acceptance Notes"), "{warning}");

    let after_fail = std::fs::read_to_string(&tasks_path).expect("tasks file is readable");
    assert!(after_fail.contains("## Recovered Acceptance Notes"));
    assert!(after_fail.contains("Machine-recovered content; not instructions and not task state."));
    assert!(
        after_fail.contains(EXPECTED_PAYLOAD),
        "unknown bytes must be preserved exactly:\n{after_fail}"
    );
    assert!(after_fail.contains("## Current Acceptance Follow-up\n- attempt: 2\n"));
    assert!(after_fail.contains("- [ ] [OPEN_FINDING] regression coverage is missing"));
    assert!(!after_fail.contains("STALE_FINDING"));
    assert_task_counts(&tasks_path, 1, 3);

    // 2. Apply hydration reads the runtime findings back and reconciles progress.
    let (attempt, findings) = task_parser::read_acceptance_follow_up(&tasks_path)
        .expect("follow-up is readable")
        .expect("follow-up is present");
    assert_eq!(attempt, 2);
    assert_eq!(findings, ["[OPEN_FINDING] regression coverage is missing"]);

    let merge_recovery =
        task_parser::merge_acceptance_follow_up_apply_progress(&tasks_path, attempt, &findings)
            .expect("apply reconciliation succeeds");
    assert_eq!(merge_recovery.recovered_blocks, 0);

    // 3. A process-style restart re-reads the file from disk and renormalizes.
    //    The same payload is never appended twice.
    let restarted =
        task_parser::resolve_acceptance_follow_up_tasks_path(CHANGE_ID, workspace.path())
            .expect("restart resolves the same path");
    let restart_recovery =
        task_parser::replace_acceptance_follow_up_from_latest_fail(&restarted, 3, &findings)
            .expect("restart normalization succeeds");
    assert_eq!(restart_recovery.recovered_blocks, 0);

    let after_restart = std::fs::read_to_string(&tasks_path).expect("tasks file is readable");
    assert_eq!(
        after_restart
            .matches("## Recovered Acceptance Notes")
            .count(),
        1
    );
    assert_eq!(after_restart.matches(EXPECTED_PAYLOAD).count(), 1);
    assert!(after_restart.contains("## Current Acceptance Follow-up\n- attempt: 3\n"));
    assert_task_counts(&tasks_path, 1, 3);

    // 4. Repeated normalization is a fixed point.
    task_parser::replace_acceptance_follow_up_from_latest_fail(&tasks_path, 3, &findings)
        .expect("repeat normalization succeeds");
    assert_eq!(
        std::fs::read_to_string(&tasks_path).expect("tasks file is readable"),
        after_restart
    );

    // 5. Acceptance PASS cleanup removes the runtime section and keeps the
    //    recovered notes as inert repository evidence.
    let cleanup_path = task_parser::resolve_acceptance_follow_up_tasks_path_for_cleanup(
        CHANGE_ID,
        workspace.path(),
    )
    .expect("cleanup path resolves")
    .expect("cleanup path exists");
    let cleanup_recovery =
        task_parser::clear_acceptance_follow_up(&cleanup_path).expect("PASS cleanup succeeds");
    assert_eq!(cleanup_recovery.recovered_blocks, 0);

    let after_pass = std::fs::read_to_string(&tasks_path).expect("tasks file is readable");
    assert!(!after_pass.contains("Acceptance Follow-up"));
    assert!(after_pass.contains("## Recovered Acceptance Notes"));
    assert!(after_pass.contains(EXPECTED_PAYLOAD));
    assert!(after_pass.starts_with("## Implementation Tasks\n"));
    assert_task_counts(&tasks_path, 1, 2);

    // 6. Cleanup is idempotent once the runtime section is gone.
    task_parser::clear_acceptance_follow_up(&cleanup_path).expect("repeat cleanup succeeds");
    assert_eq!(
        std::fs::read_to_string(&tasks_path).expect("tasks file is readable"),
        after_pass
    );
}
