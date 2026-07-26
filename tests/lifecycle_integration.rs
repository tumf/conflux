//! Integration tests for the external lifecycle integration contract.
//!
//! These tests run **real executable adapter fixtures** as child processes and
//! assert on the newline-delimited JSON they actually receive on stdin. They
//! fail if event delivery is stubbed or disconnected from runtime state changes.
//!
//! Fixture adapters are process-level boundaries, so this is integration-scoped
//! evidence. Pure protocol/serialization/dedup logic is unit-tested in
//! `src/lifecycle_integration.rs`.

#![cfg(unix)]
// `spawn_guard` deliberately holds a blocking `std` mutex across awaits: it
// serializes process-environment mutation and `fork`/`exec` across the whole
// test binary, so an async-aware mutex that yields the thread to another test
// would defeat its purpose. Each test owns its own current-thread runtime.
#![allow(clippy::await_holding_lock)]

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use conflux::config::LifecycleIntegrationConfig;
use conflux::lifecycle_integration::{
    LifecycleContext, LifecycleEvent, LifecycleExecutionMode, LifecycleIntegration, LifecycleState,
};
use serde_json::Value;

// ── Process-level test isolation ───────────────────────────────────────────

/// Secret planted in the test-process environment to prove it never leaks into payloads.
const SECRET_ENV_VALUE: &str = "cflx-lifecycle-secret-value";
/// Herdr pane identifier used to prove environment inheritance.
const PANE_ID: &str = "pane-77";

/// Serializes adapter spawns.
///
/// These tests both mutate the process environment and `fork`/`exec` child
/// processes. Doing those concurrently in one process is unsafe, so every test
/// that starts an adapter takes this guard. The environment is planted exactly
/// once, under the guard, and never removed.
fn spawn_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    static ENV_READY: OnceLock<()> = OnceLock::new();
    ENV_READY.get_or_init(|| {
        std::env::set_var("CFLX_LIFECYCLE_TEST_SECRET", SECRET_ENV_VALUE);
        std::env::set_var("HERDR_ENV", "1");
        std::env::set_var("HERDR_PANE_ID", PANE_ID);
    });

    guard
}

/// Run an async test body on a runtime created *inside* the spawn guard.
///
/// `#[tokio::test]` builds its runtime before the test body runs, so several
/// runtimes would coexist while other tests fork adapter processes. Building the
/// runtime under the guard keeps exactly one process-spawning runtime alive at a
/// time. The future argument is inert until polled, so constructing it before
/// taking the guard is safe.
fn run_guarded<F: std::future::Future<Output = ()>>(body: F) {
    let _guard = spawn_guard();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime must build")
        .block_on(body);
}

// ── Fixture adapters ───────────────────────────────────────────────────────

fn write_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("fixture script must be written");
    let mut perms = fs::metadata(&path).expect("fixture metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("fixture must be executable");
    path
}

/// Adapter that records every received line to `record_path`.
fn recording_adapter(dir: &Path, record_path: &Path) -> PathBuf {
    write_executable(
        dir,
        "recording-adapter.sh",
        &format!("#!/bin/sh\ncat > '{}'\n", record_path.display()),
    )
}

/// Adapter that exits immediately with a failure status without reading stdin.
fn crashing_adapter(dir: &Path) -> PathBuf {
    write_executable(dir, "crashing-adapter.sh", "#!/bin/sh\nexit 3\n")
}

/// Adapter that never reads stdin and never exits on its own.
fn blocked_reader_adapter(dir: &Path) -> PathBuf {
    write_executable(dir, "blocked-adapter.sh", "#!/bin/sh\nsleep 120\n")
}

