//! Regression tests for `cflx run --all` exit-on-success behavior.
//!
//! Verifies that `cflx run --all` exits promptly with status 0 after successful
//! orchestration rather than waiting for an external stop signal.

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

/// Minimal `.cflx.jsonc` for testing — no AI commands needed when there are no changes.
const MINIMAL_CONFIG: &str = r#"{
  "apply_command": "echo apply",
  "archive_command": "echo archive",
  "analyze_command": "echo analyze",
  "acceptance_command": ""
}
"#;

/// Set up a temporary project root with:
/// - `.cflx.jsonc`  (minimal config)
/// - `openspec/`    (empty — no pending changes)
fn setup_empty_project(dir: &std::path::Path) {
    fs::write(dir.join(".cflx.jsonc"), MINIMAL_CONFIG).unwrap();
    fs::create_dir_all(dir.join("openspec/changes")).unwrap();
}

fn add_change(dir: &std::path::Path, id: &str) {
    let change_dir = dir.join("openspec/changes").join(id);
    fs::create_dir_all(&change_dir).unwrap();
    fs::write(change_dir.join("proposal.md"), format!("# {id}\n")).unwrap();
    fs::write(change_dir.join("tasks.md"), "- [ ] task\n").unwrap();
}

/// Run `cflx run` from `cwd` and assert it exits within `timeout`.
/// Returns the `ExitStatus`.
fn run_cflx_with_timeout(
    cwd: &std::path::Path,
    extra_args: &[&str],
    timeout: Duration,
) -> std::process::ExitStatus {
    let bin = env!("CARGO_BIN_EXE_cflx");

    let mut child = std::process::Command::new(bin)
        .arg("run")
        .args(extra_args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn cflx");

    let start = Instant::now();

    loop {
        match child.try_wait().expect("failed to check child status") {
            Some(status) => return status,
            None => {
                if start.elapsed() >= timeout {
                    child.kill().ok();
                    panic!(
                        "cflx run did not exit within {:?} — likely hung in post-success wait loop",
                        timeout
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// ── Task 3: non-web success path ───────────────────────────────────────────

#[test]
fn test_run_exits_promptly_on_success_no_web() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    // With no pending changes the orchestrator returns Ok(()) immediately.
    // The process must exit within 10 seconds without any external signal.
    let status = run_cflx_with_timeout(tmp.path(), &["--all"], Duration::from_secs(10));
    assert!(
        status.success(),
        "cflx run --all should exit with status 0 on success, got: {:?}",
        status
    );
}

#[test]
fn test_run_rejects_bare_target_before_execution() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    let bare = run_cflx_with_timeout(tmp.path(), &[], Duration::from_secs(10));
    assert!(!bare.success());
}

#[test]
fn test_parallel_dry_run_uses_explicit_targets() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    add_change(tmp.path(), "a");
    add_change(tmp.path(), "c");
    Command::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["run", "a", "--parallel", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total changes: 1"));
    assert!(stdout.contains("  - a"));
    assert!(!stdout.contains("  - c"));
}

#[test]
fn test_run_stdout_keeps_info_and_suppresses_debug_trace_noise() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());
    add_change(tmp.path(), "a");
    Command::new("git")
        .arg("init")
        .current_dir(tmp.path())
        .output()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["run", "a", "--parallel", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Starting cflx "), "stdout={stdout}");
    assert!(stdout.contains("Total changes: 1"), "stdout={stdout}");
    assert!(!stdout.contains("DEBUG Executing git command"), "stdout={stdout}");
    assert!(!stdout.contains("TRACE registering event source with poller"), "stdout={stdout}");
    assert!(!stdout.contains("TRACE deregistering event source from poller"), "stdout={stdout}");
}

// ── Task 4: --web success path ─────────────────────────────────────────────

#[test]
#[cfg(feature = "web-monitoring")]
fn test_run_exits_promptly_on_success_with_web() {
    let tmp = tempfile::tempdir().unwrap();
    setup_empty_project(tmp.path());

    // Use port 0 so the OS auto-assigns an available port (no conflicts).
    let status = run_cflx_with_timeout(
        tmp.path(),
        &["--all", "--web", "--web-port", "0"],
        Duration::from_secs(15),
    );
    assert!(
        status.success(),
        "cflx run --web should exit with status 0 on success, got: {:?}",
        status
    );
}
