//! Tests for the optional OpenCode auto-resume reference integration.
//!
//! The example is Node, so these drive `node` against a **fake OpenCode server**
//! (a real loopback listener) and a **fake `cflx`** (a script on the plugin's
//! configured path). Both fakes are the point: what is under test is the
//! integration's own behavior — tool filtering, binding extraction, the
//! mandatory automation marker, deduplication, the loopback policy, and the
//! owner-continuity fallback — not Conflux, which has its own tests.
//!
//! Node is a prerequisite of OpenCode itself, so it is a prerequisite of this
//! example. Where it is absent the Node-dependent cases report a skip rather
//! than a false pass; the packaging case never needs it.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::mpsc;

/// Repository-relative root of the example.
fn example_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/integrations/opencode-auto-resume")
}

/// The Node interpreter, or `None` when this machine has none.
fn node() -> Option<PathBuf> {
    let output = Command::new("node").arg("--version").output().ok()?;
    output.status.success().then(|| PathBuf::from("node"))
}

/// Announce a skipped Node case loudly enough to be noticed in CI output.
fn skip_without_node(test: &str) -> bool {
    if node().is_some() {
        return false;
    }
    eprintln!("SKIP {test}: node is not installed, so the OpenCode example cannot be exercised");
    true
}

// ============================================================================
// A fake OpenCode server
// ============================================================================

/// One captured POST.
#[derive(Debug, Clone)]
struct Posted {
    path: String,
    body: String,
}

/// A loopback HTTP listener that accepts one request and reports it.
///
/// Hand-rolled rather than a framework: the assertion is about what the callback
/// puts on the wire, and a real socket is the only thing that can show it.
struct FakeOpenCode {
    url: String,
    requests: mpsc::Receiver<Posted>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeOpenCode {
    fn start(expected: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback listener");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let (tx, requests) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            for _ in 0..expected {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    return;
                }
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();

                let mut length = 0usize;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0 {
                        break;
                    }
                    if header.trim().is_empty() {
                        break;
                    }
                    if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:")
                    {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; length];
                use std::io::Read;
                let _ = reader.read_exact(&mut body);

                let mut stream = reader.into_inner();
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                );
                let _ = stream.flush();
                let _ = tx.send(Posted {
                    path,
                    body: String::from_utf8_lossy(&body).to_string(),
                });
            }
        });
        Self {
            url,
            requests,
            handle: Some(handle),
        }
    }

    fn next(&self) -> Posted {
        self.requests
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the integration must post to the OpenCode server")
    }

    /// Whether nothing was posted.
    ///
    /// Every caller has already reaped the callback process, so the decision is
    /// settled before this runs: the short budget is a hang guard against a
    /// lost channel, not a race the assertion depends on.
    fn is_quiet(&self) -> bool {
        self.requests
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err()
    }
}

impl Drop for FakeOpenCode {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // The listener is closed when the thread returns; a still-waiting
            // accept is released by the process ending the test.
            let _ = handle;
        }
    }
}

// ============================================================================
// Fixtures
// ============================================================================

