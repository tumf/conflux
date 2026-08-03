//! Transport-level tests for the local API's listeners.
//!
//! These drive real connections — a `tokio::net::UnixStream` and a real TCP
//! socket — rather than calling the router in-process. That is the point: the
//! properties under test are the ones only a real listener can have, namely that
//! the socket exists with the right permissions, that both transports reach one
//! shared `WebState`, that authentication is applied per listener, and that
//! shutdown gives the path back.

use std::path::Path;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::unix_socket;
use super::{ListenerPlan, ServerHandle, WebConfig, WebState};

/// Minimal HTTP/1.1 request. `Connection: close` makes the server end the
/// response so a plain read-to-end terminates instead of waiting on keep-alive.
fn request_bytes(method: &str, target: &str, host: &str, token: Option<&str>) -> Vec<u8> {
    let mut request =
        format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    request.into_bytes()
}

/// Split a raw HTTP response into its status code and body.
fn parse_response(raw: &str) -> (u16, String) {
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
    (status, body)
}

async fn read_response<S>(mut stream: S, request: Vec<u8>) -> (u16, String)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    stream.write_all(&request).await.expect("request write");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("response read");
    parse_response(&String::from_utf8_lossy(&raw))
}

/// Issue a real HTTP request through the Unix socket.
async fn unix_get(path: &Path, target: &str, token: Option<&str>) -> (u16, String) {
    let stream = tokio::net::UnixStream::connect(path)
        .await
        .expect("connect to the API socket");
    read_response(stream, request_bytes("GET", target, "localhost", token)).await
}

/// True when the TCP listener still answers a health probe.
///
/// Deliberately non-panicking: after shutdown every step of the exchange is
/// allowed to fail, and each failure means the same thing.
async fn tcp_serves(url: &str) -> bool {
    let authority = url.trim_start_matches("http://");
    let Ok(mut stream) = tokio::net::TcpStream::connect(authority).await else {
        return false;
    };
    let request = request_bytes("GET", "/api/v2/health", authority, None);
    if stream.write_all(&request).await.is_err() {
        return false;
    }
    let mut raw = Vec::new();
    if stream.read_to_end(&mut raw).await.is_err() {
        return false;
    }
    String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 200")
}

/// Issue a real HTTP request through the TCP listener.
async fn tcp_get(url: &str, target: &str, token: Option<&str>) -> (u16, String) {
    let authority = url.trim_start_matches("http://");
    let stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("connect to the TCP listener");
    read_response(stream, request_bytes("GET", target, authority, token)).await
}

fn state() -> Arc<WebState> {
    Arc::new(WebState::new(&[]))
}

async fn start(
    config: WebConfig,
    unix_path: Option<&Path>,
    tcp: bool,
) -> Result<ServerHandle, String> {
    let plan = ListenerPlan {
        unix_socket: unix_path.map(Path::to_path_buf),
        tcp,
    };
    super::start_listeners(config, plan, state())
        .await
        .map_err(|e| e.to_string())
}

fn instance_id(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .expect("JSON body")
        .get("instance_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("no instance_id in {body}"))
        .to_string()
}

#[tokio::test]
async fn the_unix_listener_serves_real_http_without_a_web_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let handle = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect("UDS-only startup");

    assert_eq!(
        handle.endpoints(),
        [unix_socket::unix_endpoint(&socket)],
        "only the bound Unix endpoint may be published"
    );
    assert_eq!(handle.tcp_url(), None, "no TCP listener was requested");

    let (status, body) = unix_get(&socket, "/api/v2/health", None).await;
    assert_eq!(status, 200, "body={body}");
    assert!(body.contains("\"status\""), "body={body}");

    handle.shutdown().await;
}

