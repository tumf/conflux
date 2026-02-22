//! Shared test helpers for remote module tests.
//!
//! This module provides common utilities used across `ws.rs`, `client.rs`, and
//! `types.rs` tests:
//!
//! - [`spawn_mock_ws_server`]: spawn a minimal WebSocket server for integration tests
//! - [`spawn_mock_http_server`]: spawn a minimal HTTP server for integration tests
//! - JSON fixture builders for `RemoteChange`, `RemoteProject`, and `RemoteLogEntry`

use tokio::sync::mpsc;

use super::types::{RemoteChange, RemoteLogEntry, RemoteProject};

// ─── WebSocket mock server ───────────────────────────────────────────────────

/// Spawn a minimal WebSocket server that sends `messages` then closes the connection.
///
/// Returns the bound [`std::net::SocketAddr`] so the test can construct a `ws://…` URL.
///
/// # Example
/// ```rust,ignore
/// let addr = spawn_mock_ws_server(vec![my_json]).await;
/// let ws_url = format!("ws://{}/api/v1/ws", addr);
/// ```
pub async fn spawn_mock_ws_server(messages: Vec<String>) -> std::net::SocketAddr {
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(mut ws) = accept_async(stream).await {
                use futures_util::SinkExt;
                for msg in messages {
                    let _ = ws.send(Message::Text(msg)).await;
                }
                let _ = ws.close(None).await;
            }
        }
    });

    addr
}

// ─── HTTP mock server ─────────────────────────────────────────────────────────

/// Spawn a minimal HTTP server that captures the raw request and returns an HTTP 200
/// response with an empty JSON array body (`[]`).
///
/// Returns `(addr, rx)` where `rx` yields the raw HTTP request text sent by the client.
///
/// # Example
/// ```rust,ignore
/// let (addr, req_rx) = spawn_mock_http_server().await;
/// let client = RemoteClient::new(format!("http://{}", addr), None);
/// ```
pub async fn spawn_mock_http_server(
) -> (std::net::SocketAddr, tokio::sync::oneshot::Receiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request_text = String::from_utf8_lossy(&buf[..n]).to_string();
            let _ = tx.send(request_text);

            // Send minimal HTTP 200 response with an empty JSON array body
            let response =
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n[]";
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    (addr, rx)
}

// ─── JSON fixture helpers ─────────────────────────────────────────────────────

/// Build a JSON string for a [`RemoteChange`] with the given field values.
///
/// All other fields are set to reasonable defaults so callers only need to supply what
/// matters for their particular test.
pub fn remote_change_json(
    id: &str,
    project: &str,
    completed_tasks: u32,
    total_tasks: u32,
    status: &str,
    iteration_number: Option<u32>,
) -> String {
    let iter = match iteration_number {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    };
    format!(
        r#"{{
            "id": "{id}",
            "project": "{project}",
            "completed_tasks": {completed_tasks},
            "total_tasks": {total_tasks},
            "last_modified": "2024-01-01T00:00:00Z",
            "status": "{status}",
            "iteration_number": {iter}
        }}"#
    )
}

/// Build a JSON string for a `full_state` WebSocket message containing a single project.
///
/// `changes_json` is a slice of raw JSON strings produced by [`remote_change_json`] (or
/// hand-crafted strings).  Pass an empty slice for a project with no changes.
pub fn full_state_json(project_id: &str, project_name: &str, changes_json: &[String]) -> String {
    let changes = changes_json.join(",\n");
    format!(
        r#"{{
            "type": "full_state",
            "projects": [
                {{
                    "id": "{project_id}",
                    "name": "{project_name}",
                    "changes": [{changes}]
                }}
            ]
        }}"#
    )
}

/// Build a JSON string for a `change_update` WebSocket message.
pub fn change_update_json(change_json: &str) -> String {
    format!(
        r#"{{
            "type": "change_update",
            "change": {change_json}
        }}"#
    )
}

/// Build a JSON string for a `log` WebSocket message.
pub fn log_message_json(
    message: &str,
    level: &str,
    change_id: Option<&str>,
    timestamp: &str,
) -> String {
    let change_id_json = match change_id {
        Some(id) => format!(r#""{id}""#),
        None => "null".to_string(),
    };
    format!(
        r#"{{
            "type": "log",
            "entry": {{
                "message": "{message}",
                "level": "{level}",
                "change_id": {change_id_json},
                "timestamp": "{timestamp}"
            }}
        }}"#
    )
}

// ─── Struct builders ──────────────────────────────────────────────────────────

/// Create a [`RemoteChange`] with sensible defaults for use in unit tests.
pub fn make_remote_change(id: &str, project: &str) -> RemoteChange {
    RemoteChange {
        id: id.to_string(),
        project: project.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "2024-01-01T00:00:00Z".to_string(),
        status: "queued".to_string(),
        iteration_number: None,
    }
}

/// Create a [`RemoteProject`] containing the given changes.
pub fn make_remote_project(id: &str, name: &str, changes: Vec<RemoteChange>) -> RemoteProject {
    RemoteProject {
        id: id.to_string(),
        name: name.to_string(),
        changes,
    }
}

/// Create a [`RemoteLogEntry`] with sensible defaults for use in unit tests.
pub fn make_remote_log_entry(message: &str, level: &str) -> RemoteLogEntry {
    RemoteLogEntry {
        message: message.to_string(),
        level: level.to_string(),
        change_id: None,
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        project_id: None,
        operation: None,
        iteration: None,
    }
}

