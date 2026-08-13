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
use std::time::{Duration, SystemTime};

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

/// What the fake server answers with.
#[derive(Debug, Clone)]
enum Reply {
    /// The ordinary success OpenCode gives.
    Ok,
    /// A bounce to somewhere else, which the callback must refuse to follow.
    Redirect(String),
    /// A delivery failure that leaves the callback nothing to promote.
    ServerError,
}

impl Reply {
    fn bytes(&self) -> Vec<u8> {
        match self {
            Reply::Ok => b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}"
                .to_vec(),
            Reply::Redirect(location) => format!(
                "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .into_bytes(),
            Reply::ServerError => {
                b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec()
            }
        }
    }
}

/// A loopback HTTP listener that accepts one request and reports it.
///
/// Hand-rolled rather than a framework: the assertion is about what the callback
/// puts on the wire, and a real socket is the only thing that can show it.
struct FakeOpenCode {
    url: String,
    authority: String,
    port: u16,
    requests: mpsc::Receiver<Posted>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl FakeOpenCode {
    fn start(expected: usize) -> Self {
        Self::start_with(expected, Reply::Ok)
    }

    fn start_with(expected: usize, reply: Reply) -> Self {
        Self::bind("127.0.0.1", expected, reply).expect("bind an IPv4 loopback listener")
    }

    /// Listen on an explicit loopback address, or `None` when this machine has
    /// no such stack. A host without IPv6 is a real configuration rather than a
    /// test failure, so the caller decides what to do about it.
    fn bind(host: &str, expected: usize, reply: Reply) -> Option<Self> {
        let listener = TcpListener::bind((host, 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        // The URL authority form for an IPv6 literal is bracketed.
        let authority = if host.contains(':') {
            format!("[{host}]:{port}")
        } else {
            format!("{host}:{port}")
        };
        let url = format!("http://{authority}");
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
                let _ = stream.write_all(&reply.bytes());
                let _ = stream.flush();
                let _ = tx.send(Posted {
                    path,
                    body: String::from_utf8_lossy(&body).to_string(),
                });
            }
        });
        Some(Self {
            url,
            authority,
            port,
            requests,
            handle: Some(handle),
        })
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
    run_callback_with_path(
        event_path,
        event_type,
        execution_id,
        server,
        session,
        state,
        None,
    )
}

/// The same, with an explicit `--path` — the argument the origin policy guards.
fn run_callback_with_path(
    event_path: &Path,
    event_type: &str,
    execution_id: &str,
    server: &str,
    session: &str,
    state: &Path,
    path: Option<&str>,
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
        ]);
    if let Some(path) = path {
        command.args(["--path", path]);
    }
    command
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

/// Run the callback with no `--state`, so it derives its own default, and with
/// the home directory the default is derived from.
///
/// `HOME` is what the owner does *not* pass — the environment is replaced — so
/// a real delivery falls through to the passwd entry for the same uid. Naming
/// it here points the identical derivation at a directory this test owns.
fn run_callback_default_state(
    event_path: &Path,
    event_type: &str,
    execution_id: &str,
    server: &str,
    session: &str,
    home: &Path,
) -> Output {
    let mut command = Command::new("node");
    command
        .arg(example_root().join("callback/cflx-resume-session.mjs"))
        .args(["--server", server, "--session", session])
        .env_clear()
        .env("HOME", home)
        .env("CFLX_EVENT_PATH", event_path)
        .env("CFLX_EVENT_TYPE", event_type)
        .env("CFLX_EXECUTION_ID", execution_id)
        .env("CFLX_CHANGE_ID", "alpha")
        .env("CFLX_INSTANCE_ID", "i-1");
    if let Ok(path) = std::env::var("PATH") {
        command.env("PATH", path);
    }
    command.output().expect("node must be runnable")
}

/// Where the callback keeps delivery state when nobody names a directory.
fn default_state_dir(home: &Path) -> PathBuf {
    home.join(".local/state/cflx/opencode-auto-resume")
}

/// Create a directory with exactly the permission bits asked for.
fn dir_with_mode(path: &Path, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    path.to_path_buf()
}

/// The permission bits of an existing path.
fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

/// Whether a state directory holds any claim or delivery record at all.
fn has_delivery_records(state: &Path) -> bool {
    std::fs::read_dir(state)
        .map(|entries| entries.flatten().count() > 0)
        .unwrap_or(false)
}