/// Adapter that records the value of `HERDR_PANE_ID` inherited from cflx.
fn env_probe_adapter(dir: &Path, record_path: &Path) -> PathBuf {
    write_executable(
        dir,
        "env-probe-adapter.sh",
        &format!(
            "#!/bin/sh\nprintf 'pane=%s\\n' \"$HERDR_PANE_ID\" > '{}'\ncat >> '{}'\n",
            record_path.display(),
            record_path.display()
        ),
    )
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Shutdown deadline short enough to keep the bounded-shutdown assertions meaningful.
const TIGHT_SHUTDOWN_MS: u64 = 400;

fn config_for(command: Vec<String>) -> LifecycleIntegrationConfig {
    LifecycleIntegrationConfig {
        command,
        // A small queue and write timeout keep failure paths fast and
        // deterministic. The shutdown deadline stays generous on purpose: tests
        // that assert bounded shutdown assert against their own, tighter bound,
        // so a loaded machine that is slow to `fork`/`exec` a fixture cannot turn
        // a correctness assertion into a timing flake.
        queue_capacity: Some(16),
        write_timeout_ms: Some(150),
        shutdown_timeout_ms: Some(5_000),
        ..Default::default()
    }
}

fn read_messages(path: &Path) -> Vec<Value> {
    let raw = fs::read_to_string(path).unwrap_or_default();
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("each recorded line must be valid JSON"))
        .collect()
}

fn context() -> LifecycleContext {
    LifecycleContext::workspace("/tmp/workspace")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[test]
fn recording_adapter_receives_ordered_versioned_lifecycle_stream() {
    run_guarded(recording_adapter_receives_ordered_versioned_lifecycle_stream_impl());
}

async fn recording_adapter_receives_ordered_versioned_lifecycle_stream_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let record = dir.path().join("records.jsonl");
    let adapter = recording_adapter(dir.path(), &record);

    let integration = LifecycleIntegration::start(
        Some(&config_for(vec![adapter.display().to_string()])),
        LifecycleExecutionMode::Tui,
    );
    assert!(integration.is_enabled(), "adapter must start");

    let handle = integration.handle();
    handle.publish(LifecycleEvent::ProcessStarted { context: context() });
    handle.publish_state(LifecycleState::Idle, context());
    handle.publish_state(LifecycleState::Working, context());
    // Repeated unchanged state must be deduplicated, not re-sent.
    handle.publish_state(LifecycleState::Working, context());
    handle.publish_state(LifecycleState::Blocked, context());
    handle.publish(LifecycleEvent::SessionIdentified {
        session_id: "session-1".to_string(),
    });

    integration.shutdown().await;

    let messages = read_messages(&record);
    let kinds: Vec<&str> = messages
        .iter()
        .map(|m| m["kind"].as_str().expect("kind"))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "process_started",
            "state_changed",
            "state_changed",
            "state_changed",
            "session_identified",
            "process_stopping",
        ],
        "adapter must receive the ordered lifecycle stream with deduplicated states"
    );

    let states: Vec<Option<&str>> = messages.iter().map(|m| m["state"].as_str()).collect();
    assert_eq!(
        states,
        vec![
            None,
            Some("idle"),
            Some("working"),
            Some("blocked"),
            None,
            None
        ]
    );

    for (index, message) in messages.iter().enumerate() {
        assert_eq!(message["protocol_version"], 1, "versioned protocol");
        assert_eq!(
            message["sequence"],
            (index as u64) + 1,
            "sequence numbers must increase monotonically without gaps"
        );
        assert_eq!(message["mode"], "tui", "execution mode must be reported");
        assert!(
            message["pid"].as_u64().is_some_and(|pid| pid > 0),
            "process id must be reported"
        );
    }
}

#[test]
fn adapter_payloads_stay_within_the_privacy_boundary() {
    run_guarded(adapter_payloads_stay_within_the_privacy_boundary_impl());
}

