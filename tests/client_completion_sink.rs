//! Integration tests for execution-scoped completion sinks.
//!
//! Everything here drives the **production listener path**: `start_listeners`
//! binds a real Unix socket and a real TCP socket over one shared router and one
//! shared `WebState`, exactly as `cflx`, `cflx tui`, and `cflx run` do. That is
//! deliberate — the properties under test are transport-scoped (`PUT` is
//! accepted on the socket and refused on TCP), and a test that called the router
//! in-process would be asserting the marker it injected itself.
//!
//! Callbacks are real child processes writing to real files, because "the owner
//! executed an argv with exactly these five environment variables and nothing
//! else" is not something an in-memory double can prove.
//!
//! Group names match the change's tasks: `transport`, `capability`,
//! `registration`, `delivery`, `restart`, `shutdown`.

#![cfg(all(unix, feature = "web-monitoring"))]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use conflux::events::{ExecutionEvent, OperatorCommandEffect};
use conflux::openspec::{Change, ProposalMetadata};
use conflux::orchestration::execution_facts::ExecutionFactsStore;
use conflux::orchestration::state::{OrchestratorState, ReducerCommand};
use conflux::web::remote_control_api::dto::{OwnerExecutionContract, TerminalMode};
use conflux::web::{ListenerPlan, ServerHandle, WebConfig, WebState};

// ============================================================================
// Harness
// ============================================================================

/// One owner: real listeners, a real registry, and the reducer that feeds it.
struct Owner {
    socket: PathBuf,
    tcp: String,
    state: Arc<WebState>,
    facts: Arc<ExecutionFactsStore>,
    reducer: OrchestratorState,
    handle: Option<ServerHandle>,
    dispatch: u64,
    _dir: tempfile::TempDir,
}

impl Owner {
    async fn start(change_ids: &[&str]) -> Self {
        Self::start_on(tempfile::tempdir().expect("temp dir"), change_ids).await
    }