/// Exit status the callback uses for "another process holds a fresh claim".
///
/// Distinct from 1 on purpose: nothing failed, so an operator wrapper can tell a
/// refused duplicate apart from a delivery that could not be made.
const EXIT_IN_FLIGHT: i32 = 75;

/// The claim/marker pair for one execution event, as the callback names them.
fn state_paths(state: &Path, execution_id: &str, event_type: &str) -> (PathBuf, PathBuf) {
    (
        state.join(format!("{execution_id}.{event_type}.inflight")),
        state.join(format!("{execution_id}.{event_type}.done")),
    )
}

/// Write an in-flight claim the way a live callback would.
///
/// Including the directory it would have created: the callback makes its state
/// directory `0700` and refuses one that is not, so a fixture left at the
/// process umask would be describing a state no live callback produces.
fn write_claim(path: &Path) {
    dir_with_mode(path.parent().unwrap(), 0o700);
    std::fs::write(
        path,
        "{\"owner\":\"someone-else\",\"claimed_at\":\"2026-01-01T00:00:00Z\"}\n",
    )
    .unwrap();
}

/// Backdate a file so staleness is decided by recorded state, never by waiting.
fn age_file(path: &Path, age: Duration) {
    let when = SystemTime::now() - age;
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(when)
            .set_modified(when),
    )
    .unwrap();
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

/// A hostname is not a destination, it is a question for the resolver. What
/// `localhost` answers is decided by `/etc/hosts`, NSS, and search domains, so
/// accepting it would mean asserting something this policy cannot check. Only
/// the two literal loopback addresses are accepted.
#[test]
fn only_literal_loopback_destinations_are_accepted() {
    if skip_without_node("only_literal_loopback_destinations_are_accepted") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(1);
    let event = write_event(dir.path(), "completed", "exec-a");
    let state = dir.path().join("state");

    // `localhost` on this machine almost certainly *is* the listener below —
    // which is the point. It is refused for being a name, not for where it
    // happens to point today.
    let refused = run_callback(
        &event,
        "completed",
        "exec-a",
        &format!("http://localhost:{}", server.port),
        "ses_abc",
        &state,
    );
    assert!(!refused.status.success(), "a hostname must be refused");
    let stderr = stderr_of(&refused);
    assert!(stderr.contains("loopback"), "{stderr}");
    assert!(
        server.is_quiet(),
        "a hostname must be refused before a connection is opened"
    );
    assert!(
        !state.exists(),
        "and before any delivery state is created, so a corrected retry is unblocked"
    );

    // The same listener, named by its literal address, is delivered to.
    let accepted = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(accepted.status.success(), "{}", stderr_of(&accepted));
    assert_eq!(server.next().path, "/session/ses_abc/message");
}