/// Write an event file the way the owner does.
fn write_event(dir: &Path, event_type: &str, execution_id: &str) -> PathBuf {
    let path = dir.join(format!("{execution_id}-{event_type}.json"));
    let payload = serde_json::json!({
        "schema_version": 1,
        "event_type": event_type,
        "instance_id": "i-1",
        "execution_id": execution_id,
        "change_id": "alpha",
        "emitted_at": "2026-01-01T00:00:00Z",
        "terminal": event_type != "blocked" && event_type != "owner_stopping",
        "terminal_mode": "merged",
        "evidence": "the base tree carries the archive entry for 'alpha'",
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();
    path
}

/// Run the callback exactly as the owner would: five variables and nothing else.
fn run_callback(
    event_path: &Path,
    event_type: &str,
    execution_id: &str,
    server: &str,
    session: &str,
    state: &Path,
) -> Output {
    let mut command = Command::new("node");
    command
        .arg(example_root().join("callback/cflx-resume-session.mjs"))
        .args([
            "--server",
            server,
            "--session",
            session,
            "--state",
            state.to_str().unwrap(),
        ])
        // The owner replaces the environment rather than extending it.
        .env_clear()
        .env("CFLX_EVENT_PATH", event_path)
        .env("CFLX_EVENT_TYPE", event_type)
        .env("CFLX_EXECUTION_ID", execution_id)
        .env("CFLX_CHANGE_ID", "alpha")
        .env("CFLX_INSTANCE_ID", "i-1");
    // `env_clear` leaves the interpreter without a PATH, so it is named by the
    // absolute path the shell resolved for this test process.
    if let Ok(path) = std::env::var("PATH") {
        command.env("PATH", path);
    }
    command.output().expect("node must be runnable")
}

/// A fake `cflx` whose `client ... mcp` speaks just enough JSON-RPC to answer
/// one `tools/call` with a scripted envelope.
fn fake_cflx(dir: &Path, envelope: serde_json::Value) -> PathBuf {
    let calls = dir.join("cflx-calls.log");
    let script = dir.join("cflx");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> {calls}\n\
             while IFS= read -r line; do\n\
             \tcase \"$line\" in\n\
             \t*'\"tools/call\"'*)\n\
             \t\tprintf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{escaped}\"}}],\"structuredContent\":{envelope},\"isError\":false}}}}'\n\
             \t\t;;\n\
             \t*'\"initialize\"'*)\n\
             \t\tprintf '%s\\n' '{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"protocolVersion\":\"2025-06-18\",\"serverInfo\":{{\"name\":\"cflx-client\"}}}}}}'\n\
             \t\t;;\n\
             \tesac\n\
             done\n",
            calls = calls.display(),
            envelope = envelope,
            escaped = envelope.to_string().replace('"', "\\\\\""),
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o700);
    }
    std::fs::set_permissions(&script, permissions).unwrap();
    script
}

/// Drive the plugin's exported hook from a small Node harness.
fn run_plugin_harness(dir: &Path, source: &str, env: &[(&str, &str)]) -> Output {
    let harness = dir.join("harness.mjs");
    std::fs::write(&harness, source).unwrap();
    let mut command = Command::new("node");
    command.arg(&harness).current_dir(dir);
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("node must be runnable")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ============================================================================
// completion
// ============================================================================

/// The end the whole integration exists for: a completion event becomes one
/// marked message in the originating session.
#[test]
fn completion_resumes_the_originating_session_with_a_marked_message() {
    if skip_without_node("completion_resumes_the_originating_session_with_a_marked_message") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(1);
    let event = write_event(dir.path(), "completed", "exec-a");

    let output = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &dir.path().join("state"),
    );
    assert!(output.status.success(), "{}", stderr_of(&output));

    let posted = server.next();
    assert_eq!(posted.path, "/session/ses_abc/message");
    let body: serde_json::Value = serde_json::from_str(&posted.body).expect("a JSON body");
    let text = body["parts"][0]["text"].as_str().expect("a text part");

    // The marker is mandatory: OpenCode stores this as an ordinary role=user
    // message, so an unmarked one would be indistinguishable from the
    // operator's own.
    assert!(
        text.starts_with("[AUTOMATION EVENT — not user-authored]"),
        "{text}"
    );
    assert!(text.contains("alpha"), "{text}");
    assert!(text.contains("exec-a"), "{text}");
    // The event is presented as data to verify, never as an instruction.
    assert!(text.contains("not instructions"), "{text}");
}