    /// Start on a caller-owned directory, so a test can replace the process
    /// serving one socket path.
    async fn start_on(dir: tempfile::TempDir, change_ids: &[&str]) -> Self {
        let socket = dir.path().join("cflx-api.sock");
        // The projection publishes per-change execution facts only for changes
        // the owner is actually tracking, so the snapshot has to hold them.
        let changes: Vec<Change> = change_ids
            .iter()
            .map(|id| Change {
                id: (*id).to_string(),
                completed_tasks: 0,
                total_tasks: 2,
                last_modified: "just now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            })
            .collect();
        let state = Arc::new(WebState::new(&changes));
        let facts = Arc::new(ExecutionFactsStore::new());
        // The same call the TUI makes, which is what starts the registry.
        state.set_execution_facts(facts.clone()).await;
        state.set_repo_root(dir.path().to_path_buf()).await;
        state.set_execution_contract(OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        });

        let config = WebConfig {
            enabled: true,
            port: 0,
            bind: "127.0.0.1".to_string(),
            refresh_interval_secs: 0,
            ..WebConfig::default()
        };
        // Publish the monitoring snapshot so the v2 projection tracks these
        // changes; the execution-status resource joins per-change facts onto it.
        state.update_with_mode(&changes, "select").await;

        let handle = conflux::web::start_listeners(
            config,
            ListenerPlan {
                unix_socket: Some(socket.clone()),
                tcp: true,
            },
            state.clone(),
        )
        .await
        .expect("both listeners must bind");
        let tcp = handle
            .tcp_url()
            .expect("the TCP listener publishes its URL")
            .to_string();

        Self {
            socket,
            tcp,
            state,
            facts,
            reducer: OrchestratorState::new(
                change_ids.iter().map(|id| (*id).to_string()).collect(),
                0,
            ),
            handle: Some(handle),
            dispatch: 0,
            _dir: dir,
        }
    }

    fn instance_id(&self) -> String {
        self.state
            .remote_control()
            .projection()
            .instance_id()
            .to_string()
    }

    /// Drive one typed event through the reducer and the facts store, the way
    /// the authoritative dispatch boundary does.
    fn dispatch(&mut self, event: ExecutionEvent) {
        self.dispatch += 1;
        self.reducer.apply_execution_event(&event);
        self.facts
            .observe(self.dispatch, &event, Some(&self.reducer), Utc::now());
    }

    /// Republish the monitoring snapshot so the projection tracks these changes.
    async fn publish(&self, change_ids: &[&str]) {
        let changes: Vec<Change> = change_ids
            .iter()
            .map(|id| Change {
                id: (*id).to_string(),
                completed_tasks: 0,
                total_tasks: 2,
                last_modified: "just now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            })
            .collect();
        self.state.update_with_mode(&changes, "select").await;
    }

    /// Admit a change the way every admission source ultimately does.
    fn admit(&mut self, change_id: &str) -> String {
        self.reducer
            .apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        self.dispatch(ExecutionEvent::OperatorCommandApplied {
            effect: OperatorCommandEffect::QueueDelta {
                change_id: change_id.to_string(),
                queued: true,
            },
        });
        self.facts
            .change(change_id)
            .execution_id
            .expect("admission opens an execution episode")
    }

    /// The process-local registry this owner serves.
    ///
    /// Reached so a test can shorten the injected shutdown deadline. The
    /// assertions that follow are about ordering and state, never about how long
    /// anything took: the deadline only makes the cancellation branch reachable
    /// inside a bounded test.
    fn registry(&self) -> Arc<conflux::web::completion_sink::CompletionSinkRegistry> {
        self.state
            .completion_sinks()
            .expect("an owner with an orchestration runtime holds a registry")
    }

    async fn stop(mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown().await;
        }
    }
}

/// One HTTP exchange over the owner's Unix socket.
async fn unix_request(
    socket: &Path,
    method: &str,
    target: &str,
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::UnixStream::connect(socket)
        .await
        .expect("connect to the owner socket");
    stream
        .write_all(encode(method, target, "localhost", body).as_bytes())
        .await
        .expect("request write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("response read");
    parse(&String::from_utf8_lossy(&raw))
}

/// One HTTP exchange over the owner's TCP listener.
async fn tcp_request(
    url: &str,
    method: &str,
    target: &str,
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let authority = url.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect to the owner TCP listener");
    stream
        .write_all(encode(method, target, authority, body).as_bytes())
        .await
        .expect("request write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("response read");
    parse(&String::from_utf8_lossy(&raw))
}

fn encode(method: &str, target: &str, host: &str, body: Option<&str>) -> String {
    let mut request =
        format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    match body {
        Some(body) => {
            request.push_str("Content-Type: application/json\r\n");
            request.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
            request.push_str(body);
        }
        None => request.push_str("\r\n"),
    }
    request
}

fn parse(raw: &str) -> (u16, serde_json::Value) {
    let status = raw
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line in response: {raw}"));
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    // Chunked framing is what axum uses for a body of unknown length; a JSON
    // object always starts at the first `{`, and the trailer follows the last.
    let json = body
        .find('{')
        .and_then(|start| body.rfind('}').map(|end| &body[start..=end]))
        .and_then(|slice| serde_json::from_str(slice).ok())
        .unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn sink_path(execution_id: &str) -> String {
    format!("/api/v2/executions/{execution_id}/sink")
}

fn binding(instance_id: &str, change_id: &str) -> String {
    format!("instance_id={instance_id}&change_id={change_id}")
}

/// A callback that records its environment and the event file it was handed.
///
/// A shell script rather than a Rust binary because the argv contract is
/// "whatever the operator names", and a script is what an operator will use.
fn recorder(dir: &Path, name: &str) -> Vec<String> {
    let script = dir.join(format!("{name}.sh"));
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         {\n\
         echo \"event_type=$CFLX_EVENT_TYPE\"\n\
         echo \"execution_id=$CFLX_EXECUTION_ID\"\n\
         echo \"change_id=$CFLX_CHANGE_ID\"\n\
         echo \"instance_id=$CFLX_INSTANCE_ID\"\n\
         echo \"event_path=$CFLX_EVENT_PATH\"\n\
         echo \"env_names=$(env | sed 's/=.*//' | sort | tr '\\n' ',')\"\n\
         echo '--- payload ---'\n\
         cat \"$CFLX_EVENT_PATH\"\n\
         echo\n\
         echo '--- end ---'\n\
         } >> \"$1\"\n",
    )
    .expect("write the recorder script");
    let mut permissions = std::fs::metadata(&script).expect("stat").permissions();
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o700);
    }
    std::fs::set_permissions(&script, permissions).expect("chmod");

    let log = dir.join(format!("{name}.log"));
    vec![
        "/bin/sh".to_string(),
        script.display().to_string(),
        log.display().to_string(),
    ]
}

fn log_of(command: &[String]) -> PathBuf {
    PathBuf::from(&command[2])
}

/// Wait for one *complete* callback record, or give up.
///
/// The sentinel is what makes this deterministic rather than timing-dependent:
/// the recorder writes it last, so its presence proves the whole record —
/// including the payload the assertions parse — is already on disk. The ceiling
/// is a hang guard, not a latency assertion.
async fn await_log(path: &Path) -> String {
    for _ in 0..400 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if text.contains("--- end ---") {
                return text;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no complete callback record appeared at {}", path.display());
}

/// Give the dispatcher a bounded chance to do something, for the assertions
/// that are about *absence*.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

// ============================================================================
// transport
// ============================================================================

/// Registering a sink stores an argv this process will execute, so the
/// owner-only socket is the only channel that may carry it. The TCP refusal is
/// its own code: the credentials were fine, the channel was not.
#[tokio::test]
async fn transport_accepts_sink_mutation_on_the_socket_and_refuses_it_on_tcp() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "cb");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();

    // TCP first, so a refusal cannot be mistaken for "it was already set".
    let (status, error) =
        tcp_request(&owner.tcp, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 403);
    assert_eq!(error["error_code"], "transport_not_permitted");

    let (status, read) = unix_request(
        &owner.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(read["sink"].is_null(), "the refused PUT stored no argv");

    // The same request over the socket is accepted.
    let (status, set) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);
    assert_eq!(set["sink"]["command"][0], "/bin/sh");
    assert_eq!(set["change_id"], "alpha");
    assert_eq!(set["terminal_dispatched"], false);

    // Clearing is a mutation too, so TCP is refused there as well.
    let (status, error) = tcp_request(
        &owner.tcp,
        "DELETE",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 403);
    assert_eq!(error["error_code"], "transport_not_permitted");

    // Reading is not a mutation, so TCP may do it — but the argv is a command
    // this process will run, and a channel that may not register one may not
    // read one back either.
    let (status, read) = tcp_request(
        &owner.tcp,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(read["sink_registered"], true);
    assert!(read["sink"].is_null(), "TCP is told presence, not argv");

    owner.stop().await;
}

// ============================================================================
// capability
// ============================================================================

/// A client must be able to discover the surface rather than infer it from a
/// refusal, and registering a sink must not look like a workflow command.
#[tokio::test]
async fn capability_is_published_and_registration_is_not_a_command() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");

    let (status, capabilities) =
        unix_request(&owner.socket, "GET", "/api/v2/capabilities", None).await;
    assert_eq!(status, 200);
    assert_eq!(capabilities["execution_sinks"]["available"], true);
    assert!(
        capabilities["execution_sinks"]["max_command_args"]
            .as_u64()
            .unwrap()
            > 0
    );

    // The execution ID is published where a client already reads execution facts.
    let (status, execution) =
        unix_request(&owner.socket, "GET", "/api/v2/execution-status", None).await;
    assert_eq!(status, 200);
    assert_eq!(execution["changes"][0]["execution_id"], execution_id);

    let (_, before) = unix_request(&owner.socket, "GET", "/api/v2/state", None).await;
    let revision_before = before["state_revision"].as_u64().expect("a revision");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": recorder(dir.path(), "cb"),
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);

    let (_, after) = unix_request(&owner.socket, "GET", "/api/v2/state", None).await;
    assert_eq!(
        after["state_revision"].as_u64(),
        Some(revision_before),
        "a subscription is not a command, so it advances no revision"
    );
    assert_eq!(
        after["snapshot"]["changes"][0]["queue_intent"],
        before["snapshot"]["changes"][0]["queue_intent"],
        "and it mutates no queue intent"
    );

    owner.stop().await;
}

/// A process that binds no orchestration runtime holds no subscriptions, and
/// says so explicitly. A client must never have to infer the surface from a
/// refusal: "this build cannot" and "that execution is gone" are different
/// facts, and only one of them is worth retrying.
#[tokio::test]
async fn capability_reports_unavailable_when_no_registry_is_bound() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("cflx-api.sock");
    // Deliberately no `set_execution_facts`, which is what a read-only process
    // that never bound an orchestration runtime looks like.
    let state = Arc::new(WebState::new(&[]));
    let handle = conflux::web::start_listeners(
        WebConfig {
            enabled: false,
            refresh_interval_secs: 0,
            ..WebConfig::default()
        },
        ListenerPlan {
            unix_socket: Some(socket.clone()),
            tcp: false,
        },
        state.clone(),
    )
    .await
    .expect("the socket binds");

