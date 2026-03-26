//! Regression guard: no backup-extension files may be git-tracked.
//!
//! Detects `.bak`, `.backup`, and `.bak2` files so that accidental
//! re-introduction is caught by `cargo test` in CI.

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
