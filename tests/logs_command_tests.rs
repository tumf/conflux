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

/// The viewer must resolve the same root the writers use, so a configured
/// `state_base_dir` wins over `XDG_STATE_HOME` for `cflx logs` too.
#[test]
fn logs_reads_the_configured_state_root_instead_of_xdg_state_home() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("xdg-state");
    let configured = tmp.path().join("external-state");
    let workspace = tmp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join(".cflx.jsonc"),
        format!(
            "{{\n  \"state_base_dir\": {:?}\n}}\n",
            configured.to_string_lossy()
        ),
    )
    .unwrap();

    // A decoy under the overridden root: selecting the wrong one is visible.
    fs::create_dir_all(state_home.join("cflx/logs/xdg-only")).unwrap();
    fs::write(
        state_home.join("cflx/logs/xdg-only/2026-01-01.log"),
        "wrong-root\n",
    )
    .unwrap();

    let project_dir = configured.join("cflx/logs/configured-project");
    fs::create_dir_all(&project_dir).unwrap();
    let log_file = project_dir.join("2026-01-01.log");
    fs::write(&log_file, "alpha\nbeta\n").unwrap();
    let before = fs::metadata(&log_file).unwrap().len();

    let output = cflx_command()
        .args(["logs", "--project", "configured-project", "--last", "1"])
        .current_dir(&workspace)
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("failed to run cflx logs");

    assert!(
        output.status.success(),
        "expected success, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "beta\n");

    // Reading stays read-only, and the overridden root is never touched.
    assert_eq!(fs::metadata(&log_file).unwrap().len(), before);
    assert_eq!(fs::read_to_string(&log_file).unwrap(), "alpha\nbeta\n");
    assert_eq!(
        fs::read_to_string(state_home.join("cflx/logs/xdg-only/2026-01-01.log")).unwrap(),
        "wrong-root\n"
    );
}

/// Discovery is scoped to the currently resolved root: projects that only exist
/// under the overridden root are not offered, because nothing writes there now.
#[test]
fn logs_lists_projects_from_the_configured_root_only() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("xdg-state");
    let configured = tmp.path().join("external-state");
    let workspace = tmp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join(".cflx.jsonc"),
        format!(
            "{{\n  \"state_base_dir\": {:?}\n}}\n",
            configured.to_string_lossy()
        ),
    )
    .unwrap();
    fs::create_dir_all(state_home.join("cflx/logs/xdg-only")).unwrap();
    fs::create_dir_all(configured.join("cflx/logs/configured-project")).unwrap();

    let output = cflx_command()
        .args(["logs", "--project", "missing"])
        .current_dir(&workspace)
        .env("XDG_STATE_HOME", &state_home)
        .output()
        .expect("failed to run cflx logs");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("configured-project"),
        "the configured root's projects must be listed, got stderr={stderr}"
    );
    assert!(
        !stderr.contains("xdg-only"),
        "the overridden root must not be listed, got stderr={stderr}"
    );
    assert!(
        stderr.contains(&configured.join("cflx/logs").to_string_lossy().to_string()),
        "the reported log root must be the configured one, got stderr={stderr}"
    );
}

/// A root the writers would refuse is not silently readable from somewhere else.
#[test]
fn logs_rejects_a_relative_configured_state_root() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("xdg-state");
    let workspace = tmp.path().join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join(".cflx.jsonc"),
        "{\n  \"state_base_dir\": \"relative/state\"\n}\n",
    )
    .unwrap();
    fs::create_dir_all(state_home.join("cflx/logs/xdg-only")).unwrap();

    let output = cflx_command()
        .args(["logs", "--path"])
        .current_dir(&workspace)
        .output()
        .expect("failed to run cflx logs --path");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !output.status.success(),
        "a relative configured root must be refused, stderr={stderr}"
    );
    assert!(
        stderr.contains("state_base_dir") && stderr.contains("absolute"),
        "the diagnostic must name the setting and the rule, got stderr={stderr}"
    );
    assert!(
        !workspace.join("relative").exists(),
        "a refusal must not create the configured directory"
    );
}
