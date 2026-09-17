//! Native Windows integration tests for the local `/api/v2` Named Pipe.
//!
//! Everything here drives the **production path**: `start_listeners` creates the
//! real secured pipe through `NamedPipeAcceptor`, and the requests come from the
//! real `LocalApiClient` the `cflx client` namespace uses. A test that spoke to
//! the router in-process, or wrote its own pipe client, would be asserting its
//! own fixture rather than the endpoint an operator actually gets.
//!
//! These tests only run on a Windows host. `cfg`-skipping them is indistinguishable
//! from passing them, which is why the change's evidence contract requires a
//! committed transcript with a non-zero executed-test count.
//!
//! Groups match the change's tasks: `identity`, `listener`, `transport`,
//! `shutdown`, `client`.

#![cfg(all(windows, feature = "web-monitoring"))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use conflux::client::transport::{LocalApiClient, LocalEndpoint, TransportError};
use conflux::openspec::{Change, ProposalMetadata};
use conflux::orchestration::execution_facts::ExecutionFactsStore;
use conflux::web::named_pipe::{
    current_user_sid, default_pipe_name, handle_owner_sid, pipe_endpoint, pipe_name_for,
    OwnerOnlySecurity, PIPE_NAME_PREFIX,
};
use conflux::web::remote_control_api::dto::{OwnerExecutionContract, TerminalMode};
use conflux::web::{ListenerPlan, ServerHandle, WebConfig, WebState};

// ============================================================================
// Harness
// ============================================================================

/// A pipe name no other test — and no other run of this suite — can collide on.
///
/// The pipe namespace is machine-global, so a fixed test name would make two
/// concurrent `cargo test` invocations fail each other's first-instance
/// acquisition for reasons that have nothing to do with the contract.
fn unique_pipe_name(tag: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{PIPE_NAME_PREFIX}test-{}-{}-{tag}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// One owner: the production listener on a real pipe, with a real sink registry.
struct Owner {
    pipe: String,
    tcp: Option<String>,
    handle: Option<ServerHandle>,
    _dir: tempfile::TempDir,
}

impl Owner {
    async fn start(pipe: &str, change_ids: &[&str]) -> Self {
        Self::start_with_tcp(pipe, change_ids, false).await
    }

    async fn start_with_tcp(pipe: &str, change_ids: &[&str], tcp: bool) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
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
        // The same call the TUI makes, which is what binds the proposal
        // subscription registry the transport policy guards.
        state
            .set_execution_facts(Arc::new(ExecutionFactsStore::new()))
            .await;
        state.set_repo_root(dir.path().to_path_buf()).await;
        state.set_execution_contract(OwnerExecutionContract {
            base_branch: "main".to_string(),
            terminal_mode: TerminalMode::Merged,
            remote: None,
            pushed_branch: None,
        });
        state
            .seed_workspace_observation_for_tests(&changes, "select")
            .await;

        let config = WebConfig {
            enabled: tcp,
            port: 0,
            bind: "127.0.0.1".to_string(),
            refresh_interval_secs: 0,
            ..WebConfig::default()
        };
        let handle = conflux::web::start_listeners(
            config,
            ListenerPlan {
                named_pipe: Some(pipe.to_string()),
                tcp,
            },
            state,
        )
        .await
        .expect("the Named Pipe listener must bind");
        let tcp = handle.tcp_url().map(str::to_string);

        Self {
            pipe: pipe.to_string(),
            tcp,
            handle: Some(handle),
            _dir: dir,
        }
    }

    /// The production client, bound to this owner's pipe.
    fn client(&self) -> LocalApiClient {
        LocalApiClient::new(LocalEndpoint::NamedPipe(self.pipe.clone()), None)
            .expect("no token is a valid credential source")
    }

    async fn shutdown(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.shutdown().await;
        }
    }
}