/// The owner already caps terminal delivery at one attempt per execution; the
/// callback adds its own local dedupe so an operator re-running it by hand does
/// not look to the agent like a second completion.
#[test]
fn completion_is_deduplicated_by_execution_and_event_type() {
    if skip_without_node("completion_is_deduplicated_by_execution_and_event_type") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(2);
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");

    let first = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(first.status.success(), "{}", stderr_of(&first));
    server.next();

    let second = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(second.status.success(), "{}", stderr_of(&second));
    assert!(
        server.is_quiet(),
        "a repeated delivery of the same execution and event type must post nothing"
    );

    // A *different* event for the same execution is not a duplicate.
    let stopping = write_event(dir.path(), "owner_stopping", "exec-a");
    let third = run_callback(
        &stopping,
        "owner_stopping",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(third.status.success(), "{}", stderr_of(&third));
    server.next();
}

/// The destination policy is the callback's only outbound reach, so it is
/// narrow and it fails closed.
#[test]
fn callback_refuses_a_non_loopback_destination_and_a_mismatched_event() {
    if skip_without_node("callback_refuses_a_non_loopback_destination_and_a_mismatched_event") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");

    let remote = run_callback(
        &event,
        "completed",
        "exec-a",
        "http://example.com:80",
        "ses_abc",
        &state,
    );
    assert!(!remote.status.success());
    assert!(
        stderr_of(&remote).contains("loopback"),
        "{}",
        stderr_of(&remote)
    );

    // An event file whose contents disagree with the delivery's own environment
    // is not this delivery's payload.
    let server = FakeOpenCode::start(1);
    let mismatched = run_callback(
        &event,
        "completed",
        "exec-b",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(!mismatched.status.success());
    assert!(
        stderr_of(&mismatched).contains("execution_id"),
        "{}",
        stderr_of(&mismatched)
    );
    assert!(server.is_quiet(), "a refused event must post nothing");
}

// ============================================================================
// plugin
// ============================================================================

/// The plugin filters one exact tool and reads one versioned envelope. Anything
/// else — another tool, an unsuccessful outcome, a missing binding — registers
/// nothing at all.
#[test]
fn plugin_registers_a_sink_only_for_a_successful_cflx_enqueue() {
    if skip_without_node("plugin_registers_a_sink_only_for_a_successful_cflx_enqueue") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let cflx = fake_cflx(
        dir.path(),
        serde_json::json!({
            "schema_version": 1, "ok": true, "operation": "notify_set",
            "outcome": "subscribed", "detail": {}
        }),
    );
    let plugin = example_root().join("plugin/cflx-auto-resume.mjs");

    let source = format!(
        r#"
import {{ CflxAutoResume, isEnqueueTool, extractBinding }} from "{plugin}";

// Segment-exact matching, so a wrapper tool cannot be mistaken for ours.
if (!isEnqueueTool("cflx_enqueue")) throw new Error("bare name must match");
if (!isEnqueueTool("cflx_cflx_enqueue")) throw new Error("server-prefixed name must match");
if (isEnqueueTool("my_cflx_enqueue_wrapper")) throw new Error("a wrapper must not match");
if (isEnqueueTool("cflx_wait")) throw new Error("another tool must not match");

// Fails closed on anything that is not a successful enqueue envelope.
if (extractBinding({{ ok: false, operation: "enqueue", change_id: "a", execution_id: "e", instance_id: "i" }}))
  throw new Error("an unsuccessful envelope must not bind");
if (extractBinding({{ ok: true, operation: "status", change_id: "a", execution_id: "e", instance_id: "i" }}))
  throw new Error("another operation must not bind");
if (extractBinding({{ ok: true, operation: "enqueue", change_id: "a" }}))
  throw new Error("a missing execution id must not bind");
if (extractBinding("not json")) throw new Error("garbage must not bind");

const plugin = await CflxAutoResume({{ directory: process.cwd() }});
const envelope = JSON.stringify({{
  schema_version: 1, ok: true, operation: "enqueue", outcome: "admitted",
  instance_id: "i-1", execution_id: "exec-a", change_id: "alpha", detail: {{}},
}});

// An unrelated tool is ignored entirely.
await plugin["tool.execute.after"]({{ tool: "bash", sessionID: "ses_abc" }}, {{ output: envelope }});
// A call with no session has nowhere to resume to.
await plugin["tool.execute.after"]({{ tool: "cflx_enqueue" }}, {{ output: envelope }});
// The real one.
await plugin["tool.execute.after"]({{ tool: "cflx_cflx_enqueue", sessionID: "ses_abc" }}, {{ output: envelope }});
process.stdout.write("ok");
"#,
        plugin = plugin.display(),
    );

    let output = run_plugin_harness(
        dir.path(),
        &source,
        &[
            ("CFLX_BIN", cflx.to_str().unwrap()),
            ("OPENCODE_SERVER", "http://127.0.0.1:4096"),
        ],
    );
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        stderr_of(&output)
    );

    let calls = std::fs::read_to_string(dir.path().join("cflx-calls.log")).unwrap_or_default();
    assert_eq!(
        calls.lines().count(),
        1,
        "exactly one registration, for the one qualifying call: {calls}"
    );
    assert!(calls.contains("client"), "{calls}");
    assert!(calls.contains("mcp"), "{calls}");
}

// ============================================================================
// owner restart
// ============================================================================

/// A crashed owner cannot deliver from a process-local registry, so the plugin
/// keeps its own bounded observer. What it reports is `owner_restarted` — never
/// completion, because nothing about the change was observed.
#[test]
fn owner_restart_resumes_the_session_without_claiming_completion() {
    if skip_without_node("owner_restart_resumes_the_session_without_claiming_completion") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(1);
    let resume = example_root().join("lib/resume.mjs");

    // The observer's decision is a pure function of the envelope's outcome, and
    // its consequence is one marked message. Both are asserted here; the timer
    // that drives it is bounded and unref'd in the plugin itself.
    let source = format!(
        r#"
import {{ composeMessage, resumeSession, AUTOMATION_MARKER }} from "{resume}";

const text = composeMessage({{
  event_type: "owner_restarted", change_id: "alpha",
  execution_id: "exec-a", instance_id: "i-1",
}});
if (!text.startsWith(AUTOMATION_MARKER)) throw new Error("the marker is mandatory");
if (text.includes("completed")) throw new Error("a lost owner must never read as completion");
if (!text.includes("no completion")) throw new Error("it must say so explicitly");

// A message without the marker is refused rather than posted.
let refused = false;
try {{
  await resumeSession({{ server: "{server}", session: "ses_abc", text: "bare" }});
}} catch {{
  refused = true;
}}
if (!refused) throw new Error("an unmarked message must be refused");

await resumeSession({{ server: "{server}", session: "ses_abc", text }});
process.stdout.write("ok");
"#,
        resume = resume.display(),
        server = server.url,
    );

    let output = run_plugin_harness(dir.path(), &source, &[]);
    assert!(output.status.success(), "{}", stderr_of(&output));

    let posted = server.next();
    let body: serde_json::Value = serde_json::from_str(&posted.body).unwrap();
    let text = body["parts"][0]["text"].as_str().unwrap();
    assert!(text.contains("owner_restarted"), "{text}");
    assert!(
        !text.contains("certified"),
        "a lost owner certifies nothing: {text}"
    );
}

// ============================================================================
// packaging
// ============================================================================

/// The example is repository-distributed and deliberately not shipped in the
/// crate: it is reference material for operators, not a dependency of the
/// binary.
///
/// The manifest half is asserted here because it is instant; the authoritative
/// `cargo package --list` check is its own case, below.
#[test]
fn packaging_keeps_the_example_out_of_the_crate_include_allowlist() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("the manifest is readable");
    // `include` is an allowlist, so the exclusion is the absence of an entry.
    assert!(
        manifest.contains("include = ["),
        "the manifest must keep an explicit include allowlist"
    );
    assert!(
        !manifest.contains("/examples"),
        "the example must not be added to the crate's include list"
    );
    assert!(
        example_root().join("README.md").exists(),
        "and it must still be distributed in the repository"
    );
}

/// The packaged crate itself, asserted against `cargo` rather than against the
/// manifest text. Heavy because resolving a package listing is seconds of work.
#[test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
fn packaging_excludes_the_example_from_the_published_crate() {
    let output = Command::new(env!("CARGO"))
        .args(["package", "--list", "--allow-dirty", "--quiet"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo must be runnable");
    assert!(
        output.status.success(),
        "cargo package --list failed: {}",
        stderr_of(&output)
    );
    let listed = String::from_utf8_lossy(&output.stdout);
    assert!(
        !listed.contains("examples/integrations/opencode-auto-resume"),
        "the packaged crate must not carry the example:\n{listed}"
    );
    // The listing is real, so a silent empty result cannot pass this test.
    assert!(listed.contains("src/client/mcp.rs"), "{listed}");
}