    let (status, capabilities) = unix_request(&socket, "GET", "/api/v2/capabilities", None).await;
    assert_eq!(status, 200);
    assert_eq!(capabilities["execution_sinks"]["available"], false);

    let body = serde_json::json!({
        "instance_id": state.remote_control().projection().instance_id(),
        "change_id": "alpha",
        "command": ["/bin/true"],
    })
    .to_string();
    let (status, error) = unix_request(&socket, "PUT", &sink_path("whatever"), Some(&body)).await;
    assert_eq!(status, 409);
    assert_eq!(error["error_code"], "command_executor_unbound");

    handle.shutdown().await;
}

// ============================================================================
// registration
// ============================================================================

/// The binding is checked, not trusted. A sink that attached itself to whatever
/// currently holds an ID would silently follow work nobody subscribed to.
#[tokio::test]
async fn registration_validates_the_exact_instance_execution_and_change_binding() {
    let mut owner = Owner::start(&["alpha", "beta"]).await;
    let execution_id = owner.admit("alpha");
    owner.admit("beta");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "cb");

    // Wrong change.
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "beta",
        "command": command,
    })
    .to_string();
    let (status, error) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 409);
    assert_eq!(error["error_code"], "execution_binding_mismatch");

    // Wrong instance.
    let body = serde_json::json!({
        "instance_id": "not-this-owner",
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    let (status, error) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 409);
    assert_eq!(error["error_code"], "execution_binding_mismatch");

    // Unknown execution.
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    let (status, error) = unix_request(
        &owner.socket,
        "PUT",
        &sink_path("0123456789abcdef"),
        Some(&body),
    )
    .await;
    assert_eq!(status, 404);
    assert_eq!(error["error_code"], "not_found");

    owner.stop().await;
}

/// argv is data, and the bounds on it are contract.
#[tokio::test]
async fn registration_refuses_an_argv_that_is_not_a_bounded_direct_command() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();

    for command in [
        serde_json::json!([]),
        serde_json::json!(["/bin/echo", "a\nb"]),
        serde_json::json!([""]),
    ] {
        let body = serde_json::json!({
            "instance_id": instance_id,
            "change_id": "alpha",
            "command": command,
        })
        .to_string();
        let (status, error) =
            unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
        assert_eq!(status, 422, "{command} must be refused");
        assert_eq!(error["error_code"], "validation_failed");
    }

    // A shell string is not an argv, and the schema refuses it outright.
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": "echo hi | mail me",
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 422);

    // An unknown field fails closed rather than being silently ignored.
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": ["/bin/true"],
        "shell": true,
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 422);

    owner.stop().await;
}

/// A different valid argv replaces the prior sink atomically, and an identical
/// one changes nothing.
#[tokio::test]
async fn registration_replaces_a_prior_sink_and_repeats_are_idempotent() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let first = recorder(dir.path(), "first");
    let second = recorder(dir.path(), "second");

    let put = |command: &Vec<String>| {
        serde_json::json!({
            "instance_id": instance_id,
            "change_id": "alpha",
            "command": command,
        })
        .to_string()
    };

    let (status, _) = unix_request(
        &owner.socket,
        "PUT",
        &sink_path(&execution_id),
        Some(&put(&first)),
    )
    .await;
    assert_eq!(status, 200);
    let (status, repeated) = unix_request(
        &owner.socket,
        "PUT",
        &sink_path(&execution_id),
        Some(&put(&first)),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(repeated["sink"]["command"], serde_json::json!(first));
    assert_eq!(repeated["delivered_events"], serde_json::json!([]));

    let (status, replaced) = unix_request(
        &owner.socket,
        "PUT",
        &sink_path(&execution_id),
        Some(&put(&second)),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(replaced["sink"]["command"], serde_json::json!(second));

    // Only the replacement runs.
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });
    settle().await;
    assert!(
        !log_of(&first).exists(),
        "a replaced sink must never be invoked"
    );

    let (status, cleared) = unix_request(
        &owner.socket,
        "DELETE",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(cleared["sink"].is_null());

    owner.stop().await;
}