async fn adapter_payloads_stay_within_the_privacy_boundary_impl() {
    // The guard plants a secret in the inherited environment; it must never reach the payload.
    let dir = tempfile::tempdir().expect("tempdir");
    let record = dir.path().join("records.jsonl");
    let adapter = recording_adapter(dir.path(), &record);

    let integration = LifecycleIntegration::start(
        Some(&config_for(vec![adapter.display().to_string()])),
        LifecycleExecutionMode::Run,
    );
    integration.handle().publish_state(
        LifecycleState::Working,
        context().with_change_id(Some("change-a".to_string())),
    );
    integration.shutdown().await;

    let raw = fs::read_to_string(&record).expect("records");
    assert!(
        !raw.contains(SECRET_ENV_VALUE),
        "lifecycle payloads must not contain environment values: {raw}"
    );

    let messages = read_messages(&record);
    let state_changed = messages
        .iter()
        .find(|m| m["kind"] == "state_changed")
        .expect("state change present");
    let mut keys: Vec<&str> = state_changed
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "context",
            "kind",
            "mode",
            "pid",
            "protocol_version",
            "sequence",
            "state"
        ],
        "only explicitly specified fields may be serialized"
    );

    let mut context_keys: Vec<&str> = state_changed["context"]
        .as_object()
        .expect("context object")
        .keys()
        .map(String::as_str)
        .collect();
    context_keys.sort_unstable();
    assert_eq!(context_keys, vec!["change_id", "workspace"]);
}

#[test]
fn adapter_inherits_the_cflx_process_environment() {
    run_guarded(adapter_inherits_the_cflx_process_environment_impl());
}

async fn adapter_inherits_the_cflx_process_environment_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let record = dir.path().join("records.txt");
    let adapter = env_probe_adapter(dir.path(), &record);

    let integration = LifecycleIntegration::start(
        Some(&config_for(vec![adapter.display().to_string()])),
        LifecycleExecutionMode::Tui,
    );
    integration
        .handle()
        .publish(LifecycleEvent::ProcessStarted { context: context() });
    integration.shutdown().await;

    let raw = fs::read_to_string(&record).expect("records");
    assert!(
        raw.contains(&format!("pane={PANE_ID}")),
        "adapter must inherit the cflx process environment: {raw}"
    );
}

#[test]
fn missing_adapter_command_does_not_prevent_startup_or_shutdown() {
    run_guarded(missing_adapter_command_does_not_prevent_startup_or_shutdown_impl());
}

async fn missing_adapter_command_does_not_prevent_startup_or_shutdown_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("definitely-not-installed-adapter");

    let integration = LifecycleIntegration::start(
        Some(&config_for(vec![missing.display().to_string()])),
        LifecycleExecutionMode::Tui,
    );

    assert!(
        !integration.is_enabled(),
        "a missing adapter must degrade to a no-op instead of failing startup"
    );

    let handle = integration.handle();
    assert!(!handle.is_enabled());
    handle.publish_state(LifecycleState::Working, context());

    let started = Instant::now();
    integration.shutdown().await;
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "shutdown must stay bounded when no adapter is running"
    );
}

#[test]
fn crashing_adapter_does_not_block_execution_or_shutdown() {
    run_guarded(crashing_adapter_does_not_block_execution_or_shutdown_impl());
}

async fn crashing_adapter_does_not_block_execution_or_shutdown_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = crashing_adapter(dir.path());

    let integration = LifecycleIntegration::start(
        Some(&config_for(vec![adapter.display().to_string()])),
        LifecycleExecutionMode::Run,
    );

    let handle = integration.handle();
    for index in 0..200 {
        handle.publish_state(
            LifecycleState::Working,
            context().with_change_id(Some(format!("change-{index}"))),
        );
    }

    let started = Instant::now();
    integration.shutdown().await;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "an early adapter exit must not stall shutdown"
    );
}

#[test]
fn adapter_that_stops_reading_cannot_block_workflow_or_shutdown() {
    run_guarded(adapter_that_stops_reading_cannot_block_workflow_or_shutdown_impl());
}

