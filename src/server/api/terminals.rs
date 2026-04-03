use super::*;

use crate::server::api::files::resolve_file_root;

// ─────────────────────────────── Terminal session handlers ────────────────────

/// Create a new terminal session with project context (resolves cwd server-side).
pub(super) async fn create_terminal(
    State(state): State<AppState>,
    Json(request): Json<CreateTerminalFromContextRequest>,
) -> Response {
    info!(
        project_id = %request.project_id,
        root = %request.root,
        "Creating terminal session from context"
    );

    // Resolve cwd from project context using the same logic as file browser
    let cwd = match resolve_file_root(&state, &request.project_id, &request.root).await {
        Ok(path) => path.to_string_lossy().to_string(),
        Err(resp) => return resp,
    };

    let create_request = CreateTerminalRequest {
        cwd,
        rows: request.rows,
        cols: request.cols,
        project_id: request.project_id.clone(),
        root: request.root.clone(),
    };

    match state.terminal_manager.create_session(create_request).await {
        Ok(info) => (StatusCode::CREATED, Json(info)).into_response(),
        Err(e) => {
            error!(error = %e, "Failed to create terminal session");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e})),
            )
                .into_response()
        }
    }
}

/// List all terminal sessions.
pub(super) async fn list_terminals(
    State(state): State<AppState>,
) -> Json<Vec<TerminalSessionInfo>> {
    Json(state.terminal_manager.list_sessions().await)
}

/// Delete a terminal session.
pub(super) async fn delete_terminal(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    info!(session_id = %session_id, "Deleting terminal session");
    match state.terminal_manager.delete_session(&session_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to delete terminal session");
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response()
        }
    }
}

/// Resize a terminal session.
pub(super) async fn resize_terminal(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<ResizeTerminalRequest>,
) -> Response {
    debug!(session_id = %session_id, rows = request.rows, cols = request.cols, "Resizing terminal");
    match state
        .terminal_manager
        .resize_session(&session_id, request.rows, request.cols)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

/// WebSocket handler for terminal I/O streaming.
pub(super) async fn terminal_ws_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let manager = state.terminal_manager.clone();

    // Verify session exists before upgrading
    if !manager.session_exists(&session_id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Session not found: {}", session_id)})),
        )
            .into_response();
    }

    ws.on_upgrade(move |socket| handle_terminal_ws(socket, manager, session_id))
}

/// Handle a terminal WebSocket connection.
async fn handle_terminal_ws(socket: WebSocket, manager: SharedTerminalManager, session_id: String) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    info!(session_id = %session_id, "Terminal WebSocket connected");

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Send scrollback buffer contents before subscribing to live output.
    // This ensures the client receives recent output history on reconnection.
    match manager.get_scrollback(&session_id).await {
        Ok(scrollback) if !scrollback.is_empty() => {
            debug!(session_id = %session_id, bytes = scrollback.len(), "Sending scrollback buffer");
            if ws_tx
                .send(Message::Binary(scrollback.into()))
                .await
                .is_err()
            {
                debug!(session_id = %session_id, "WebSocket closed while sending scrollback");
                return;
            }
        }
        Ok(_) => {
            debug!(session_id = %session_id, "No scrollback data to send");
        }
        Err(e) => {
            debug!(session_id = %session_id, error = %e, "Failed to get scrollback, continuing without it");
        }
    }

    // Subscribe to terminal output
    let mut output_rx = match manager.subscribe_output(&session_id).await {
        Ok(rx) => rx,
        Err(e) => {
            error!(session_id = %session_id, error = %e, "Failed to subscribe to terminal output");
            let _ = ws_tx.send(Message::Close(None)).await;
            return;
        }
    };

    let manager_for_input = manager.clone();
    let session_id_for_input = session_id.clone();
    let session_id_for_output = session_id.clone();

    // Task: Forward terminal output to WebSocket
    let output_task = tokio::spawn(async move {
        loop {
            match output_rx.recv().await {
                Ok(data) => {
                    if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                        debug!(session_id = %session_id_for_output, "WebSocket send failed, closing output stream");
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    debug!(session_id = %session_id_for_output, lagged = n, "Terminal output receiver lagged");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    debug!(session_id = %session_id_for_output, "Terminal output channel closed");
                    let _ = ws_tx.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    });

    // Task: Forward WebSocket input to terminal
    let input_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = manager_for_input
                        .write_input(&session_id_for_input, &data)
                        .await
                    {
                        error!(session_id = %session_id_for_input, error = %e, "Failed to write terminal input");
                        break;
                    }
                }
                Ok(Message::Text(text)) => {
                    // Handle text messages as terminal input
                    // Check if it's a JSON resize message
                    if let Ok(resize) = serde_json::from_str::<ResizeTerminalRequest>(&text) {
                        if let Err(e) = manager_for_input
                            .resize_session(&session_id_for_input, resize.rows, resize.cols)
                            .await
                        {
                            debug!(session_id = %session_id_for_input, error = %e, "Failed to resize terminal via WS");
                        }
                    } else {
                        // Plain text input
                        if let Err(e) = manager_for_input
                            .write_input(&session_id_for_input, text.as_bytes())
                            .await
                        {
                            error!(session_id = %session_id_for_input, error = %e, "Failed to write terminal text input");
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!(session_id = %session_id_for_input, "Terminal WebSocket close received");
                    break;
                }
                Ok(_) => {} // Ignore pings/pongs
                Err(e) => {
                    debug!(session_id = %session_id_for_input, error = %e, "Terminal WebSocket error");
                    break;
                }
            }
        }
    });

    // Wait for either task to finish, then abort the other
    tokio::select! {
        _ = output_task => {},
        _ = input_task => {},
    }

    info!(session_id = %session_id, "Terminal WebSocket disconnected");
}
