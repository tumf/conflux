use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn run_init(project: &TempDir, config_home: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cflx"))
        .arg("init")
        .current_dir(project.path())
        .env("XDG_CONFIG_HOME", config_home.path())
        .env("HOME", project.path().join("home"))
        .output()
        .expect("cflx init must run")
}

#[test]
fn init_writes_only_required_commands_missing_from_global_config() {
    let project = TempDir::new().unwrap();
    let config_home = TempDir::new().unwrap();
    let global_dir = config_home.path().join("cflx");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("config.jsonc"),
        r#"{
  "analyze_command": "global analyze",
  "apply_command": "global apply",
  "archive_command": "global archive",
  "acceptance_command": "global acceptance"
}"#,
    )
    .unwrap();

    let output = run_init(&project, &config_home);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = fs::read_to_string(project.path().join(".cflx.jsonc")).unwrap();
    let parsed = conflux::config::OrchestratorConfig::parse_jsonc(&generated).unwrap();
    assert!(parsed.resolve_command.is_some());
    assert!(parsed.analyze_command.is_none());
    assert!(parsed.apply_command.is_none());
    assert!(parsed.archive_command.is_none());
    assert!(parsed.acceptance_command.is_none());
}

#[test]
fn init_does_not_create_project_config_when_global_config_is_complete() {
    let project = TempDir::new().unwrap();
    let config_home = TempDir::new().unwrap();
    let global_dir = config_home.path().join("cflx");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("config.jsonc"),
        r#"{
  "analyze_command": "global analyze",
  "apply_command": "global apply",
  "archive_command": "global archive",
  "acceptance_command": "global acceptance",
  "resolve_command": "global resolve"
}"#,
    )
    .unwrap();

    let output = run_init(&project, &config_home);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!project.path().join(".cflx.jsonc").exists());
}