/// The IPv6 half of the same policy, against a real `[::1]` listener.
#[test]
fn literal_loopback_includes_the_ipv6_address() {
    if skip_without_node("literal_loopback_includes_the_ipv6_address") {
        return;
    }
    let Some(server) = FakeOpenCode::bind("::1", 1, Reply::Ok) else {
        eprintln!("SKIP literal_loopback_includes_the_ipv6_address: no IPv6 loopback on this host");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
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
    assert_eq!(server.next().path, "/session/ses_abc/message");
}

/// The plugin builds an argv the *owner* will execute, so its destination is
/// proven before registration rather than when something finally posts to it.
#[test]
fn plugin_registers_nothing_for_a_non_literal_loopback_server() {
    if skip_without_node("plugin_registers_nothing_for_a_non_literal_loopback_server") {
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
import {{ CflxAutoResume }} from "{plugin}";

const plugin = await CflxAutoResume({{ directory: process.cwd() }});
const envelope = JSON.stringify({{
  schema_version: 1, ok: true, operation: "enqueue", outcome: "admitted",
  instance_id: "i-1", execution_id: "exec-a", change_id: "alpha", detail: {{}},
}});
// A qualifying call in every other respect. The destination is what refuses it.
await plugin["tool.execute.after"]({{ tool: "cflx_enqueue", sessionID: "ses_abc" }}, {{ output: envelope }});
process.stdout.write("ok");
"#,
        plugin = plugin.display(),
    );

    let output = run_plugin_harness(
        dir.path(),
        &source,
        &[
            ("CFLX_BIN", cflx.to_str().unwrap()),
            ("OPENCODE_SERVER", "http://localhost:4096"),
        ],
    );
    // Refusing is not crashing: the hook declines and says so on stderr.
    assert!(output.status.success(), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("loopback"),
        "{}",
        stderr_of(&output)
    );
    assert!(
        !dir.path().join("cflx-calls.log").exists(),
        "a hostname destination must register no sink at all"
    );
}

/// The base URL is not the boundary; the *final* URL is. `new URL` lets a path
/// replace everything before it, so `--path` is checked for scheme,
/// protocol-relative, and backslash shapes and then the resolved origin is
/// compared with the base's.
#[test]
fn callback_refuses_an_origin_changing_path_before_sending() {
    if skip_without_node("callback_refuses_an_origin_changing_path_before_sending") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let allowed = FakeOpenCode::start(1);
    // A second *loopback* listener: the refusal must come from the origin
    // change itself, not from the destination merely being off-machine.
    let elsewhere = FakeOpenCode::start(1);
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");

    let cases = [
        format!("{}/session/{{session}}/message", elsewhere.url),
        format!("//{}/session/{{session}}/message", elsewhere.authority),
        format!("/\\{}/session/{{session}}/message", elsewhere.authority),
        format!("\\\\{}/session/{{session}}/message", elsewhere.authority),
    ];
    for path in cases {
        let output = run_callback_with_path(
            &event,
            "completed",
            "exec-a",
            &allowed.url,
            "ses_abc",
            &state,
            Some(&path),
        );
        assert!(!output.status.success(), "'{path}' must be refused");
        let stderr = stderr_of(&output);
        assert!(
            stderr.contains("scheme")
                || stderr.contains("protocol-relative")
                || stderr.contains("change origin"),
            "'{path}': {stderr}"
        );
    }

    assert!(
        elsewhere.is_quiet(),
        "an origin-changing path must not open a connection to its target"
    );
    assert!(
        allowed.is_quiet(),
        "and nothing was sent to the base either"
    );

    let (inflight, done) = state_paths(&state, "exec-a", "completed");
    assert!(!done.exists(), "a refusal is not a delivery");
    assert!(
        !inflight.exists(),
        "and it must not leave a claim that blocks a corrected retry"
    );
}

/// Following a redirect is how a loopback listener would bounce a callback off
/// the machine, so redirects are refused rather than followed.
#[test]
fn callback_refuses_a_redirect_without_reaching_its_target() {
    if skip_without_node("callback_refuses_a_redirect_without_reaching_its_target") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let destination = FakeOpenCode::start(1);
    let server = FakeOpenCode::start_with(
        1,
        Reply::Redirect(format!("{}/session/ses_abc/message", destination.url)),
    );
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");

    let output = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(!output.status.success(), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("redirect"),
        "{}",
        stderr_of(&output)
    );

    // The first hop really happened; the second one must not have.
    server.next();
    assert!(
        destination.is_quiet(),
        "the redirect target must receive no connection at all"
    );

    let (inflight, done) = state_paths(&state, "exec-a", "completed");
    assert!(!done.exists(), "a refused redirect is not a delivery");
    assert!(!inflight.exists(), "and the claim is released for a retry");
}

/// The claim is state, not a timer: every case here is decided by a file that
/// already exists when the callback starts, so nothing depends on a race.
#[test]
fn callback_claim_refuses_fresh_takes_over_stale_and_honours_delivery() {
    if skip_without_node("callback_claim_refuses_fresh_takes_over_stale_and_honours_delivery") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(1);
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");
    let (inflight, done) = state_paths(&state, "exec-a", "completed");

    // A fresh claim belongs to a live process: stand down, distinctly.
    write_claim(&inflight);
    let refused = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert_eq!(
        refused.status.code(),
        Some(EXIT_IN_FLIGHT),
        "a fresh in-flight claim is a distinct non-success: {}",
        stderr_of(&refused)
    );
    assert!(server.is_quiet(), "and it posts nothing");
    assert!(inflight.exists(), "the live process keeps its claim");
    assert!(!done.exists());

    // The same claim, older than the bound: a crashed process must not suppress
    // delivery forever, so a later invocation takes it over and delivers.
    age_file(&inflight, Duration::from_secs(6 * 60));
    let takeover = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(takeover.status.success(), "{}", stderr_of(&takeover));
    let posted = server.next();
    assert_eq!(posted.path, "/session/ses_abc/message");
    assert!(done.exists(), "success is promoted to a durable marker");
    assert!(!inflight.exists(), "and the claim is consumed by it");

    // With the marker in place a later duplicate is a plain success, not a POST.
    let duplicate = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &state,
    );
    assert!(duplicate.status.success(), "{}", stderr_of(&duplicate));
    assert!(
        server.is_quiet(),
        "an existing success marker must not post again"
    );
}

