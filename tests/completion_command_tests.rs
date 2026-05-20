use std::fs;
use std::path::Path;
use std::process::Command;

fn cflx_command(workdir: &Path, state_home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cflx"));
    cmd.current_dir(workdir)
        .env("XDG_STATE_HOME", state_home)
        .env("HOME", workdir.join("home"));
    cmd
}

fn write_proposal(root: &Path, relative_dir: &str) {
    let dir = root.join(relative_dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("proposal.md"), "# Proposal\n").unwrap();
}

fn stdout_string(output: std::process::Output) -> String {
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn completion_generation_is_non_empty_and_does_not_create_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    for (shell, marker) in [
        ("zsh", "#compdef cflx"),
        ("bash", "complete"),
        ("fish", "complete -c cflx"),
        ("powershell", "Register-ArgumentCompleter"),
    ] {
        let output = cflx_command(tmp.path(), &state_home)
            .args(["completion", shell])
            .output()
            .unwrap();
        let stdout = stdout_string(output);
        assert!(!stdout.trim().is_empty(), "{shell} completion was empty");
        assert!(stdout.contains(marker), "{shell} missing marker {marker}");
        assert!(stdout.contains("cflx __complete change-ids"));
    }

    assert!(!state_home.join("cflx/logs").exists());
}

#[test]
fn hidden_candidate_command_lists_active_default_and_filters_prefix_without_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    write_proposal(tmp.path(), "openspec/changes/add-one");
    write_proposal(tmp.path(), "openspec/changes/add-two");
    write_proposal(tmp.path(), "openspec/changes/fix-one");
    write_proposal(tmp.path(), "openspec/changes/archive/add-archived");

    let output = cflx_command(tmp.path(), &state_home)
        .args(["__complete", "change-ids", "--prefix", "add-"])
        .output()
        .unwrap();
    let stdout = stdout_string(output);

    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["add-one", "add-two"]
    );
    assert!(!state_home.join("cflx/logs").exists());
}

#[test]
fn hidden_candidate_command_includes_archived_on_request_and_normalizes_dates() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    write_proposal(tmp.path(), "openspec/changes/active-one");
    write_proposal(tmp.path(), "openspec/changes/archive/direct-archive");
    write_proposal(
        tmp.path(),
        "openspec/changes/archive/2026-05-20-dated-archive",
    );

    let output = cflx_command(tmp.path(), &state_home)
        .args(["__complete", "change-ids", "--active", "--archived"])
        .output()
        .unwrap();
    let stdout = stdout_string(output);

    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["active-one", "dated-archive", "direct-archive"]
    );
}

#[test]
fn hidden_candidate_command_missing_workspace_is_empty_success() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let output = cflx_command(tmp.path(), &state_home)
        .args(["__complete", "change-ids"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!state_home.join("cflx/logs").exists());
}

#[test]
fn generated_scripts_reference_required_dynamic_surfaces() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let stdout = stdout_string(
        cflx_command(tmp.path(), &state_home)
            .args(["completion", "bash"])
            .output()
            .unwrap(),
    );

    assert!(stdout.contains("run --change"));
    assert!(stdout.contains("openspec show"));
    assert!(stdout.contains("openspec validate/archive"));
    assert!(stdout.contains("--active --archived"));
    assert!(stdout.contains("_cflx_dynamic_run_change_ids"));
}
