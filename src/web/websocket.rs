//! WebSocket handler for real-time state updates.

use super::state::WebState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

/// WebSocket connection handler for real-time state updates
///
/// This endpoint upgrades the HTTP connection to a WebSocket connection
/// and streams state updates to the client in real-time.
///
/// # Protocol
/// - Upon connection, the server sends an initial state message
/// - Subsequently, the server broadcasts state updates as they occur
/// - The client can send ping frames to keep the connection alive
/// - The server responds with pong frames automatically
///
/// # Message Format
/// All messages are JSON-encoded with the following structure:
/// ```json
/// {
///   "type": "initial_state" | "state_update",
///   "timestamp": "ISO 8601 timestamp",
///   "state": { ... }  // OrchestratorStateSnapshot
/// }
/// ```
#[cfg_attr(
    feature = "web-monitoring",
    utoipa::path(
        get,
        path = "/ws",
        tag = "websocket",
        responses(
            (status = 101, description = "Switching Protocols - WebSocket connection established"),
            (status = 400, description = "Bad Request - Not a valid WebSocket upgrade request")
        )
    )
)]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: Arc<WebState>) {
    info!("WebSocket client connected");

    // Refresh state from disk to ensure latest data
    let _ = state.refresh_from_disk().await;

    // Send initial state to the client
    let initial_state = state.get_state().await;
    let initial_msg = serde_json::json!({
        "type": "initial_state",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "state": initial_state,
    });

    if let Err(e) = socket
        .send(Message::Text(initial_msg.to_string().into()))
        .await
    {
        error!("Failed to send initial state: {}", e);
        return;
    }

    // Subscribe to state updates
    let mut rx = state.subscribe();

    // Handle messages in a loop
    loop {
        tokio::select! {
            // Receive state updates from broadcast channel
            result = rx.recv() => {
                match result {
                    Ok(update) => {
                        let msg = match serde_json::to_string(&update) {
                            Ok(json) => json,
                            Err(e) => {
                                error!("Failed to serialize state update: {}", e);
                                continue;
                            }
                        };

                        if let Err(e) = socket.send(Message::Text(msg.into())).await {
                            debug!("Failed to send update, client likely disconnected: {}", e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WebSocket client lagged by {} messages", n);
                        // Continue receiving, the client will get the next update
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("Broadcast channel closed");
                        break;
                    }
                }
            }

            // Receive messages from the client (for ping/pong and close handling)
            result = socket.recv() => {
                match result {
                    Some(Ok(Message::Close(_))) => {
                        debug!("Client sent close frame");
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if let Err(e) = socket.send(Message::Pong(data)).await {
                            debug!("Failed to send pong: {}", e);
                            break;
                        }
                    }
                    Some(Ok(_)) => {
                        // Ignore other message types (we don't expect client messages)
                    }
                    Some(Err(e)) => {
                        debug!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        debug!("WebSocket stream ended");
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}

#[cfg(test)]
mod tests {
    // WebSocket tests would require a test server setup
    // For now, we verify the handler compiles correctly
}