// ─── Flexible HTTP mock server ────────────────────────────────────────────────

/// A captured HTTP request from the flexible mock server.
#[derive(Debug)]
pub struct CapturedRequest {
    /// Raw HTTP request text (lowercased for easy header inspection)
    pub raw: String,
    /// HTTP method extracted from the request line (e.g., "GET", "POST", "DELETE")
    pub method: String,
    /// Request path extracted from the request line (e.g., "/api/v1/projects")
    pub path: String,
    /// Request body (if any)
    pub body: String,
}

/// Spawn a flexible HTTP mock server that captures the full request and returns a
/// configurable response body with HTTP 200.
///
/// Returns `(addr, rx)` where `rx` yields a [`CapturedRequest`] for the first
/// connection received.
pub async fn spawn_flexible_mock_http_server(
    response_body: String,
) -> (
    std::net::SocketAddr,
    tokio::sync::oneshot::Receiver<CapturedRequest>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request_text = String::from_utf8_lossy(&buf[..n]).to_string();

            // Parse method and path from the request line
            let first_line = request_text.lines().next().unwrap_or("");
            let parts: Vec<&str> = first_line.split_whitespace().collect();
            let method = parts.first().copied().unwrap_or("").to_string();
            let path = parts.get(1).copied().unwrap_or("").to_string();

            // Extract body (after blank line separating headers from body)
            let body = if let Some(idx) = request_text.find("\r\n\r\n") {
                request_text[idx + 4..].to_string()
            } else {
                String::new()
            };

            let captured = CapturedRequest {
                raw: request_text.to_lowercase(),
                method,
                path,
                body,
            };
            let _ = tx.send(captured);

            let content_len = response_body.len();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                content_len, response_body
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
    });

    (addr, rx)
}

// ─── WS header capture server ────────────────────────────────────────────────

/// Spawn a raw TCP server that captures the HTTP upgrade request (including headers)
/// without completing the WebSocket handshake.
///
/// Unlike [`spawn_mock_ws_server`], this server captures raw bytes so that tests can
/// inspect arbitrary headers (e.g. `Authorization`) in the WS upgrade request.
///
/// Returns `(addr, rx)` where `rx` yields the lowercased raw HTTP request text.
///
/// # Example
/// ```rust,ignore
/// let (addr, mut header_rx) = spawn_ws_header_capture_server().await;
/// let ws_url = format!("ws://{}/api/v1/ws", addr);
/// // … initiate connection …
/// let request = recv_with_timeout(&mut header_rx, 2, "WS upgrade request").await;
/// assert!(request.contains("authorization: bearer my-token"));
/// ```
pub async fn spawn_ws_header_capture_server(
) -> (std::net::SocketAddr, tokio::sync::mpsc::Receiver<String>) {
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(1);

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            let request_text = String::from_utf8_lossy(&buf[..n]).to_lowercase();
            let _ = tx.send(request_text).await;
        }
    });

    (addr, rx)
}

// ─── Multi-request ordered mock HTTP server ───────────────────────────────────

/// Spawn a mock HTTP server that handles `responses.len()` sequential requests.
///
/// For each connection, the server reads the request, captures the HTTP method
/// and path from the request line, and sends the corresponding `(status, body)`.
///
/// Returns `(addr, path_rx)` where `path_rx` yields (in order) strings of the form
/// `"METHOD /path"` for each request received.
///
/// # Example
/// ```rust,ignore
/// let responses = vec![
///     (200, r#"[{"id":"p1","remote_url":"u","branch":"b","status":"idle","created_at":"t"}]"#.to_string()),
///     (200, "{}".to_string()),
/// ];
/// let (addr, mut path_rx) = spawn_mock_http_server_ordered(responses).await;
/// ```
pub async fn spawn_mock_http_server_ordered(
    responses: Vec<(u16, String)>,
) -> (std::net::SocketAddr, tokio::sync::mpsc::Receiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let count = responses.len();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(count + 1);

    tokio::spawn(async move {
        for (status, body) in responses {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let request_text = String::from_utf8_lossy(&buf[..n]).to_string();

                // Extract "METHOD /path" from the first request line
                let method_path = request_text
                    .lines()
                    .next()
                    .and_then(|line| {
                        let parts: Vec<&str> = line.splitn(3, ' ').collect();
                        if parts.len() >= 2 {
                            Some(format!("{} {}", parts[0], parts[1]))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let _ = tx.send(method_path).await;

                let reason = if status == 200 {
                    "OK"
                } else {
                    "Internal Server Error"
                };
                let content_len = body.len();
                // Include Connection: close so that HTTP/1.1 clients (e.g. reqwest) do not
                // attempt to reuse the connection.  Without this header, reqwest's keep-alive
                // pool may try to reuse a connection that the server has already dropped,
                // causing intermittent "error sending request" failures in the test suite.
                let response = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {content_len}\r\nConnection: close\r\n\r\n{body}"
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        }
    });

    (addr, rx)
}

// ─── Channel receive helper ───────────────────────────────────────────────────

/// Wait up to `timeout_secs` seconds for a message on `rx`, panicking with a helpful
/// message if the timeout is exceeded or the channel closes before a message arrives.
pub async fn recv_with_timeout<T: std::fmt::Debug>(
    rx: &mut mpsc::Receiver<T>,
    timeout_secs: u64,
    context: &str,
) -> T {
    tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for {} message", context))
        .unwrap_or_else(|| panic!("Channel closed before receiving {} message", context))
}