/// Filesystem permissions are the whole access story for a token-free socket,
/// so a local client reaches protected resources without credentials.
#[tokio::test]
async fn token_free_unix_access_reaches_protected_resources() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let handle = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect("UDS-only startup");

    let (status, body) = unix_get(&socket, "/api/v2/instance", None).await;
    assert_eq!(status, 200, "body={body}");

    let mode = std::os::unix::fs::PermissionsExt::mode(
        &std::fs::symlink_metadata(&socket).unwrap().permissions(),
    ) & 0o777;
    assert_eq!(mode, 0o600, "socket mode must be 0600, got {mode:o}");

    handle.shutdown().await;
}

/// `--web` adds TCP to the same process; it never replaces the Unix listener,
/// and both listeners must reach one `WebState` rather than two projections.
#[tokio::test]
async fn both_transports_serve_the_same_instance_and_state() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let handle = start(
        WebConfig::enabled(0, "127.0.0.1".to_string()),
        Some(&socket),
        true,
    )
    .await
    .expect("dual startup");

    let url = handle.tcp_url().expect("TCP URL").to_string();
    assert_eq!(
        handle.endpoints(),
        [unix_socket::unix_endpoint(&socket), url.clone()],
        "both bound endpoints must be published"
    );
    assert!(
        !url.contains(":0"),
        "the published URL must carry the actual OS-assigned port, got {url}"
    );

    let (unix_status, unix_body) = unix_get(&socket, "/api/v2/instance", None).await;
    let (tcp_status, tcp_body) = tcp_get(&url, "/api/v2/instance", None).await;
    assert_eq!(unix_status, 200, "body={unix_body}");
    assert_eq!(tcp_status, 200, "body={tcp_body}");
    assert_eq!(
        instance_id(&unix_body),
        instance_id(&tcp_body),
        "one process must present one instance over both transports"
    );

    // The console assets stay on the browser-facing transport.
    let (index_status, _) = tcp_get(&url, "/", None).await;
    assert_eq!(index_status, 200);

    handle.shutdown().await;
}

/// One authentication policy covers every active listener: a token configured
/// for the process protects UDS exactly as it protects TCP.
#[tokio::test]
async fn a_configured_token_protects_both_transports_except_health() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let config = WebConfig::enabled(0, "127.0.0.1".to_string()).with_auth(
        Some("secret-token".to_string()),
        None,
        Vec::new(),
    );
    let handle = start(config, Some(&socket), true)
        .await
        .expect("dual startup");
    let url = handle.tcp_url().expect("TCP URL").to_string();

    // Health stays public on both transports so a probe never needs a secret.
    assert_eq!(unix_get(&socket, "/api/v2/health", None).await.0, 200);
    assert_eq!(tcp_get(&url, "/api/v2/health", None).await.0, 200);

    // Every other resource is refused without credentials on both transports.
    for target in ["/api/v2/instance", "/api/v2/state", "/api/v2/changes"] {
        assert_eq!(
            unix_get(&socket, target, None).await.0,
            401,
            "unauthenticated UDS {target} must be refused"
        );
        assert_eq!(
            tcp_get(&url, target, None).await.0,
            401,
            "unauthenticated TCP {target} must be refused"
        );
    }

    // ...and accepted with them.
    let (unix_status, unix_body) =
        unix_get(&socket, "/api/v2/instance", Some("secret-token")).await;
    let (tcp_status, tcp_body) = tcp_get(&url, "/api/v2/instance", Some("secret-token")).await;
    assert_eq!(unix_status, 200, "body={unix_body}");
    assert_eq!(tcp_status, 200, "body={tcp_body}");

    // A wrong token is not better than no token.
    assert_eq!(
        unix_get(&socket, "/api/v2/instance", Some("wrong")).await.0,
        401
    );

    handle.shutdown().await;
}

#[tokio::test]
async fn shutdown_stops_the_listeners_and_removes_the_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let handle = start(
        WebConfig::enabled(0, "127.0.0.1".to_string()),
        Some(&socket),
        true,
    )
    .await
    .expect("dual startup");
    let url = handle.tcp_url().expect("TCP URL").to_string();
    assert_eq!(unix_get(&socket, "/api/v2/health", None).await.0, 200);

    handle.shutdown().await;

    assert!(
        !socket.exists(),
        "shutdown must remove the socket it created, without another signal"
    );
    assert!(
        tokio::net::UnixStream::connect(&socket).await.is_err(),
        "the Unix listener must be gone"
    );
    assert!(!tcp_serves(&url).await, "the TCP listener must be gone");
}