/// A bare HTTP/1.1 exchange over TCP, for the transport-policy comparison.
async fn tcp_request(url: &str, method: &str, target: &str, body: Option<&str>) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let authority = url.trim_start_matches("http://");
    let mut stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect to the TCP listener");
    let mut request =
        format!("{method} {target} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    match body {
        Some(body) => {
            request.push_str("Content-Type: application/json\r\n");
            request.push_str(&format!("Content-Length: {}\r\n\r\n", body.len()));
            request.push_str(body);
        }
        None => request.push_str("\r\n"),
    }
    stream
        .write_all(request.as_bytes())
        .await
        .expect("request write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("response read");
    let raw = String::from_utf8_lossy(&raw).to_string();
    let status = raw
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status line in {raw}"));
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    (status, body)
}

fn json(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap_or_else(|error| {
        panic!(
            "body must be JSON ({error}): {}",
            String::from_utf8_lossy(body)
        )
    })
}

// ============================================================================
// identity
// ============================================================================

/// The endpoint a client derives has to be the one the owner bound, on a real
/// Windows filesystem with real canonicalization — which is exactly what a
/// cross-platform unit test of the digest cannot show.
#[test]
fn identity_a_repository_derives_one_name_for_this_user() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).expect("create .git");

    let derived = default_pipe_name(&git_dir).expect("a SID and a path derive a name");
    assert!(derived.starts_with(PIPE_NAME_PREFIX), "{derived}");
    assert_eq!(
        derived,
        default_pipe_name(&git_dir).expect("derivation is deterministic")
    );
    assert_eq!(
        derived,
        pipe_name_for(
            &current_user_sid().expect("this process has a user SID"),
            &git_dir
        ),
        "the owner and the client must derive the name the same way"
    );

    // Neither digest input may be recoverable from a machine-global name.
    let sid = current_user_sid().expect("user SID");
    assert!(!derived.contains(&sid), "{derived}");
    assert!(
        !derived.to_lowercase().contains(
            &git_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap()
                .to_lowercase()
        ) || derived.ends_with(".git"),
        "{derived}"
    );
}

#[test]
fn identity_distinct_repositories_derive_distinct_names() {
    let one = tempfile::tempdir().expect("temp dir");
    let two = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(one.path().join(".git")).unwrap();
    std::fs::create_dir_all(two.path().join(".git")).unwrap();

    assert_ne!(
        default_pipe_name(&one.path().join(".git")).unwrap(),
        default_pipe_name(&two.path().join(".git")).unwrap(),
        "two repositories must not share one owner endpoint"
    );
}

/// Two local users must not collide, and the only thing that separates them is
/// the SID in the digest.
#[test]
fn identity_a_different_user_derives_a_different_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();

    let mine = current_user_sid().expect("user SID");
    let theirs = format!("{mine}0");
    assert_ne!(
        pipe_name_for(&mine, &git_dir),
        pipe_name_for(&theirs, &git_dir)
    );
}

/// A linked worktree is the case the digest exists to get right: it has its own
/// `.git` *file*, and it must still reach the main repository's owner.
#[test]
fn identity_a_linked_worktree_reaches_the_repository_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("canonical root");
    let main_git = root.join(".git");
    std::fs::create_dir_all(main_git.join("worktrees").join("wt")).unwrap();
    // Git's own linked-worktree layout: a `.git` file pointing at the
    // administrative directory, and a `commondir` pointing back.
    let worktree = root.join("wt");
    std::fs::create_dir_all(&worktree).unwrap();
    std::fs::write(
        worktree.join(".git"),
        format!(
            "gitdir: {}\n",
            main_git.join("worktrees").join("wt").display()
        ),
    )
    .unwrap();
    std::fs::write(
        main_git.join("worktrees").join("wt").join("commondir"),
        "../..\n",
    )
    .unwrap();

    let from_main = conflux::repo_lock::discover_common_dir(&root).expect("main common dir");
    let from_linked =
        conflux::repo_lock::discover_common_dir(&worktree).expect("linked common dir");
    assert_eq!(
        default_pipe_name(&from_main).unwrap(),
        default_pipe_name(&from_linked).unwrap(),
        "every worktree of one repository must derive one endpoint"
    );
}

// ============================================================================
// listener
// ============================================================================