/// The race an enqueue caller is actually in: the execution settles before the
/// caller has registered anything. Registering afterwards must still deliver.
#[tokio::test]
async fn registration_after_terminal_settlement_delivers_that_event_once() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "late");

    // Settle first, with nothing registered.
    owner.dispatch(ExecutionEvent::ApplyFailed {
        change_id: "alpha".to_string(),
        error: "boom".to_string(),
    });
    settle().await;
    assert!(!log_of(&command).exists());

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);

    let record = await_log(&log_of(&command)).await;
    assert!(record.contains("event_type=failed"), "{record}");

    // Registering again must not produce a second terminal delivery: the budget
    // is one attempt per execution, however many registrations race.
    let (_, again) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(again["terminal_dispatched"], true);
    settle().await;
    assert_eq!(
        record_count(&log_of(&command), "event_type="),
        1,
        "one terminal delivery per execution"
    );

    owner.stop().await;
}

fn record_count(path: &Path, needle: &str) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .matches(needle)
        .count()
}

// ============================================================================
// delivery
// ============================================================================

/// A typed terminal failure is delivered directly; only success needs proof.
/// The environment is exactly the five documented variables, and the payload
/// carries bounded typed data rather than anything the owner happened to hold.
#[tokio::test]
async fn delivery_carries_only_the_five_variables_and_a_bounded_typed_payload() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "env");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    owner.dispatch(ExecutionEvent::ApplyFailed {
        change_id: "alpha".to_string(),
        error: "a secret-looking failure body".to_string(),
    });
    let record = await_log(&log_of(&command)).await;

    assert!(record.contains("event_type=failed"), "{record}");
    assert!(
        record.contains(&format!("execution_id={execution_id}")),
        "{record}"
    );
    assert!(record.contains("change_id=alpha"), "{record}");
    assert!(
        record.contains(&format!("instance_id={instance_id}")),
        "{record}"
    );
    // The environment is *replaced*, not extended: an owner's configured token,
    // provider credentials, and terminal settings must not reach a third-party
    // helper. `/bin/sh` sets PWD, SHLVL, and `_` for itself; nothing the owner
    // inherited survives.
    let listed = record
        .lines()
        .find_map(|line| line.strip_prefix("env_names="))
        .expect("the callback lists its environment");
    // Comma-delimited on both sides so `PATH` cannot match inside
    // `CFLX_EVENT_PATH`.
    let names = format!(",{listed}");
    for inherited in ["PATH", "HOME", "USER", "SHELL", "TMPDIR", "CARGO"] {
        assert!(
            !names.contains(&format!(",{inherited},")),
            "{inherited} must not be inherited by a callback: {names}"
        );
    }
    for provided in [
        "CFLX_EVENT_PATH",
        "CFLX_EVENT_TYPE",
        "CFLX_EXECUTION_ID",
        "CFLX_CHANGE_ID",
        "CFLX_INSTANCE_ID",
    ] {
        assert!(
            names.contains(&format!(",{provided},")),
            "{provided} must be provided: {names}"
        );
    }
    assert_eq!(
        names.matches("CFLX_").count(),
        5,
        "exactly the five documented variables: {names}"
    );

    let payload = record
        .split_once("--- payload ---")
        .and_then(|(_, rest)| rest.split_once("--- end ---"))
        .map(|(payload, _)| payload.trim().to_string())
        .expect("the callback read its event file");
    let event: serde_json::Value = serde_json::from_str(&payload).expect("a typed event file");
    assert_eq!(event["schema_version"], 1);
    assert_eq!(event["event_type"], "failed");
    assert_eq!(event["terminal"], true);
    assert_eq!(event["change_id"], "alpha");
    assert_eq!(event["terminal_mode"], "merged");
    assert!(
        event.get("evidence").is_none(),
        "only a proven completion carries evidence"
    );
    assert!(
        !payload.contains("secret-looking"),
        "an unrestricted error body must never reach the callback: {payload}"
    );

    // The callback read the file while it was running — that is what the
    // payload above proves — and the owner removes it once the child is reaped.
    // Reaping happens after the callback's own write, so the absence is polled
    // rather than asserted at the instant the log appeared.
    let path = record
        .lines()
        .find_map(|line| line.strip_prefix("event_path="))
        .expect("the path is published");
    let path = Path::new(path);
    for _ in 0..400 {
        if !path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        !path.exists(),
        "the event file must not outlive its callback"
    );

    owner.stop().await;
}

/// A claimed terminal success without repository proof is *not* a completion.
/// The owner records a diagnostic and dispatches nothing rather than making an
/// unprovable claim to a callback that would act on it.
#[tokio::test]
async fn delivery_refuses_to_report_completion_without_repository_proof() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "unproven");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    // The reducer reaches a terminal success, but the bound repository root is
    // an empty temporary directory with no archive entry to prove it.
    owner.dispatch(ExecutionEvent::MergeCompleted {
        change_id: "alpha".to_string(),
        revision: "r1".to_string(),
    });
    settle().await;
    settle().await;

    assert!(
        !log_of(&command).exists(),
        "an unproven completion must dispatch nothing at all"
    );
    let (_, read) = unix_request(
        &owner.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(read["terminal_dispatched"], false);
    assert_eq!(read["delivered_events"], serde_json::json!([]));

    owner.stop().await;
}