/// Shutdown must never delete a path that stopped being ours during the run.
#[tokio::test]
async fn shutdown_preserves_a_replacement_socket() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let handle = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect("UDS-only startup");

    // Somebody unlinks our socket and puts their own listener at the path.
    std::fs::remove_file(&socket).unwrap();
    let (_replacement, replacement_guard) = unix_socket::bind_unix_listener(&socket)
        .await
        .expect("replacement binds");

    handle.shutdown().await;
    assert!(
        replacement_guard.still_owns_path(),
        "the replacement endpoint must survive our shutdown"
    );
}

/// The startup transaction is all-or-nothing: a TCP failure after the socket
/// bound must leave no socket behind for a client to find, and no listener task
/// still running behind it.
///
/// Removing the pathname alone is not rollback — an orphaned `axum::serve` task
/// would keep serving the socket it already bound, and keep the process alive,
/// while the caller believes nothing started. The live task count is the direct
/// evidence for that, so it is asserted rather than inferred from the filesystem.
#[tokio::test]
async fn a_failed_tcp_bind_publishes_nothing_and_stops_the_unix_listener() {
    let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = occupied.local_addr().unwrap().port();

    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");

    let before = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();
    let error = start(
        WebConfig::enabled(port, "127.0.0.1".to_string()),
        Some(&socket),
        true,
    )
    .await
    .expect_err("the occupied port must fail startup");
    assert!(!error.is_empty());

    assert_eq!(
        tokio::runtime::Handle::current()
            .metrics()
            .num_alive_tasks(),
        before,
        "the Unix listener task spawned by the failed transaction must be \
         cancelled and awaited before the error is returned"
    );
    assert!(
        !socket.exists(),
        "a failed startup transaction must not leave its socket behind"
    );

    // Nothing owns the path any more, so the next attempt gets a clean start.
    let retry = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect("the path must be reusable after the rolled-back attempt");
    assert_eq!(unix_get(&socket, "/api/v2/health", None).await.0, 200);
    retry.shutdown().await;
}

/// An occupied socket path fails startup before any listener exists, and the
/// live endpoint is left untouched.
#[tokio::test]
async fn a_live_socket_at_the_target_path_fails_startup() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let owner = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect("first owner starts");

    let error = start(WebConfig::default(), Some(&socket), false)
        .await
        .expect_err("a second owner must be refused");
    assert!(
        error.contains("another process is listening"),
        "error={error}"
    );
    assert_eq!(
        unix_get(&socket, "/api/v2/health", None).await.0,
        200,
        "the live endpoint must keep serving"
    );

    owner.shutdown().await;
}

/// An unsafe TCP configuration is rejected before any listener binds, so the
/// Unix socket is never created for a process that cannot start.
#[tokio::test]
async fn an_unsafe_tcp_configuration_binds_nothing_at_all() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("cflx-api.sock");
    let error = start(
        WebConfig::enabled(0, "0.0.0.0".to_string()),
        Some(&socket),
        true,
    )
    .await
    .expect_err("a routable bind without credentials must be refused");
    assert!(error.contains("non-loopback"), "error={error}");
    assert!(
        !socket.exists(),
        "no socket may be created for a refused start"
    );
}

#[tokio::test]
async fn an_opted_out_plan_requests_no_listener() {
    let plan = ListenerPlan {
        unix_socket: None,
        tcp: false,
    };
    assert!(plan.is_empty());
    assert!(!ListenerPlan {
        unix_socket: None,
        tcp: true
    }
    .is_empty());
    assert!(!ListenerPlan {
        unix_socket: Some("/tmp/x.sock".into()),
        tcp: false
    }
    .is_empty());
}