#[tokio::test]
async fn listener_serves_the_shared_api_without_a_web_flag() {
    let pipe = unique_pipe_name("serves");
    let mut owner = Owner::start(&pipe, &["alpha"]).await;

    assert_eq!(
        owner.handle.as_ref().unwrap().endpoints(),
        [pipe_endpoint(&pipe)],
        "only the bound pipe endpoint may be published"
    );
    assert_eq!(
        owner.handle.as_ref().unwrap().tcp_url(),
        None,
        "no TCP listener was requested or inferred"
    );

    let client = owner.client();
    let health = client.get("/api/v2/health").await.expect("health answers");
    assert_eq!(health.status, 200);

    // Token-free local access reaches protected resources, exactly as it does
    // over a mode-0600 Unix socket: the DACL is the whole access story.
    let instance = client
        .get("/api/v2/instance")
        .await
        .expect("instance answers");
    assert_eq!(instance.status, 200);
    assert!(json(&instance.body)["instance_id"].is_string());

    let state = client.get("/api/v2/state").await.expect("state answers");
    assert_eq!(state.status, 200);
    assert_eq!(
        json(&state.body)["snapshot"]["changes"][0]["id"],
        "alpha",
        "the pipe must serve the same projection every listener serves"
    );

    owner.shutdown().await;
}

/// Several requests in a row prove the accept loop really replaces its instance
/// rather than serving one connection and going quiet.
#[tokio::test]
async fn listener_keeps_answering_after_each_connection() {
    let pipe = unique_pipe_name("sequential");
    let mut owner = Owner::start(&pipe, &["alpha"]).await;
    let client = owner.client();

    for attempt in 0..5 {
        let response = client
            .get("/api/v2/health")
            .await
            .unwrap_or_else(|error| panic!("request {attempt} failed: {error}"));
        assert_eq!(response.status, 200, "request {attempt}");
    }

    owner.shutdown().await;
}

#[tokio::test]
async fn listener_refuses_a_second_owner_of_one_endpoint() {
    let pipe = unique_pipe_name("collision");
    let mut owner = Owner::start(&pipe, &[]).await;

    let state = Arc::new(WebState::new(&[]));
    let error = conflux::web::start_listeners(
        WebConfig::default(),
        ListenerPlan {
            named_pipe: Some(pipe.clone()),
            tcp: false,
        },
        state,
    )
    .await
    .expect_err("first-instance ownership is exclusive")
    .to_string();
    assert!(error.contains("first instance"), "error={error}");

    // The live endpoint is untouched by the refused start.
    assert_eq!(
        owner
            .client()
            .get("/api/v2/health")
            .await
            .expect("the live owner keeps serving")
            .status,
        200
    );

    owner.shutdown().await;
}

// ============================================================================
// transport
// ============================================================================