/// A callback that fails is an observability problem and nothing more.
#[tokio::test]
async fn delivery_failure_changes_nothing_about_the_execution() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();

    let (_, before) = unix_request(&owner.socket, "GET", "/api/v2/state", None).await;

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        // A program that does not exist: spawn itself fails.
        "command": ["/nonexistent/cflx-callback"],
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);

    owner.dispatch(ExecutionEvent::ChangeDequeued {
        change_id: "alpha".to_string(),
    });
    settle().await;

    let (_, after) = unix_request(&owner.socket, "GET", "/api/v2/state", None).await;
    assert_eq!(
        after["snapshot"]["changes"][0]["display_status"],
        before["snapshot"]["changes"][0]["display_status"],
        "a failed callback must not move the change"
    );
    let (_, commands) = unix_request(&owner.socket, "GET", "/api/v2/execution-status", None).await;
    assert_eq!(commands["changes"][0]["execution_id"], execution_id);

    owner.stop().await;
}

/// Blocked attention is opt-in and edge-triggered, and terminal events are not
/// opt-out.
#[tokio::test]
async fn delivery_of_blocked_attention_is_opt_in_and_edge_triggered() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let quiet = recorder(dir.path(), "quiet");

    // Without the opt-in, an attention edge delivers nothing.
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": quiet,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    owner.dispatch(ExecutionEvent::MergeDeferred {
        change_id: "alpha".to_string(),
        reason: "waiting on the base lane".to_string(),
        auto_resumable: true,
    });
    settle().await;
    assert!(!log_of(&quiet).exists());

    // With it, the same edge is delivered — and the terminal still follows.
    let loud = recorder(dir.path(), "loud");
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": loud,
        "notify_blocked": true,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    owner.dispatch(ExecutionEvent::ChangeDequeued {
        change_id: "alpha".to_string(),
    });
    let record = await_log(&log_of(&loud)).await;
    assert!(
        record.contains("event_type=stopped"),
        "terminal events are never opt-out: {record}"
    );

    owner.stop().await;
}

// ============================================================================
// restart
// ============================================================================