async fn adapter_that_stops_reading_cannot_block_workflow_or_shutdown_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = blocked_reader_adapter(dir.path());

    // This is the case where the shutdown deadline is what ends the wait, so it
    // is pinned tight and asserted against explicitly below.
    let config = LifecycleIntegrationConfig {
        shutdown_timeout_ms: Some(TIGHT_SHUTDOWN_MS),
        ..config_for(vec![adapter.display().to_string()])
    };
    let integration = LifecycleIntegration::start(Some(&config), LifecycleExecutionMode::Run);
    assert!(integration.is_enabled(), "adapter must start");

    // Producing far more transitions than the bounded queue holds must never block.
    let handle = integration.handle();
    let publish_started = Instant::now();
    for index in 0..5_000 {
        handle.publish_state(
            LifecycleState::Working,
            context().with_change_id(Some(format!("change-{index}"))),
        );
    }
    assert!(
        publish_started.elapsed() < Duration::from_secs(1),
        "publishing must never block on adapter backpressure"
    );

    let shutdown_started = Instant::now();
    integration.shutdown().await;
    let elapsed = shutdown_started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1_500),
        "shutdown must complete within the documented bounded deadline, took {elapsed:?}"
    );
}

#[test]
fn disabled_configuration_starts_no_adapter_process() {
    run_guarded(disabled_configuration_starts_no_adapter_process_impl());
}

async fn disabled_configuration_starts_no_adapter_process_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let record = dir.path().join("records.jsonl");
    let adapter = recording_adapter(dir.path(), &record);

    let config = LifecycleIntegrationConfig {
        enabled: Some(false),
        ..config_for(vec![adapter.display().to_string()])
    };

    let integration = LifecycleIntegration::start(Some(&config), LifecycleExecutionMode::Tui);
    integration
        .handle()
        .publish(LifecycleEvent::ProcessStarted { context: context() });
    integration.shutdown().await;

    assert!(
        !record.exists(),
        "an explicitly disabled integration must not start the adapter process"
    );
}

#[test]
fn unconfigured_integration_starts_no_adapter_process() {
    run_guarded(unconfigured_integration_starts_no_adapter_process_impl());
}

async fn unconfigured_integration_starts_no_adapter_process_impl() {
    let integration = LifecycleIntegration::start(None, LifecycleExecutionMode::Tui);

    assert!(!integration.is_enabled());
    integration.shutdown().await;
}

#[test]
fn invalid_adapter_configuration_is_rejected_without_starting_a_process() {
    run_guarded(invalid_adapter_configuration_is_rejected_without_starting_a_process_impl());
}

async fn invalid_adapter_configuration_is_rejected_without_starting_a_process_impl() {
    let config = LifecycleIntegrationConfig {
        enabled: Some(true),
        command: Vec::new(),
        ..Default::default()
    };

    assert!(
        config.validate().is_err(),
        "empty argv must be rejected with an actionable diagnostic"
    );

    let integration = LifecycleIntegration::start(Some(&config), LifecycleExecutionMode::Tui);
    assert!(!integration.is_enabled());
    integration.shutdown().await;
}

// ── Tracked Herdr reference adapter ────────────────────────────────────────

/// The reference Herdr adapter tracked in this repository.
fn tracked_herdr_adapter() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/herdr_lifecycle_adapter.py")
}

/// Adapter configuration for the tracked Herdr fixture.
///
/// The fixture is a real interpreter process, so it gets a longer shutdown
/// deadline than the shell fixtures above.
fn herdr_adapter_config() -> LifecycleIntegrationConfig {
    LifecycleIntegrationConfig {
        shutdown_timeout_ms: Some(10_000),
        ..config_for(vec![
            "python3".to_string(),
            tracked_herdr_adapter().display().to_string(),
        ])
    }
}

/// Accept one connection on a fake Herdr socket and collect its reports.
///
/// Both accept and read are bounded so an adapter that never connects fails the
/// test instead of hanging it.
fn collect_herdr_reports(listener: UnixListener) -> std::thread::JoinHandle<Vec<Value>> {
    std::thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("listener must be pollable");
        let accept_deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < accept_deadline,
                        "tracked adapter never connected to the fake Herdr socket"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("fake Herdr socket accept failed: {err}"),
            }
        };
        // Poll rather than block: `SO_RCVTIMEO` is not portable across the unix
        // socket implementations this suite runs on, and the deadline must hold
        // even if the adapter stops writing without closing the connection.
        let mut stream = stream;
        stream
            .set_nonblocking(true)
            .expect("accepted stream must be pollable");
        let read_deadline = Instant::now() + Duration::from_secs(10);
        let mut received = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => received.extend_from_slice(&chunk[..read]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= read_deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("fake Herdr socket read failed: {err}"),
            }
        }

        String::from_utf8_lossy(&received)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("each Herdr report must be valid JSON"))
            .collect()
    })
}

