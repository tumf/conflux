//! Integration coverage for the read-only `cflx logs` command path.

use std::fs;
use std::process::Command;

fn cflx_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cflx"))
}

#[test]
fn logs_path_does_not_create_or_append_log_file() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    let workspace = tmp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();

    let output = cflx_command()
        .arg("logs")
        .arg("--path")
        .current_dir(&workspace)
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("failed to run cflx logs --path");

    assert!(
        output.status.success(),
        "expected success, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let selected_path = String::from_utf8(output.stdout).unwrap();
    let selected_path = selected_path.trim();
    assert!(selected_path.contains("cflx/logs/workspace-"));
    assert!(selected_path.ends_with(".log"));
    assert!(
        !std::path::Path::new(selected_path).exists(),
        "--path must not create the expected log file"
    );
    assert!(
        !state_home.join("cflx/logs").exists(),
        "--path must not create the log directory"
    );
}

#[test]
fn logs_last_reads_existing_project_without_appending() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    let project_dir = state_home.join("cflx/logs/explicit-project");
    fs::create_dir_all(&project_dir).unwrap();
    let log_file = project_dir.join("2026-01-01.log");
    fs::write(&log_file, "one\ntwo\nthree\n").unwrap();
    let before = fs::metadata(&log_file).unwrap().len();

    let output = cflx_command()
        .arg("logs")
        .arg("--project")
        .arg("explicit-project")
        .arg("--last")
        .arg("2")
        .current_dir(tmp.path())
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("failed to run cflx logs --last");

    assert!(
        output.status.success(),
        "expected success, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "two\nthree\n");
    assert_eq!(fs::metadata(&log_file).unwrap().len(), before);
    assert_eq!(fs::read_to_string(&log_file).unwrap(), "one\ntwo\nthree\n");
}

#[test]
fn logs_missing_project_lists_available_slugs() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    fs::create_dir_all(state_home.join("cflx/logs/alpha")).unwrap();
    fs::create_dir_all(state_home.join("cflx/logs/beta")).unwrap();

    let output = cflx_command()
        .arg("logs")
        .arg("--project")
        .arg("missing")
        .current_dir(tmp.path())
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("failed to run cflx logs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"));
    assert!(stderr.contains("alpha"));
    assert!(stderr.contains("beta"));
    assert!(stderr.contains("--project <slug>"));
}