/// A new incarnation is a new registry. The old binding is gone rather than
/// silently rebound onto whatever this process is doing now.
#[tokio::test]
async fn restart_invalidates_every_prior_execution_binding() {
    let mut owner = Owner::start(&["alpha"]).await;
    owner.publish(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let first_instance = owner.instance_id();
    let socket = owner.socket.clone();
    let dir2 = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir2.path(), "cb");

    let body = serde_json::json!({
        "instance_id": first_instance,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    let (status, _) = unix_request(&socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);

    owner.stop().await;
    assert!(
        tokio::net::UnixStream::connect(&socket).await.is_err(),
        "a stopped owner gives its endpoint back"
    );

    // A replacement process. The assertion is about incarnation identity, not
    // about reusing one inode.
    let mut replacement = Owner::start(&["alpha"]).await;
    replacement.publish(&["alpha"]).await;
    assert_ne!(replacement.instance_id(), first_instance);

    let (status, error) = unix_request(
        &replacement.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&first_instance, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(
        status, 404,
        "an execution from a lost incarnation is not one this process has"
    );
    assert_eq!(error["error_code"], "not_found");

    // And the new incarnation's own admission is a different episode entirely.
    let second = replacement.admit("alpha");
    assert_ne!(second, execution_id);

    replacement.stop().await;
}

/// A graceful stop tells live registrations the owner is leaving. It is not an
/// outcome: `owner_stopping` says nothing about whether the work finished.
#[tokio::test]
async fn restart_graceful_shutdown_attempts_owner_stopping_for_live_registrations() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "stopping");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    owner.stop().await;

    let record = await_log(&log_of(&command)).await;
    assert!(record.contains("event_type=owner_stopping"), "{record}");
    let payload = record
        .split_once("--- payload ---")
        .and_then(|(_, rest)| rest.split_once("--- end ---"))
        .map(|(payload, _)| payload.trim().to_string())
        .expect("the callback read its event file");
    let event: serde_json::Value = serde_json::from_str(&payload).expect("a typed event file");
    assert_eq!(
        event["terminal"], false,
        "the owner leaving is not a terminal classification"
    );
}

// ============================================================================
// binding
// ============================================================================

/// Inspection asserts the same complete binding a registration does, and the
/// registered argv is disclosed only over the owner's own socket.
///
/// The binding is coherence, not access control — every identifier in it is
/// readable through another authenticated resource. What it buys is that a
/// caller and the owner cannot silently mean two different execution episodes.
/// The argv is the part that is genuinely privileged: it is a command this
/// process will run, and a channel that may not register one may not read one
/// back either.
#[tokio::test]
async fn binding_is_required_for_inspection_and_argv_is_disclosed_only_over_the_socket() {
    let mut owner = Owner::start(&["alpha", "beta"]).await;
    let execution_id = owner.admit("alpha");
    owner.admit("beta");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");
    let command = recorder(dir.path(), "cb");

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    let (status, registered) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);
    assert_eq!(registered["sink_registered"], true);
    assert_eq!(registered["sink"]["command"][0], "/bin/sh");

    // An omitted half of the binding is a typed validation refusal, not a
    // silently-permissive read.
    for query in [
        String::new(),
        format!("instance_id={instance_id}"),
        "change_id=alpha".to_string(),
    ] {
        let (status, error) = unix_request(
            &owner.socket,
            "GET",
            &format!("{}?{}", sink_path(&execution_id), query),
            None,
        )
        .await;
        assert_eq!(status, 422, "'{query}' is not a complete binding");
        assert_eq!(error["error_code"], "validation_failed");
        assert!(
            !error.to_string().contains("/bin/sh"),
            "a refusal discloses no argv: {error}"
        );
    }

    // A complete but wrong binding is the mismatch it always was.
    for query in [
        binding(&instance_id, "beta"),
        binding("not-this-owner", "alpha"),
    ] {
        let (status, error) = unix_request(
            &owner.socket,
            "GET",
            &format!("{}?{}", sink_path(&execution_id), query),
            None,
        )
        .await;
        assert_eq!(status, 409, "'{query}' must be refused");
        assert_eq!(error["error_code"], "execution_binding_mismatch");
        assert!(
            !error.to_string().contains("/bin/sh"),
            "a mismatch discloses no argv: {error}"
        );
    }

    // Clearing asserts the same complete binding.
    let (status, error) = unix_request(
        &owner.socket,
        "DELETE",
        &format!("{}?instance_id={instance_id}", sink_path(&execution_id)),
        None,
    )
    .await;
    assert_eq!(status, 422);
    assert_eq!(error["error_code"], "validation_failed");

    // An authenticated TCP reader learns everything it needs to decide whether
    // it still owes a resume — except the command line.
    let (status, read) = tcp_request(
        &owner.tcp,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert!(
        read["sink"].is_null(),
        "TCP must not be told the argv: {read}"
    );
    assert_eq!(
        read["sink_registered"], true,
        "but subscription presence is not the secret"
    );
    assert_eq!(read["execution_state"], "queued");
    assert_eq!(read["delivered_events"], serde_json::json!([]));
    assert_eq!(read["terminal_dispatched"], false);

    // The same read over the socket carries it.
    let (status, read) = unix_request(
        &owner.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(read["sink"]["command"][0], "/bin/sh");
    assert_eq!(read["sink_registered"], true);

    owner.stop().await;
}

// ============================================================================
// artifact
// ============================================================================

/// The event payload is created `0400` inside a `0700` owner-private directory,
/// so an ordinary callback opening it for writing is refused by the permissions.
///
/// This is *default* mutation refusal, not an integrity boundary: a callback
/// runs under the owner's own UID and can `chmod` its way past it, which this
/// test then does. What makes that harmless is the other half of the contract —
/// the owner never reads the artifact back, so rewriting it changes no delivered
/// classification.
#[tokio::test]
async fn artifact_is_owner_read_only_by_default_and_is_never_read_back() {
    // Under a privileged EUID the permission check does not apply at all, so the
    // assertion would be vacuous rather than wrong.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: default write refusal is only observable under an unprivileged EUID");
        return;
    }

    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");

    let script = dir.path().join("mutate.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         {\n\
         echo \"event_path=$CFLX_EVENT_PATH\"\n\
         echo \"perm=$(ls -l \"$CFLX_EVENT_PATH\" | cut -c1-10)\"\n\
         echo \"dir_perm=$(ls -ld \"$(dirname \"$CFLX_EVENT_PATH\")\" | cut -c1-10)\"\n\
         if : > \"$CFLX_EVENT_PATH\" 2>/dev/null; then\n\
         echo 'write=allowed'\n\
         else\n\
         echo 'write=refused'\n\
         fi\n\
         echo \"still_readable=$(head -c 1 \"$CFLX_EVENT_PATH\" >/dev/null 2>&1 && echo yes || echo no)\"\n\
         chmod u+w \"$CFLX_EVENT_PATH\" 2>/dev/null\n\
         echo 'this is not an event file' > \"$CFLX_EVENT_PATH\"\n\
         echo '--- end ---'\n\
         } >> \"$1\"\n",
    )
    .expect("write the mutating script");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script).expect("stat").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).expect("chmod");
    }
    let log = dir.path().join("mutate.log");
    let command = vec![
        "/bin/sh".to_string(),
        script.display().to_string(),
        log.display().to_string(),
    ];

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    owner.dispatch(ExecutionEvent::ApplyFailed {
        change_id: "alpha".to_string(),
        error: "boom".to_string(),
    });
    let record = await_log(&log).await;

    assert!(
        record.contains("perm=-r--------"),
        "the payload is owner-read-only: {record}"
    );
    assert!(
        record.contains("dir_perm=drwx------"),
        "and it lives in an owner-private directory: {record}"
    );
    assert!(
        record.contains("write=refused"),
        "an ordinary callback cannot open it for writing: {record}"
    );
    assert!(
        record.contains("still_readable=yes"),
        "and the payload it was handed stays readable: {record}"
    );

    // The callback then defeated the permissions and rewrote the file. Nothing
    // the owner reports about the delivery changed, because nothing the owner
    // decides reads that file.
    let (_, read) = unix_request(
        &owner.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(read["terminal_dispatched"], true);
    assert_eq!(read["delivered_events"], serde_json::json!(["failed"]));

    // And the artifact is removed once the child is reaped.
    let path = record
        .lines()
        .find_map(|line| line.strip_prefix("event_path="))
        .expect("the path is published");
    let path = PathBuf::from(path);
    for _ in 0..400 {
        if !path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        !path.exists(),
        "the event file must not outlive its callback"
    );

    owner.stop().await;
}