#[test]
fn tracked_herdr_adapter_translates_lifecycle_onto_a_fake_herdr_socket() {
    run_guarded(tracked_herdr_adapter_translates_lifecycle_onto_a_fake_herdr_socket_impl());
}

async fn tracked_herdr_adapter_translates_lifecycle_onto_a_fake_herdr_socket_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket_path = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket_path).expect("fake Herdr socket must bind");
    // Inherited by the adapter exactly like a real Herdr pane would provide it.
    std::env::set_var("HERDR_SOCKET_PATH", &socket_path);

    let reader = collect_herdr_reports(listener);

    let integration =
        LifecycleIntegration::start(Some(&herdr_adapter_config()), LifecycleExecutionMode::Tui);
    assert!(integration.is_enabled(), "tracked adapter must start");

    let handle = integration.handle();
    handle.publish(LifecycleEvent::ProcessStarted { context: context() });
    handle.publish_state(
        LifecycleState::Working,
        context().with_change_id(Some("change-a".to_string())),
    );
    handle.publish_state(
        LifecycleState::Blocked,
        context().with_change_id(Some("change-a".to_string())),
    );
    integration.shutdown().await;

    let reports = reader.join().expect("Herdr reader thread must finish");
    let kinds: Vec<&str> = reports
        .iter()
        .map(|report| report["type"].as_str().expect("report type"))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "agent_attach",
            "agent_status",
            "agent_status",
            "agent_detach"
        ],
        "tracked adapter must translate the lifecycle stream into Herdr agent reports"
    );

    for report in &reports {
        assert_eq!(
            report["pane_id"].as_str(),
            Some(PANE_ID),
            "adapter must use the pane id inherited from the environment"
        );
    }
    assert_eq!(reports[0]["mode"].as_str(), Some("tui"));
    assert_eq!(reports[1]["status"].as_str(), Some("working"));
    assert_eq!(reports[1]["change_id"].as_str(), Some("change-a"));
    assert_eq!(reports[2]["status"].as_str(), Some("blocked"));
}

#[test]
fn tracked_herdr_adapter_no_ops_outside_a_herdr_pane() {
    let _guard = spawn_guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let socket_path = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket_path).expect("fake Herdr socket must bind");
    listener
        .set_nonblocking(true)
        .expect("listener must be pollable");

    let mut child = std::process::Command::new("python3")
        .arg(tracked_herdr_adapter())
        .env_remove("HERDR_ENV")
        .env("HERDR_SOCKET_PATH", &socket_path)
        .env("HERDR_PANE_ID", PANE_ID)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("tracked adapter must be executable");

    let mut stdin = child.stdin.take().expect("piped stdin");
    stdin
        .write_all(
            br#"{"protocol_version":1,"sequence":1,"kind":"process_started","mode":"tui","pid":1}
"#,
        )
        .expect("adapter stdin must accept a lifecycle message");
    drop(stdin);

    let status = child.wait().expect("adapter must exit");
    assert!(
        status.success(),
        "adapter must exit cleanly outside a Herdr pane, got {status:?}"
    );
    assert!(
        matches!(listener.accept(), Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock),
        "adapter must not connect to Herdr when HERDR_ENV is not 1"
    );
}

// ── Entrypoint wiring ──────────────────────────────────────────────────────
//
// These tests spawn the real `cflx` binary, so they exceed the one-second
// default-suite budget and stay behind the `heavy-tests` feature.

/// Unreachable endpoint used to end a TUI entrypoint before terminal setup.
///
/// The TUI itself needs a real terminal, so these tests exercise the entrypoint
/// wiring around it: the adapter must already be running before the TUI would be
/// presented, and shutdown must still close the stream.
#[cfg(feature = "heavy-tests")]
const UNREACHABLE_SERVER: &str = "http://127.0.0.1:1";