/// The bug the split was introduced for: the old marker was written *before* the
/// POST, so one failed delivery silenced every later attempt.
#[test]
fn failed_delivery_releases_the_claim_so_a_later_invocation_retries() {
    if skip_without_node("failed_delivery_releases_the_claim_so_a_later_invocation_retries") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let broken = FakeOpenCode::start_with(1, Reply::ServerError);
    let state = dir.path().join("state");
    let event = write_event(dir.path(), "completed", "exec-a");
    let (inflight, done) = state_paths(&state, "exec-a", "completed");

    let failed = run_callback(
        &event,
        "completed",
        "exec-a",
        &broken.url,
        "ses_abc",
        &state,
    );
    assert!(!failed.status.success(), "{}", stderr_of(&failed));
    assert!(stderr_of(&failed).contains("500"), "{}", stderr_of(&failed));
    broken.next();
    assert!(!done.exists(), "a failed POST is not a delivery");
    assert!(
        !inflight.exists(),
        "and the claim is released rather than left to expire"
    );

    // Which is the whole point: recovery needs no waiting and no cleanup.
    let working = FakeOpenCode::start(1);
    let retried = run_callback(
        &event,
        "completed",
        "exec-a",
        &working.url,
        "ses_abc",
        &state,
    );
    assert!(retried.status.success(), "{}", stderr_of(&retried));
    working.next();
    assert!(done.exists());
}

/// The claim and the marker decide whether a message is sent, so a directory
/// anybody else on the machine can reach is not one to keep them in. Each
/// refusal happens before a record is read or created, and before any HTTP.
///
/// Foreign ownership is checked by the callback but not exercised here: an
/// unprivileged test cannot create a file owned by another user, and a test
/// that needed root would not run.
#[test]
fn callback_requires_private_state_before_it_claims_anything() {
    if skip_without_node("callback_requires_private_state_before_it_claims_anything") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let server = FakeOpenCode::start(1);
    let event = write_event(dir.path(), "completed", "exec-a");

    // A symlink: what it points at can be changed after it is checked.
    let real = dir_with_mode(&dir.path().join("real"), 0o700);
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let output = run_callback(&event, "completed", "exec-a", &server.url, "ses_abc", &link);
    assert!(!output.status.success(), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("symlink"),
        "{}",
        stderr_of(&output)
    );
    assert!(
        !has_delivery_records(&real),
        "a refused state path must leave no claim behind its link either"
    );

    // A plain file where a directory belongs.
    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, b"x").unwrap();
    let output = run_callback(&event, "completed", "exec-a", &server.url, "ses_abc", &file);
    assert!(!output.status.success(), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("not a directory"),
        "{}",
        stderr_of(&output)
    );

    // A directory the rest of the machine can read, write, or list.
    for mode in [0o755, 0o770, 0o707] {
        let open = dir_with_mode(&dir.path().join(format!("open-{mode:o}")), mode);
        let output = run_callback(&event, "completed", "exec-a", &server.url, "ses_abc", &open);
        assert!(!output.status.success(), "mode {mode:o} must be refused");
        assert!(
            stderr_of(&output).contains("0700"),
            "{}",
            stderr_of(&output)
        );
        assert!(
            !has_delivery_records(&open),
            "mode {mode:o} must be refused before a claim is created"
        );
    }

    assert!(
        server.is_quiet(),
        "an unsafe state directory must send no OpenCode request"
    );

    // And the private one still delivers, with the ordinary claim semantics.
    let private = dir_with_mode(&dir.path().join("private"), 0o700);
    let output = run_callback(
        &event,
        "completed",
        "exec-a",
        &server.url,
        "ses_abc",
        &private,
    );
    assert!(output.status.success(), "{}", stderr_of(&output));
    server.next();
    let (inflight, done) = state_paths(&private, "exec-a", "completed");
    assert!(done.exists(), "a delivered event is marked delivered");
    assert!(
        !inflight.exists(),
        "and its claim was promoted, not left behind"
    );
}