/// The Named Pipe is the second owner-only local transport, so the argv-bearing
/// resources must treat it exactly as they treat the Unix socket — and TCP must
/// stay refused on the same owner, in the same process.
#[tokio::test]
async fn transport_named_pipe_may_register_callback_argv_and_tcp_may_not() {
    let pipe = unique_pipe_name("subscription");
    let mut owner = Owner::start_with_tcp(&pipe, &["alpha"], true).await;
    let client = owner.client();
    let instance = json(
        &client
            .get("/api/v2/instance")
            .await
            .expect("instance answers")
            .body,
    )["instance_id"]
        .as_str()
        .expect("instance_id")
        .to_string();

    let request = serde_json::json!({
        "instance_id": instance,
        "command": ["C:\\Windows\\System32\\cmd.exe", "/c", "exit"],
        "notify_blocked": false,
    })
    .to_string();

    // Over the pipe: accepted, and the registered argv is echoed back.
    let set = client
        .put_json("/api/v2/proposals/alpha/subscription", &request)
        .await
        .expect("the pipe accepts a subscription");
    assert_eq!(set.status, 200, "{}", String::from_utf8_lossy(&set.body));
    let body = json(&set.body);
    assert_eq!(body["subscribed"], true);
    assert_eq!(body["sink"]["command"][0], "C:\\Windows\\System32\\cmd.exe");

    // Over TCP on the same owner: refused, with nothing disclosed.
    let tcp = owner.tcp.clone().expect("TCP URL");
    let (status, refusal) = tcp_request(
        &tcp,
        "PUT",
        "/api/v2/proposals/alpha/subscription",
        Some(&request),
    )
    .await;
    assert_eq!(status, 403, "{refusal}");
    assert!(
        refusal.contains("transport_not_permitted"),
        "refusal={refusal}"
    );

    // ...and a TCP read reports presence without the argv.
    let (status, disclosed) = tcp_request(
        &tcp,
        "GET",
        &format!("/api/v2/proposals/alpha/subscription?instance_id={instance}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{disclosed}");
    let disclosed: serde_json::Value = serde_json::from_str(&disclosed).expect("JSON");
    assert_eq!(disclosed["subscribed"], true);
    assert!(disclosed["sink"].is_null(), "TCP must not see the argv");

    // The pipe, which may register one, may read one back.
    let read = client
        .get(&format!(
            "/api/v2/proposals/alpha/subscription?instance_id={instance}"
        ))
        .await
        .expect("the pipe reads its own subscription");
    assert_eq!(
        json(&read.body)["sink"]["command"][0],
        "C:\\Windows\\System32\\cmd.exe"
    );

    owner.shutdown().await;
}

/// The client checks the connected handle's owner before writing a byte, so a
/// successful exchange is itself proof the check ran against a real handle.
#[tokio::test]
async fn transport_the_connected_pipe_owner_is_this_process_user() {
    let pipe = unique_pipe_name("owner");
    let mut owner = Owner::start(&pipe, &[]).await;

    let security = OwnerOnlySecurity::for_current_user().expect("descriptor");
    let instance = security
        .create_instance(&unique_pipe_name("owner-probe"), true)
        .expect("a probe instance");
    assert_eq!(
        handle_owner_sid(&instance).expect("owner SID"),
        current_user_sid().expect("user SID"),
        "an instance this process created must be owned by this process's user"
    );

    assert_eq!(
        owner
            .client()
            .get("/api/v2/health")
            .await
            .expect("the owner check passed against a real handle")
            .status,
        200
    );

    owner.shutdown().await;
}

/// A pipe this user may not open is a typed transport refusal — never
/// `owner_not_running`, never a retry, and never a fallback to another endpoint.
#[tokio::test]
async fn transport_a_foreign_endpoint_is_refused_without_sending_a_request() {
    let sid = current_user_sid().expect("user SID");
    let name = unique_pipe_name("foreign");
    // Created through the production path, with a protected DACL that denies
    // this user everything — the reachable stand-in for "another principal owns
    // this name" on a single-account builder.
    let hostile =
        OwnerOnlySecurity::from_sddl(&format!("D:P(D;;GA;;;{sid})")).expect("a deny descriptor");
    let _instance = hostile
        .create_instance(&name, true)
        .expect("the hostile endpoint is created");

    let client = LocalApiClient::new(LocalEndpoint::NamedPipe(name.clone()), None).expect("client");
    let error = client
        .get("/api/v2/health")
        .await
        .expect_err("an endpoint this user may not open must be refused");
    assert!(
        matches!(error, TransportError::UntrustedPeer(_)),
        "a foreign endpoint must not read as an absent owner: {error:?}"
    );
    assert!(
        !error.to_string().contains("no Conflux owner is listening"),
        "{error}"
    );
}

// ============================================================================
// shutdown
// ============================================================================

#[tokio::test]
async fn shutdown_stops_accepting_and_gives_the_name_back() {
    let pipe = unique_pipe_name("shutdown");
    let mut owner = Owner::start(&pipe, &[]).await;
    assert_eq!(
        owner.client().get("/api/v2/health").await.unwrap().status,
        200
    );

    owner.shutdown().await;

    let error = owner
        .client()
        .get("/api/v2/health")
        .await
        .expect_err("the listener must be gone");
    assert!(
        matches!(error, TransportError::NotListening { .. }),
        "{error:?}"
    );

    // A later owner acquires the first instance cleanly, which is what proves
    // the pending unconnected instance was released rather than leaked.
    let mut replacement = Owner::start(&pipe, &[]).await;
    assert_eq!(
        replacement
            .client()
            .get("/api/v2/health")
            .await
            .unwrap()
            .status,
        200
    );
    replacement.shutdown().await;
}

/// Startup stays all-or-nothing on Windows: a TCP failure after the pipe was
/// acquired must release the pipe too, or the next owner could never start.
#[tokio::test]
async fn shutdown_a_failed_tcp_bind_releases_the_pipe_it_already_acquired() {
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = occupied.local_addr().unwrap().port();
    let pipe = unique_pipe_name("rollback");

    let error = conflux::web::start_listeners(
        WebConfig::enabled(port, "127.0.0.1".to_string()),
        ListenerPlan {
            named_pipe: Some(pipe.clone()),
            tcp: true,
        },
        Arc::new(WebState::new(&[])),
    )
    .await
    .expect_err("the occupied port must fail startup");
    assert!(!error.to_string().is_empty());

    let mut retry = Owner::start(&pipe, &[]).await;
    assert_eq!(
        retry.client().get("/api/v2/health").await.unwrap().status,
        200,
        "the rolled-back attempt must not keep the endpoint"
    );
    retry.shutdown().await;
}

// ============================================================================
// client
// ============================================================================

/// The namespace is available on Windows: an absent owner is reported as the
/// ordinary `owner_not_running` outcome, not as `feature_unavailable`.
#[test]
fn client_status_reaches_the_repository_route_rather_than_refusing_the_platform() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("canonical root");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    // A minimal repository is enough for endpoint derivation; `wait` is the only
    // verb that needs the Git binary, and this asserts routing, not completion.
    std::fs::write(root.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "status", "--json"])
        .current_dir(&root)
        .output()
        .expect("the compiled cflx binary must be runnable");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8 JSON");
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|error| panic!("{error}: {stdout}"));

    assert_eq!(envelope["operation"], "status");
    assert_ne!(
        envelope["outcome"], "feature_unavailable",
        "a Windows build with web-monitoring speaks the local API: {stdout}"
    );
    assert_eq!(envelope["outcome"], "owner_not_running", "{stdout}");
}