/// Output volume is bounded by the owner, not by the callback: both streams keep
/// draining past the retention limit, so a callback that writes far more than
/// the limit is never blocked on a full pipe, is never terminated for it, and
/// exits with its own status.
#[tokio::test]
async fn delivery_drains_a_callback_that_overflows_both_streams_without_killing_it() {
    let mut owner = Owner::start(&["alpha"]).await;
    let execution_id = owner.admit("alpha");
    let instance_id = owner.instance_id();
    let dir = tempfile::tempdir().expect("temp dir");

    // 256 KiB per stream: far past the 8 KiB retention limit and past any pipe
    // buffer, so an owner that stopped reading would deadlock the callback until
    // its runtime ceiling killed it — and the marker below would never appear.
    let script = dir.path().join("loud.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         dd if=/dev/zero bs=1024 count=256 2>/dev/null | tr '\\0' 'a'\n\
         dd if=/dev/zero bs=1024 count=256 2>/dev/null | tr '\\0' 'b' 1>&2\n\
         echo 'exited=0' >> \"$1\"\n\
         echo '--- end ---' >> \"$1\"\n\
         exit 0\n",
    )
    .expect("write the loud script");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script).expect("stat").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).expect("chmod");
    }
    let log = dir.path().join("loud.log");
    let command = vec![
        "/bin/sh".to_string(),
        script.display().to_string(),
        log.display().to_string(),
    ];

    // A short runtime ceiling turns "the owner stopped draining" into a bounded
    // failure instead of a twenty-second wait. It is a hang guard: the assertion
    // below is that the callback reached its own successful exit, not that it
    // did so quickly.
    owner
        .registry()
        .set_callback_timeout(Duration::from_secs(10));

    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": "alpha",
        "command": command,
    })
    .to_string();
    unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;

    owner.dispatch(ExecutionEvent::ChangeDequeued {
        change_id: "alpha".to_string(),
    });
    let record = await_log(&log).await;
    assert!(
        record.contains("exited=0"),
        "the callback ran to its own successful exit rather than being terminated: {record}"
    );

    // Overflow alone changed nothing about the delivery.
    let (_, read) = unix_request(
        &owner.socket,
        "GET",
        &format!(
            "{}?{}",
            sink_path(&execution_id),
            binding(&instance_id, "alpha")
        ),
        None,
    )
    .await;
    assert_eq!(read["terminal_dispatched"], true);
    assert_eq!(read["delivered_events"], serde_json::json!(["stopped"]));

    owner.stop().await;
}

// ============================================================================
// shutdown
// ============================================================================

/// A callback that records what it was handed and then holds on.
///
/// It records first, then becomes a long sleep *in place*: `exec` keeps the
/// PID, so terminating it terminates the sleeper and closes the inherited pipe,
/// rather than orphaning a grandchild.
fn holding_callback(dir: &Path, name: &str) -> Vec<String> {
    let script = dir.join(format!("{name}.sh"));
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         echo \"start=$CFLX_CHANGE_ID pid=$$ event_path=$CFLX_EVENT_PATH \
         held=$([ -f \"$CFLX_EVENT_PATH\" ] && echo yes || echo no)\" >> \"$1\"\n\
         exec sleep 30\n",
    )
    .expect("write the holding script");
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script).expect("stat").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).expect("chmod");
    }
    let log = dir.join(format!("{name}.log"));
    vec![
        "/bin/sh".to_string(),
        script.display().to_string(),
        log.display().to_string(),
    ]
}

/// Register `command` as the sink of a freshly admitted execution.
async fn register_sink(owner: &mut Owner, change_id: &str, command: &[String]) -> String {
    let instance_id = owner.instance_id();
    let execution_id = owner.admit(change_id);
    let body = serde_json::json!({
        "instance_id": instance_id,
        "change_id": change_id,
        "command": command,
    })
    .to_string();
    let (status, _) =
        unix_request(&owner.socket, "PUT", &sink_path(&execution_id), Some(&body)).await;
    assert_eq!(status, 200);
    execution_id
}

/// The `start=` record a holding callback writes before it sleeps.
///
/// Bounded because a missing record must fail rather than hang; the callback
/// writes it before `exec`, so its presence proves the callback is running.
async fn await_start(log: &Path) -> String {
    for _ in 0..400 {
        if let Some(line) = std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .find(|line| line.starts_with("start="))
        {
            return line.to_string();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("no callback start record appeared at {}", log.display());
}

/// One `name=value` field of a callback start record.
fn field<'a>(record: &'a str, name: &str) -> &'a str {
    record
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("the callback publishes {name}: {record}"))
}

/// How many callbacks have started, from the records they wrote themselves.
fn started_count(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("start="))
        .count()
}

fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Graceful shutdown is bounded by one deadline across *every* queued or running
/// callback, and the ordering around it is the contract: nothing new starts
/// after the deadline, the callback that was running is terminated and reaped,
/// and only then are the artifacts removed.
///
/// Asserted from state and ordering, never from elapsed time: the injected
/// deadline exists only to make the cancellation branch reachable.
#[tokio::test]
async fn shutdown_reaps_every_callback_before_removing_any_artifact() {
    let mut owner = Owner::start(&["alpha", "beta", "gamma"]).await;
    let dir = tempfile::tempdir().expect("temp dir");
    let command = holding_callback(dir.path(), "slow");
    let log = log_of(&command);

    // More than two live registrations, which is the case a per-callback budget
    // would have got wrong.
    for change_id in ["alpha", "beta", "gamma"] {
        register_sink(&mut owner, change_id, &command).await;
    }

    owner
        .registry()
        .set_shutdown_deadline(Duration::from_millis(300));

    // Returns only after every callback is terminated and reaped.
    owner.stop().await;

    let record = std::fs::read_to_string(&log).unwrap_or_default();
    let started: Vec<&str> = record
        .lines()
        .filter(|line| line.starts_with("start="))
        .collect();
    assert_eq!(
        started.len(),
        1,
        "delivery is serialized and no queued delivery starts after the deadline: {record}"
    );
    let started = started[0];
    assert!(
        started.contains("held=yes"),
        "the running callback held its own artifact: {started}"
    );

    // The artifact is gone, and it was removed only after the child was reaped.
    let event_path = started
        .split_whitespace()
        .find_map(|field| field.strip_prefix("event_path="))
        .expect("the callback publishes the path it was handed");
    let event_path = PathBuf::from(event_path);
    assert!(
        !event_path.exists(),
        "shutdown removes the event artifact it created"
    );
    assert!(
        !event_path
            .parent()
            .expect("the artifact lives in the owner-private directory")
            .exists(),
        "and the owner-private event directory with it"
    );

    let pid: i32 = field(started, "pid").parse().expect("a numeric pid");
    assert!(
        !alive(pid),
        "the cancelled callback was terminated before the owner returned, not left running"
    );
}

