use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

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

fn run_with_timeout(mut command: Command, timeout: Duration) -> std::process::Output {
    let start = std::time::Instant::now();
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    loop {
        if let Some(_status) = child.try_wait().unwrap() {
            return child.wait_with_output().unwrap();
        }

        if start.elapsed() > timeout {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!(
                "command timed out after {:?}: status={:?}\nstdout={}\nstderr={}",
                timeout,
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn unsupported_completion_shell_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let output = cflx_command(tmp.path(), &state_home)
        .args(["completion", "elvish"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
    assert!(!state_home.join("cflx/logs").exists());
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
fn generated_bash_script_wires_dynamic_change_id_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let stdout = stdout_string(
        cflx_command(tmp.path(), &state_home)
            .args(["completion", "bash"])
            .output()
            .unwrap(),
    );

    assert!(stdout.contains("complete -F _cflx_dynamic_completion"));
    assert!(stdout.contains("if [[ \"$prev\" == \"--change\" ]]"));
    assert!(stdout.contains("_cflx_dynamic_change_ids all \"$cur\""));
    assert!(stdout.contains("_cflx_dynamic_change_ids active \"$cur\""));
    assert!(stdout.contains("_cflx_static_completion \"$@\""));
    assert!(stdout.contains("_cflx \"$@\""));
    assert!(!stdout.contains("complete -p cflx"));
}

#[test]
fn generated_bash_dynamic_completion_falls_back_without_recursing() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");
    let script_path = tmp.path().join("cflx-completion.bash");
    let stdout = stdout_string(
        cflx_command(tmp.path(), &state_home)
            .args(["completion", "bash"])
            .output()
            .unwrap(),
    );
    fs::write(&script_path, stdout).unwrap();

    let mut command = Command::new("bash");
    command.arg("-c").arg(format!(
        "source {}; COMP_WORDS=(cflx r); COMP_CWORD=1; _cflx_dynamic_completion >/tmp/cflx-fallback.out; status=$?; printf 'rc=%s count=%s\\n' \"$status\" \"${{#COMPREPLY[@]}}\"",
        script_path.display()
    ));

    let output = run_with_timeout(command, Duration::from_secs(2));
    assert!(
        output.status.success(),
        "fallback completion failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("rc=0"),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn generated_zsh_script_wires_dynamic_change_id_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let stdout = stdout_string(
        cflx_command(tmp.path(), &state_home)
            .args(["completion", "zsh"])
            .output()
            .unwrap(),
    );

    assert!(stdout.contains("compdef _cflx_dynamic_completion cflx"));
    assert!(stdout.contains("if [[ \"${words[CURRENT-1]}\" == \"--change\" ]]"));
    assert!(stdout.contains("_cflx_dynamic_change_ids all \"${words[CURRENT]}\""));
    assert!(stdout.contains("_cflx_dynamic_change_ids active \"${words[CURRENT]}\""));
    assert!(stdout.contains("_cflx_static_completion \"$@\""));
}

#[test]
fn generated_powershell_script_registers_dynamic_change_id_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let state_home = tmp.path().join("state");

    let stdout = stdout_string(
        cflx_command(tmp.path(), &state_home)
            .args(["completion", "powershell"])
            .output()
            .unwrap(),
    );

    assert!(stdout.contains("function __CflxDynamicRunChangeIds($WordToComplete)"));
    assert!(stdout.contains("$commandText -match '^cflx\\s+run\\b'"));
    assert!(stdout.contains("$commandText -match '^cflx\\s+openspec\\s+show\\b'"));
    assert!(stdout.contains("$commandText -match '^cflx\\s+openspec\\s+(validate|archive)\\b'"));
    assert!(
        stdout.contains("[CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue")
    );
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