/// `--unix-socket` is a Unix-only low-level override, so a Windows build refuses
/// it at parse time rather than reinterpreting it as some other endpoint.
#[test]
fn client_the_unix_only_override_is_absent_from_this_platform() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args([
            "client",
            "--unix-socket",
            r"C:\tmp\cflx-api.sock",
            "status",
            "--json",
        ])
        .output()
        .expect("the compiled cflx binary must be runnable");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let envelope: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|error| panic!("{error}: {stdout}"));

    assert_eq!(envelope["outcome"], "usage_error", "{stdout}");
    assert_eq!(output.status.code(), Some(2), "{stdout}");
}

/// The MCP server starts on Windows; it is not the platform refusal it was.
#[test]
fn client_mcp_is_available_on_this_platform() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("canonical root");
    std::fs::create_dir_all(root.join(".git")).unwrap();

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_cflx"))
        .args(["client", "mcp"])
        .current_dir(&root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the MCP server must start");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"t","version":"0"}}}}}}"#
        )
        .expect("write initialize");
        stdin.flush().expect("flush");
    }
    // Closing stdin ends the session, so the server exits on its own.
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("the MCP server must exit");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");

    assert_ne!(
        output.status.code(),
        Some(20),
        "the platform refusal exit status must be gone: {stdout}"
    );
    assert!(
        stdout.contains("\"protocolVersion\""),
        "stdout must carry the initialize result: {stdout}"
    );
}

/// The published tool schema names only the selectors this platform has.
#[test]
fn client_mcp_publishes_no_unix_only_selector() {
    let descriptors = conflux::client::mcp::tool_descriptors().to_string();
    assert!(descriptors.contains("project_dir"), "{descriptors}");
    assert!(
        !descriptors.contains("unix_socket"),
        "a Windows build must not advertise the Unix-only override: {descriptors}"
    );
}