/// Cancelling is not the end of shutdown's obligations. The owner still owes a
/// confirmed reap before it removes anything, and the acknowledgement it waits
/// for has no deadline of its own: a secondary timeout that cleaned up anyway
/// would pull an event file out from under a callback that is provably still
/// being torn down.
///
/// The gate is what makes that observable as ordering rather than as latency.
/// It is held at exactly the point cancellation has reached a live callback and
/// is about to reap it, so every assertion below is about state — the child is
/// running, its artifact is on disk, shutdown has not returned, and the queued
/// deliveries never started — and none of them is about how long anything took.
#[tokio::test]
async fn shutdown_retains_artifacts_until_the_reap_is_acknowledged() {
    let mut owner = Owner::start(&["alpha", "beta", "gamma"]).await;
    let dir = tempfile::tempdir().expect("temp dir");
    let command = holding_callback(dir.path(), "held");
    let log = log_of(&command);
    // Queued deliveries behind the one that is cancelled, so "nothing starts
    // after cancellation" has something to be true of.
    for change_id in ["alpha", "beta", "gamma"] {
        register_sink(&mut owner, change_id, &command).await;
    }

    let registry = owner.registry();
    // Only so the cancellation branch is reachable inside a bounded test; no
    // assertion below depends on it.
    registry.set_shutdown_deadline(Duration::from_millis(200));
    let gate = conflux::web::completion_sink::ReapGate::new();
    registry.set_reap_gate(gate.clone());

    let stopping = tokio::spawn({
        let registry = registry.clone();
        async move { registry.owner_stopping().await }
    });

    // Arrival at the gate *is* the state transition under test: cancellation
    // has fired and the callback it cancelled has not been reaped yet.
    tokio::time::timeout(Duration::from_secs(30), gate.reached())
        .await
        .expect("shutdown cancellation reaches the callback it must reap");

    let started = await_start(&log).await;
    let pid: i32 = field(&started, "pid").parse().expect("a numeric pid");
    let event_path = PathBuf::from(field(&started, "event_path"));
    let event_dir = event_path
        .parent()
        .expect("the artifact lives in the owner-private directory")
        .to_path_buf();

    assert!(
        alive(pid),
        "the callback is still being torn down while its reap is unacknowledged"
    );
    assert!(
        event_path.exists(),
        "so its event artifact is still there to be taken away: {}",
        event_path.display()
    );
    assert!(
        event_dir.exists(),
        "and so is the owner-private directory holding it"
    );
    assert!(
        !stopping.is_finished(),
        "shutdown has not returned, because the reap it requires is outstanding"
    );
    assert_eq!(
        started_count(&log),
        1,
        "delivery is serialized, and cancellation started none of the queued ones"
    );

    gate.release();
    stopping.await.expect("graceful shutdown completes");

    assert_eq!(
        started_count(&log),
        1,
        "and none started while the reap was being acknowledged either"
    );
    assert!(
        !alive(pid),
        "the acknowledged reap is what shutdown waited for, and it terminated the callback"
    );
    assert!(
        !event_path.exists(),
        "cleanup then removes the event artifact"
    );
    assert!(
        !event_dir.exists(),
        "and the owner-private event directory with it"
    );

    owner.stop().await;
}

/// The same ordering, held long enough that any secondary acknowledgement
/// timeout of the kind this change removed would have expired and cleaned up
/// behind the owner's back.
///
/// Heavy, and the one observation here that cannot be made cheaply: proving a
/// fallback deadline is *absent* means outlasting it. It is still an absence
/// check, not a latency assertion — nothing is required to happen within the
/// hold, only that nothing does.
#[cfg(feature = "heavy-tests")]
#[tokio::test]
async fn shutdown_never_removes_artifacts_on_a_secondary_timeout() {
    /// Comfortably past the 10s grace the removed fallback used.
    const BEYOND_ANY_REMOVED_GRACE: Duration = Duration::from_secs(15);

    let mut owner = Owner::start(&["alpha"]).await;
    let dir = tempfile::tempdir().expect("temp dir");
    let command = holding_callback(dir.path(), "outlast");
    let log = log_of(&command);
    register_sink(&mut owner, "alpha", &command).await;

    let registry = owner.registry();
    registry.set_shutdown_deadline(Duration::from_millis(200));
    // The callback must outlive the hold, so it may not hit its own ceiling.
    registry.set_callback_timeout(Duration::from_secs(120));
    let gate = conflux::web::completion_sink::ReapGate::new();
    registry.set_reap_gate(gate.clone());

    let stopping = tokio::spawn({
        let registry = registry.clone();
        async move { registry.owner_stopping().await }
    });
    tokio::time::timeout(Duration::from_secs(30), gate.reached())
        .await
        .expect("shutdown cancellation reaches the callback it must reap");

    let started = await_start(&log).await;
    let event_path = PathBuf::from(field(&started, "event_path"));
    let event_dir = event_path
        .parent()
        .expect("the artifact lives in the owner-private directory")
        .to_path_buf();

    tokio::time::sleep(BEYOND_ANY_REMOVED_GRACE).await;

    assert!(
        event_path.exists(),
        "no elapsed time authorizes removing a live callback's artifact: {}",
        event_path.display()
    );
    assert!(
        event_dir.exists(),
        "nor the owner-private directory holding it"
    );
    assert!(
        !stopping.is_finished(),
        "and shutdown is still waiting for the reap acknowledgement"
    );

    gate.release();
    stopping.await.expect("graceful shutdown completes");
    assert!(
        !event_dir.exists(),
        "cleanup runs once the reap is confirmed"
    );

    owner.stop().await;
}
