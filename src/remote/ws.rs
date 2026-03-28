//! WebSocket client for the remote Conflux server
//!
//! Subscribes to real-time state updates pushed by the server.

use crate::error::{OrchestratorError, Result};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use super::types::RemoteStateUpdate;

/// Connect to the remote server's WebSocket endpoint and stream state updates.
///
/// # Arguments
/// * `ws_url`  – WebSocket URL (e.g., `"ws://host:39876/api/v1/ws"`)
/// * `token`   – Optional bearer token. Sent as an `Authorization` header in the
///   HTTP upgrade request.
/// * `tx`      – Channel sender; received [`RemoteStateUpdate`] messages are forwarded here.
///
/// The function returns as soon as the connection is established and spawns a background
/// Tokio task to receive messages. The task terminates when the connection closes or an
/// unrecoverable error occurs.
pub async fn connect_and_subscribe(
    ws_url: impl Into<String>,
    token: Option<&str>,
    tx: mpsc::Sender<RemoteStateUpdate>,
) -> Result<tokio::task::JoinHandle<()>> {
    let url_str = ws_url.into();
    info!("Connecting to remote WebSocket: {}", url_str);

    // Build the WebSocket request, optionally adding the Authorization header.
    let mut request = url_str.clone().into_client_request().map_err(|e| {
        OrchestratorError::Io(std::io::Error::other(format!(
            "Invalid WebSocket URL '{}': {}",
            url_str, e
        )))
    })?;

    if let Some(t) = token {
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {}", t).parse().map_err(|e| {
                OrchestratorError::Io(std::io::Error::other(format!(
                    "Invalid bearer token (cannot be used as HTTP header value): {}",
                    e
                )))
            })?,
        );
    }

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to WebSocket '{}': {}",
                url_str, e
            )))
        })?;

    info!("WebSocket connection established");

    // Split the stream and discard the sink (we are read-only for now).
    let (_write, mut read) = ws_stream.split();

    let handle = tokio::spawn(async move {
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    debug!("WS message received: {} bytes", text.len());
                    match serde_json::from_str::<RemoteStateUpdate>(&text) {
                        Ok(update) => {
                            if tx.send(update).await.is_err() {
                                // Receiver dropped – stop the task
                                debug!("WS receiver dropped, stopping WebSocket task");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to deserialize WS message: {}", e);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed by server");
                    break;
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // Protocol-level ping/pong – handled automatically by tungstenite
                }
                Ok(_) => {
                    // Binary and other frame types are ignored
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
        info!("WebSocket receiver task finished");
    });

    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{
        change_update_json, full_state_json, log_message_json, recv_with_timeout,
        remote_change_json, spawn_mock_ws_server,
    };
    use super::*;
    use tokio::sync::mpsc;

    /// Verify that connecting to an invalid URL returns an error rather than panicking.
    #[tokio::test]
    async fn test_connect_invalid_url_returns_error() {
        let (tx, _rx) = mpsc::channel(16);
        let result =
            connect_and_subscribe("ws://127.0.0.1:19999/nonexistent", None, tx.clone()).await;
        // Should fail to connect (nothing listening on that port)
        assert!(
            result.is_err(),
            "Expected an error when connecting to an unavailable server"
        );
    }

    /// Verify that a malformed URL produces an error.
    #[tokio::test]
    async fn test_connect_malformed_url_returns_error() {
        let (tx, _rx) = mpsc::channel(16);
        let result = connect_and_subscribe("not a url !", None, tx).await;
        assert!(result.is_err(), "Expected an error for a malformed URL");
    }

    /// Verify that a `full_state` WS message is received and forwarded as a
    /// `RemoteStateUpdate::FullState` through the channel.
    #[tokio::test]
    async fn test_receive_full_state_message() {
        let change = remote_change_json("my-change", "proj-1", 2, 5, "applying", None);
        let msg_json = full_state_json("proj-1", "Project 1", &[change]);

        let addr = spawn_mock_ws_server(vec![msg_json]).await;
        let ws_url = format!("ws://{}/api/v1/ws", addr);

        let (tx, mut rx) = mpsc::channel(16);
        let handle = connect_and_subscribe(ws_url, None, tx)
            .await
            .expect("Should connect to mock server");

        let msg = recv_with_timeout(&mut rx, 3, "full_state").await;

        match msg {
            super::super::types::RemoteStateUpdate::FullState { projects, .. } => {
                assert_eq!(projects.len(), 1);
                assert_eq!(projects[0].id, "proj-1");
                assert_eq!(projects[0].changes.len(), 1);
                assert_eq!(projects[0].changes[0].id, "my-change");
            }
            other => panic!("Expected FullState, got {:?}", other),
        }

        handle.abort();
    }

    /// Verify that a `change_update` WS message is received and forwarded as a
    /// `RemoteStateUpdate::ChangeUpdate` through the channel.
    #[tokio::test]
    async fn test_receive_change_update_message() {
        let change = remote_change_json("feat-x", "proj-2", 3, 7, "applying", Some(1));
        let msg_json = change_update_json(&change);

        let addr = spawn_mock_ws_server(vec![msg_json]).await;
        let ws_url = format!("ws://{}/api/v1/ws", addr);

        let (tx, mut rx) = mpsc::channel(16);
        let handle = connect_and_subscribe(ws_url, None, tx)
            .await
            .expect("Should connect to mock server");

        let msg = recv_with_timeout(&mut rx, 3, "change_update").await;

        match msg {
            super::super::types::RemoteStateUpdate::ChangeUpdate { change } => {
                assert_eq!(change.id, "feat-x");
                assert_eq!(change.project, "proj-2");
                assert_eq!(change.completed_tasks, 3);
                assert_eq!(change.total_tasks, 7);
                assert_eq!(change.iteration_number, Some(1));
            }
            other => panic!("Expected ChangeUpdate, got {:?}", other),
        }

        handle.abort();
    }

    /// Verify that a `log` WS message is received and forwarded as a
    /// `RemoteStateUpdate::Log` through the channel.
    #[tokio::test]
    async fn test_receive_log_message() {
        let msg_json = log_message_json(
            "Build completed successfully",
            "success",
            Some("my-change"),
            "2024-01-01T00:00:00Z",
        );

        let addr = spawn_mock_ws_server(vec![msg_json]).await;
        let ws_url = format!("ws://{}/api/v1/ws", addr);

        let (tx, mut rx) = mpsc::channel(16);
        let handle = connect_and_subscribe(ws_url, None, tx)
            .await
            .expect("Should connect to mock server");

        let msg = recv_with_timeout(&mut rx, 3, "log").await;

        match msg {
            super::super::types::RemoteStateUpdate::Log { entry } => {
                assert_eq!(entry.message, "Build completed successfully");
                assert_eq!(entry.level, "success");
                assert_eq!(entry.change_id, Some("my-change".to_string()));
                assert_eq!(entry.timestamp, "2024-01-01T00:00:00Z");
            }
            other => panic!("Expected Log, got {:?}", other),
        }

        handle.abort();
    }

    /// Verify that the Authorization header is sent when a bearer token is provided.
    /// We use a mock server that captures the raw HTTP upgrade request and checks headers.
    #[tokio::test]
    async fn test_bearer_token_sent_in_ws_upgrade() {
        use super::super::test_helpers::{recv_with_timeout, spawn_ws_header_capture_server};

        let (addr, mut header_rx) = spawn_ws_header_capture_server().await;
        let ws_url = format!("ws://{}/api/v1/ws", addr);
        let (tx, _rx) = mpsc::channel(16);

        // This will fail to complete the WS handshake (mock server doesn't respond properly)
        // but we only care that the HTTP upgrade request contained the Authorization header.
        let _ = connect_and_subscribe(ws_url, Some("test-token-abc"), tx).await;

        // Check if the auth header was present in the request
        let request_lower = recv_with_timeout(&mut header_rx, 2, "WS upgrade request").await;

        assert!(
            request_lower.contains("authorization: bearer test-token-abc"),
            "Expected 'authorization: bearer test-token-abc' in WS upgrade request, got:\n{}",
            request_lower
        );
    }
}