/// Create a minimal project whose configuration records lifecycle messages.
///
/// Returns the record path the configured adapter writes to.
#[cfg(feature = "heavy-tests")]
fn setup_lifecycle_project(dir: &Path) -> PathBuf {
    let record = dir.join("records.jsonl");
    let adapter = recording_adapter(dir, &record);
    let config = serde_json::json!({
        "apply_command": "echo apply",
        "archive_command": "echo archive",
        "analyze_command": "echo analyze",
        "acceptance_command": "",
        "lifecycle_integration": { "command": [adapter.display().to_string()] },
    });
    fs::write(
        dir.join(".cflx.jsonc"),
        serde_json::to_string_pretty(&config).expect("config must serialize"),
    )
    .expect("config must be written");
    fs::create_dir_all(dir.join("openspec/changes")).expect("openspec dir");
    record
}

#[cfg(feature = "heavy-tests")]
fn run_cflx(cwd: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("cflx binary must run")
}

/// Assert the recorded stream came from a real cflx child process.
#[cfg(feature = "heavy-tests")]
fn assert_entrypoint_stream(messages: &[Value], mode: &str, workspace: &Path) {
    assert!(
        !messages.is_empty(),
        "the configured adapter must receive lifecycle messages from the {mode} entrypoint"
    );

    for (index, message) in messages.iter().enumerate() {
        assert_eq!(message["protocol_version"], 1, "versioned protocol");
        assert_eq!(
            message["sequence"],
            (index as u64) + 1,
            "sequence numbers must be gap-free"
        );
        assert_eq!(message["mode"], mode, "execution mode must be reported");
        // A placeholder or same-process fake would not carry the child's own pid.
        let pid = message["pid"].as_u64().expect("pid must be reported");
        assert!(pid > 0);
        assert_ne!(
            pid as u32,
            std::process::id(),
            "records must come from the spawned cflx process"
        );
    }

    let kinds: Vec<&str> = messages
        .iter()
        .map(|message| message["kind"].as_str().expect("kind"))
        .collect();
    assert_eq!(
        kinds.first(),
        Some(&"process_started"),
        "the adapter must be started before the entrypoint does its work: {kinds:?}"
    );
    assert_eq!(
        kinds.last(),
        Some(&"process_stopping"),
        "the entrypoint must report shutdown: {kinds:?}"
    );

    assert_eq!(
        messages[0]["context"]["workspace"].as_str(),
        Some(workspace.display().to_string().as_str()),
        "the reported workspace must be the real repository root"
    );
}

#[cfg(feature = "heavy-tests")]
#[test]
fn run_entrypoint_streams_ordered_lifecycle_to_a_configured_adapter() {
    let _guard = spawn_guard();
    let dir = tempfile::tempdir().expect("tempdir");
    let record = setup_lifecycle_project(dir.path());
    let workspace = fs::canonicalize(dir.path()).expect("canonical workspace");

    let output = run_cflx(dir.path(), &["run", "--all"]);
    assert!(
        output.status.success(),
        "cflx run --all must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let messages = read_messages(&record);
    assert_entrypoint_stream(&messages, "run", &workspace);

    let states: Vec<&str> = messages
        .iter()
        .filter_map(|message| message["state"].as_str())
        .collect();
    assert!(
        states.contains(&"working"),
        "non-interactive run must report active orchestration: {states:?}"
    );
}

#[cfg(feature = "heavy-tests")]
#[test]
fn tui_entrypoints_start_the_adapter_before_presenting_the_tui() {
    let _guard = spawn_guard();

    for args in [
        vec!["--server", UNREACHABLE_SERVER],
        vec!["tui", "--server", UNREACHABLE_SERVER],
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let record = setup_lifecycle_project(dir.path());
        let workspace = fs::canonicalize(dir.path()).expect("canonical workspace");

        let output = run_cflx(dir.path(), &args);
        assert!(
            !output.status.success(),
            "an unreachable server must end the TUI entrypoint with an error: {args:?}"
        );

        let messages = read_messages(&record);
        assert_entrypoint_stream(&messages, "tui", &workspace);
    }
}
