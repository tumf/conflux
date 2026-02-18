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
/// * `ws_url`  – WebSocket URL (e.g., `"ws://host:9876/api/v1/ws"`)
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
}