/// The default path is derived per user rather than shared. A predictable path
/// under the system temporary directory could be created first by anybody, and
/// whoever owns it owns every other user's delivery decisions.
#[test]
fn private_state_defaults_to_an_owner_private_directory() {
    if skip_without_node("private_state_defaults_to_an_owner_private_directory") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let server = FakeOpenCode::start(1);
    let event = write_event(dir.path(), "completed", "exec-a");

    let output =
        run_callback_default_state(&event, "completed", "exec-a", &server.url, "ses_abc", &home);
    assert!(output.status.success(), "{}", stderr_of(&output));
    server.next();

    let state = default_state_dir(&home);
    let (_, done) = state_paths(&state, "exec-a", "completed");
    assert!(
        done.exists(),
        "the default state directory must be the one under this user's home"
    );
    assert_eq!(
        mode_of(&state),
        0o700,
        "and it must be created closed, not relaxed afterwards"
    );

    // Pre-created wide open — the shape a second local user would leave behind.
    // The default is checked exactly like a supplied one.
    dir_with_mode(&state, 0o755);
    let quiet = FakeOpenCode::start(1);
    let event_b = write_event(dir.path(), "completed", "exec-b");
    let output = run_callback_default_state(
        &event_b,
        "completed",
        "exec-b",
        &quiet.url,
        "ses_abc",
        &home,
    );
    assert!(!output.status.success(), "{}", stderr_of(&output));
    assert!(
        stderr_of(&output).contains("0700"),
        "{}",
        stderr_of(&output)
    );
    assert!(quiet.is_quiet(), "and nothing was posted");
    let (inflight_b, done_b) = state_paths(&state, "exec-b", "completed");
    assert!(!inflight_b.exists() && !done_b.exists());
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

/// The result of an enqueue becomes an argv the owner executes, so the plugin
/// reads it as the versioned contract it is: a version it understands, an
/// outcome that admitted something, and three identifiers that are actually
/// identifiers. Anything else registers nothing.
#[test]
fn plugin_rejects_an_incompatible_enqueue_envelope() {
    if skip_without_node("plugin_rejects_an_incompatible_enqueue_envelope") {
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
import {{ CflxAutoResume, extractBinding }} from "{plugin}";

const base = {{
  schema_version: 1, ok: true, operation: "enqueue", outcome: "admitted",
  instance_id: "i-1", execution_id: "exec-a", change_id: "alpha", detail: {{}},
}};
const reject = (patch, why) => {{
  if (extractBinding({{ ...base, ...patch }})) throw new Error(why);
}};

// The version gate: an envelope whose shape this plugin has never seen is not
// one to read three fields out of.
reject({{ schema_version: undefined }}, "an unversioned envelope must not bind");
reject({{ schema_version: 2 }}, "a future schema version must not bind");
reject({{ schema_version: "1" }}, "a non-numeric schema version must not bind");

// The outcome gate: success is narrow, and only an admission created an episode.
reject({{ outcome: undefined }}, "a missing outcome must not bind");
reject({{ outcome: "owner_not_running" }}, "a refusal must not bind");
reject({{ outcome: "observed" }}, "another operation's success must not bind");

// The identifier gate: truthiness is not a type.
reject({{ change_id: 7 }}, "a non-string change id must not bind");
reject({{ execution_id: {{ id: "exec-a" }} }}, "an object execution id must not bind");
reject({{ instance_id: "" }}, "an empty instance id must not bind");
reject({{ execution_id: "   " }}, "a blank execution id must not bind");

// And what the current contract actually returns.
if (!extractBinding(base)) throw new Error("an admitted envelope must bind");
if (!extractBinding({{ ...base, outcome: "already_admitted" }}))
  throw new Error("already_admitted is an admission too");
if (!extractBinding(JSON.stringify(base))) throw new Error("the JSON text form must bind");

// End to end: an incompatible envelope reaches the hook and registers nothing.
const plugin = await CflxAutoResume({{ directory: process.cwd() }});
await plugin["tool.execute.after"](
  {{ tool: "cflx_enqueue", sessionID: "ses_abc" }},
  {{ output: JSON.stringify({{ ...base, schema_version: 99 }}) }},
);
await plugin["tool.execute.after"](
  {{ tool: "cflx_enqueue", sessionID: "ses_abc" }},
  {{ output: JSON.stringify({{ ...base, outcome: "target_ineligible" }}) }},
);
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
    assert!(
        !dir.path().join("cflx-calls.log").exists(),
        "an incompatible envelope must register no completion sink"
    );
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
