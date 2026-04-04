use super::*;

use crate::server::api::worktrees::resolve_project_worktree_path;

// ─────────────────────────────── Proposal Session types ────────────────────────

/// WebSocket message from client to server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProposalWsClientMessage {
    Prompt {
        #[serde(alias = "content")]
        text: String,
        #[serde(default)]
        client_message_id: Option<String>,
    },
    ElicitationResponse {
        #[allow(dead_code)]
        #[serde(alias = "elicitation_id")]
        request_id: String,
        #[allow(dead_code)]
        action: String,
        #[allow(dead_code)]
        #[serde(default, alias = "data")]
        content: Option<serde_json::Value>,
    },
    Cancel,
}

/// WebSocket message from server to client.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProposalWsServerMessage {
    UserMessage {
        id: String,
        content: String,
        timestamp: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        client_message_id: Option<String>,
    },
    AgentMessageChunk {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    AgentThoughtChunk {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    ToolCall {
        tool_call_id: String,
        title: String,
        kind: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    ToolCallUpdate {
        tool_call_id: String,
        status: String,
        content: Vec<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    #[allow(dead_code)]
    Elicitation {
        request_id: String,
        mode: String,
        message: String,
        schema: Option<serde_json::Value>,
    },
    TurnComplete {
        stop_reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    RecoveryState {
        active: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_id: Option<String>,
    },
    Heartbeat {
        sent_at: String,
    },
    Error {
        message: String,
    },
}

/// Request body for closing a proposal session.
#[derive(Debug, Deserialize)]
pub struct CloseProposalSessionRequest {
    #[serde(default)]
    pub force: bool,
}

// ─────────────────────────────── Proposal Session handlers ─────────────────────

/// POST /api/v1/projects/{id}/proposal-sessions
pub(super) async fn create_proposal_session(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let mut manager = state.proposal_session_manager.write().await;
    match manager.create_session(&project_id, &worktree_path).await {
        Ok(info) => {
            let dirty_state = manager.check_dirty(&info.id).await.ok();
            let response = serde_json::json!({
                "id": info.id,
                "project_id": info.project_id,
                "status": info.status,
                "worktree_branch": info.worktree_branch,
                "is_dirty": dirty_state.as_ref().map(|(is_dirty, _)| *is_dirty).unwrap_or(false),
                "uncommitted_files": dirty_state.map(|(_, files)| files).unwrap_or_default(),
                "created_at": info.created_at,
                "updated_at": info.updated_at,
            });
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
    }
}

/// GET /api/v1/projects/{id}/proposal-sessions
pub(super) async fn list_proposal_sessions(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Response {
    let manager = state.proposal_session_manager.read().await;
    let sessions = manager.list_sessions(&project_id);
    let responses = futures_util::future::join_all(sessions.into_iter().map(|info| {
        let manager = state.proposal_session_manager.clone();
        async move {
            let dirty_state = manager.read().await.check_dirty(&info.id).await.ok();
            serde_json::json!({
                "id": info.id,
                "project_id": info.project_id,
                "status": info.status,
                "worktree_branch": info.worktree_branch,
                "is_dirty": dirty_state.as_ref().map(|(is_dirty, _)| *is_dirty).unwrap_or(false),
                "uncommitted_files": dirty_state.map(|(_, files)| files).unwrap_or_default(),
                "created_at": info.created_at,
                "updated_at": info.updated_at,
            })
        }
    }))
    .await;
    (StatusCode::OK, Json(serde_json::json!(responses))).into_response()
}

/// DELETE /api/v1/projects/{id}/proposal-sessions/{session_id}
pub(super) async fn close_proposal_session(
    State(state): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
    body: Option<Json<CloseProposalSessionRequest>>,
) -> Response {
    let force = body.map(|b| b.force).unwrap_or(false);

    let (worktree_path, _entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let mut manager = state.proposal_session_manager.write().await;
    match manager
        .close_session(&session_id, force, &worktree_path)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "closed"})),
        )
            .into_response(),
        Err(ProposalSessionError::NotFound(id)) => {
            error_response(StatusCode::NOT_FOUND, format!("Session not found: {}", id))
        }
        Err(ProposalSessionError::DirtyWorktree { files }) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "status": "dirty",
                "message": "Worktree has uncommitted changes. Use force: true to close anyway.",
                "uncommitted_files": files
            })),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
    }
}

/// POST /api/v1/projects/{id}/proposal-sessions/{session_id}/merge
pub(super) async fn merge_proposal_session(
    State(state): State<AppState>,
    Path((project_id, session_id)): Path<(String, String)>,
) -> Response {
    let (worktree_path, entry) = match resolve_project_worktree_path(&state, &project_id).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let mut manager = state.proposal_session_manager.write().await;
    match manager
        .merge_session(&session_id, &worktree_path, &entry.branch)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "merged"})),
        )
            .into_response(),
        Err(ProposalSessionError::NotFound(id)) => {
            error_response(StatusCode::NOT_FOUND, format!("Session not found: {}", id))
        }
        Err(ProposalSessionError::DirtyWorktree { files }) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "status": "dirty",
                "message": "Worktree has uncommitted changes. Resolve them before merging.",
                "uncommitted_files": files
            })),
        )
            .into_response(),
        Err(ProposalSessionError::MergeConflict(msg)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "status": "conflict",
                "message": msg
            })),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
    }
}

/// GET /api/v1/projects/{id}/proposal-sessions/{session_id}/changes
pub(super) async fn list_proposal_session_changes(
    State(state): State<AppState>,
    Path((_project_id, session_id)): Path<(String, String)>,
) -> Response {
    let manager = state.proposal_session_manager.read().await;
    match manager.detect_changes(&session_id).await {
        Ok(changes) => (StatusCode::OK, Json(serde_json::json!(changes))).into_response(),
        Err(ProposalSessionError::NotFound(id)) => {
            error_response(StatusCode::NOT_FOUND, format!("Session not found: {}", id))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
    }
}

/// GET /api/v1/projects/{id}/proposal-sessions/{session_id}/messages
pub(super) async fn get_proposal_session_messages(
    State(state): State<AppState>,
    Path((_project_id, session_id)): Path<(String, String)>,
) -> Response {
    let manager = state.proposal_session_manager.read().await;
    match manager.list_messages(&session_id) {
        Ok(messages) => {
            let response = serde_json::json!({ "messages": messages });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(ProposalSessionError::NotFound(id)) => {
            error_response(StatusCode::NOT_FOUND, format!("Session not found: {}", id))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("{}", e)),
    }
}

/// GET /api/v1/proposal-sessions/{session_id}/ws
pub(super) async fn proposal_session_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    ws.on_upgrade(move |socket| proposal_session_ws(socket, state, session_id))
}

fn build_replay_ws_messages(
    messages: Vec<ProposalSessionMessageRecord>,
    active_turn_id: Option<String>,
) -> Vec<String> {
    let mut replay_messages = Vec::new();

    for msg in messages {
        if msg.role == "user" {
            replay_messages.push(
                serde_json::to_string(&ProposalWsServerMessage::UserMessage {
                    id: msg.id,
                    content: msg.content,
                    timestamp: msg.timestamp,
                    client_message_id: msg.client_message_id,
                })
                .unwrap_or_default(),
            );
            continue;
        }

        if msg.role == "assistant" {
            let message_id = Some(msg.id.clone());
            let turn_id = msg.turn_id.clone();
            if !msg.content.is_empty() {
                let replay_chunk = if msg.is_thought == Some(true) {
                    ProposalWsServerMessage::AgentThoughtChunk {
                        text: msg.content.clone(),
                        message_id: message_id.clone(),
                        turn_id: turn_id.clone(),
                    }
                } else {
                    ProposalWsServerMessage::AgentMessageChunk {
                        text: msg.content.clone(),
                        message_id: message_id.clone(),
                        turn_id: turn_id.clone(),
                    }
                };
                replay_messages.push(serde_json::to_string(&replay_chunk).unwrap_or_default());
            }
            if let Some(tool_calls) = msg.tool_calls {
                for tool_call in tool_calls {
                    replay_messages.push(
                        serde_json::to_string(&ProposalWsServerMessage::ToolCall {
                            tool_call_id: tool_call.id,
                            title: tool_call.title,
                            kind: "tool".to_string(),
                            status: tool_call.status,
                            message_id: message_id.clone(),
                            turn_id: turn_id.clone(),
                        })
                        .unwrap_or_default(),
                    );
                }
            }
            if msg.turn_id.is_some() && msg.turn_id != active_turn_id {
                replay_messages.push(
                    serde_json::to_string(&ProposalWsServerMessage::TurnComplete {
                        stop_reason: "end_turn".to_string(),
                        message_id,
                        turn_id,
                    })
                    .unwrap_or_default(),
                );
            }
        }
    }

    if !replay_messages.is_empty() || active_turn_id.is_some() {
        replay_messages.push(
            serde_json::to_string(&ProposalWsServerMessage::RecoveryState {
                active: active_turn_id.is_some(),
                turn_id: active_turn_id,
            })
            .unwrap_or_default(),
        );
    }

    replay_messages
}

async fn proposal_session_ws(socket: WebSocket, state: AppState, session_id: String) {
    use futures_util::{SinkExt, StreamExt};

    let (mut ws_sender, mut ws_receiver) = socket.split();

    info!(session_id = %session_id, "Proposal session WebSocket connected");

    let (acp_client, acp_session_id, prompt_prefix_blocks) = {
        let manager = state.proposal_session_manager.read().await;
        match manager.get_session(&session_id) {
            Some(session) => {
                if session.status != crate::server::proposal_session::ProposalSessionStatus::Active
                {
                    let _ = ws_sender
                        .send(Message::Text(
                            serde_json::to_string(&ProposalWsServerMessage::Error {
                                message: "Session is not active".to_string(),
                            })
                            .unwrap_or_default()
                            .into(),
                        ))
                        .await;
                    return;
                }

                let prompt_prefix_blocks = match manager.prompt_prefix_blocks(&session_id) {
                    Ok(blocks) => blocks.to_vec(),
                    Err(_) => {
                        let _ = ws_sender
                            .send(Message::Text(
                                serde_json::to_string(&ProposalWsServerMessage::Error {
                                    message: "Session prompt configuration missing".to_string(),
                                })
                                .unwrap_or_default()
                                .into(),
                            ))
                            .await;
                        return;
                    }
                };

                (
                    session.acp_client.clone(),
                    session.acp_session_id.clone(),
                    prompt_prefix_blocks,
                )
            }
            None => {
                let _ = ws_sender
                    .send(Message::Text(
                        serde_json::to_string(&ProposalWsServerMessage::Error {
                            message: "Session not found".to_string(),
                        })
                        .unwrap_or_default()
                        .into(),
                    ))
                    .await;
                return;
            }
        }
    };

    let (ws_send_tx, mut ws_send_rx) = tokio::sync::mpsc::channel::<String>(256);

    // Replay in-memory message history on reconnect.
    let replay_payload = {
        let manager = state.proposal_session_manager.read().await;
        let messages = manager.list_messages(&session_id);
        let active_turn_id = manager.get_active_turn_id(&session_id);
        (messages, active_turn_id)
    };

    match replay_payload {
        (Ok(messages), Ok(active_turn_id)) => {
            for replay_message in build_replay_ws_messages(messages, active_turn_id) {
                let _ = ws_send_tx.send(replay_message).await;
            }
        }
        (Err(e), _) => {
            error!(error = %e, session_id = %session_id, "Failed to load proposal session history");
        }
        (_, Err(e)) => {
            error!(error = %e, session_id = %session_id, "Failed to read active turn for recovery replay");
        }
    }

    let acp_client_for_notifs = acp_client.clone();
    let acp_session_id_for_notifs = acp_session_id.clone();
    let state_for_notifs = state.clone();
    let session_id_for_notifs = session_id.clone();
    let ws_send_tx_for_notifs = ws_send_tx.clone();

    let notif_task = tokio::spawn(async move {
        while let Some(notification) = acp_client_for_notifs.recv_notification().await {
            if let Some(update) = notification.as_update() {
                if let Some(event_session_id) = update.session_id.as_deref() {
                    if event_session_id != acp_session_id_for_notifs {
                        continue;
                    }
                }

                let ws_message = match update.update {
                    crate::server::acp_client::AcpEvent::AgentMessageChunk { content } => {
                        let text = content.map(|c| c.text).unwrap_or_default();
                        let turn_id = match state_for_notifs
                            .proposal_session_manager
                            .write()
                            .await
                            .append_assistant_chunk(&session_id_for_notifs, &text)
                        {
                            Ok(turn_id) => Some(turn_id),
                            Err(e) => {
                                warn!(
                                    session_id = %session_id_for_notifs,
                                    error = %e,
                                    "Failed to append assistant chunk to proposal history"
                                );
                                None
                            }
                        };
                        let message_id = turn_id.as_ref().map(|id| format!("assistant-{id}"));
                        Some(ProposalWsServerMessage::AgentMessageChunk {
                            text,
                            message_id,
                            turn_id,
                        })
                    }
                    crate::server::acp_client::AcpEvent::AgentThoughtChunk { content } => {
                        let text = content.map(|c| c.text).unwrap_or_default();
                        let turn_id = match state_for_notifs
                            .proposal_session_manager
                            .write()
                            .await
                            .append_assistant_thought_chunk(&session_id_for_notifs, &text)
                        {
                            Ok(turn_id) => Some(turn_id),
                            Err(e) => {
                                warn!(
                                    session_id = %session_id_for_notifs,
                                    error = %e,
                                    "Failed to append assistant thought chunk to proposal history"
                                );
                                None
                            }
                        };
                        let message_id = turn_id.as_ref().map(|id| format!("assistant-{id}"));
                        Some(ProposalWsServerMessage::AgentThoughtChunk {
                            text,
                            message_id,
                            turn_id,
                        })
                    }
                    crate::server::acp_client::AcpEvent::ToolCall {
                        tool_call_id,
                        title,
                        kind,
                        status,
                    } => {
                        let (message_id, turn_id) = match state_for_notifs
                            .proposal_session_manager
                            .write()
                            .await
                            .record_tool_call(
                                &session_id_for_notifs,
                                &tool_call_id,
                                &title,
                                &status,
                            ) {
                            Ok((message_id, turn_id)) => (Some(message_id), Some(turn_id)),
                            Err(e) => {
                                warn!(
                                    session_id = %session_id_for_notifs,
                                    error = %e,
                                    "Failed to record proposal tool call in history"
                                );
                                (None, None)
                            }
                        };
                        Some(ProposalWsServerMessage::ToolCall {
                            tool_call_id,
                            title,
                            kind,
                            status,
                            message_id,
                            turn_id,
                        })
                    }
                    crate::server::acp_client::AcpEvent::ToolCallUpdate {
                        tool_call_id,
                        status,
                        content,
                    } => {
                        let (message_id, turn_id) = match state_for_notifs
                            .proposal_session_manager
                            .write()
                            .await
                            .update_tool_call_status(&session_id_for_notifs, &tool_call_id, &status)
                        {
                            Ok((message_id, turn_id)) => (Some(message_id), turn_id),
                            Err(e) => {
                                warn!(
                                    session_id = %session_id_for_notifs,
                                    error = %e,
                                    "Failed to update proposal tool call status in history"
                                );
                                (None, None)
                            }
                        };
                        Some(ProposalWsServerMessage::ToolCallUpdate {
                            tool_call_id,
                            status,
                            content,
                            message_id,
                            turn_id,
                        })
                    }
                    crate::server::acp_client::AcpEvent::Elicitation {
                        request_id,
                        mode,
                        message,
                        schema,
                    } => Some(ProposalWsServerMessage::Elicitation {
                        request_id,
                        mode,
                        message,
                        schema,
                    }),
                    crate::server::acp_client::AcpEvent::TurnComplete { stop_reason } => {
                        let (message_id, turn_id) = match state_for_notifs
                            .proposal_session_manager
                            .write()
                            .await
                            .complete_active_turn(&session_id_for_notifs)
                        {
                            Ok(Some((message_id, turn_id))) => (Some(message_id), turn_id),
                            Ok(None) => (None, None),
                            Err(e) => {
                                warn!(
                                    session_id = %session_id_for_notifs,
                                    error = %e,
                                    "Failed to complete active proposal turn"
                                );
                                (None, None)
                            }
                        };
                        Some(ProposalWsServerMessage::TurnComplete {
                            stop_reason,
                            message_id,
                            turn_id,
                        })
                    }
                    crate::server::acp_client::AcpEvent::Unknown => None,
                };

                if let Some(msg) = ws_message {
                    let mut mgr = state_for_notifs.proposal_session_manager.write().await;
                    if let Err(e) = mgr.touch_session_activity(&session_id_for_notifs) {
                        warn!(
                            session_id = %session_id_for_notifs,
                            error = %e,
                            "Failed to persist proposal session activity"
                        );
                    }
                    drop(mgr);

                    if ws_send_tx_for_notifs
                        .send(serde_json::to_string(&msg).unwrap_or_default())
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                continue;
            }

            if let Some(elicitation) = notification.as_elicitation() {
                if let Some(event_session_id) = elicitation.session_id.as_deref() {
                    if event_session_id != acp_session_id_for_notifs {
                        continue;
                    }
                }

                let msg = ProposalWsServerMessage::Elicitation {
                    request_id: elicitation.request_id,
                    mode: elicitation.mode,
                    message: elicitation.message,
                    schema: elicitation.schema,
                };

                if ws_send_tx_for_notifs
                    .send(serde_json::to_string(&msg).unwrap_or_default())
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });

    let send_task = tokio::spawn(async move {
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                maybe_json = ws_send_rx.recv() => {
                    match maybe_json {
                        Some(json) => {
                            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = heartbeat_interval.tick() => {
                    let heartbeat = ProposalWsServerMessage::Heartbeat {
                        sent_at: chrono::Utc::now().to_rfc3339(),
                    };
                    let json = serde_json::to_string(&heartbeat).unwrap_or_default();
                    if ws_sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let acp_client_for_recv = acp_client.clone();
    let acp_session_id_for_recv = acp_session_id.clone();
    let prompt_prefix_blocks_for_recv = prompt_prefix_blocks.clone();
    let session_id_for_recv = session_id.clone();
    let state_for_recv = state.clone();
    let ws_send_tx_for_recv = ws_send_tx.clone();

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let text_str: &str = &text;
                    match serde_json::from_str::<ProposalWsClientMessage>(text_str) {
                        Ok(ProposalWsClientMessage::Prompt {
                            text,
                            client_message_id,
                        }) => {
                            let mut should_forward_prompt = true;
                            {
                                let mut mgr = state_for_recv.proposal_session_manager.write().await;
                                if let Err(e) = mgr.touch_session_activity(&session_id_for_recv) {
                                    warn!(
                                        session_id = %session_id_for_recv,
                                        error = %e,
                                        "Failed to persist proposal session activity"
                                    );
                                }

                                if let Some(cid) = client_message_id.as_deref() {
                                    match mgr.is_client_message_recorded(&session_id_for_recv, cid)
                                    {
                                        Ok(true) => {
                                            should_forward_prompt = false;
                                            debug!(
                                                session_id = %session_id_for_recv,
                                                client_message_id = %cid,
                                                "Skipping duplicate prompt during reconnect recovery"
                                            );
                                        }
                                        Ok(false) => {}
                                        Err(e) => {
                                            warn!(
                                                session_id = %session_id_for_recv,
                                                client_message_id = %cid,
                                                error = %e,
                                                "Failed duplicate prompt check"
                                            );
                                        }
                                    }
                                }

                                if should_forward_prompt {
                                    if let Ok(user_message) = mgr
                                        .record_user_prompt_with_client_message_id(
                                            &session_id_for_recv,
                                            &text,
                                            client_message_id.as_deref(),
                                        )
                                    {
                                        let _ = ws_send_tx_for_recv
                                            .send(
                                                serde_json::to_string(
                                                    &ProposalWsServerMessage::UserMessage {
                                                        id: user_message.id,
                                                        content: user_message.content,
                                                        timestamp: user_message.timestamp,
                                                        client_message_id,
                                                    },
                                                )
                                                .unwrap_or_default(),
                                            )
                                            .await;
                                    }
                                }
                            }

                            if !should_forward_prompt {
                                continue;
                            }

                            if let Err(e) = acp_client_for_recv
                                .send_prompt_with_prefix(
                                    &acp_session_id_for_recv,
                                    &prompt_prefix_blocks_for_recv,
                                    &text,
                                )
                                .await
                            {
                                error!(error = %e, "Failed to send prompt to ACP");
                                let _ = ws_send_tx_for_recv
                                    .send(
                                        serde_json::to_string(&ProposalWsServerMessage::Error {
                                            message: format!("Failed to send prompt: {}", e),
                                        })
                                        .unwrap_or_default(),
                                    )
                                    .await;
                            }
                        }
                        Ok(ProposalWsClientMessage::ElicitationResponse {
                            request_id,
                            action,
                            content,
                        }) => {
                            if let Err(e) = acp_client_for_recv
                                .respond_elicitation(&request_id, &action, content)
                                .await
                            {
                                error!(error = %e, "Failed to send elicitation response to ACP");
                            }
                        }
                        Ok(ProposalWsClientMessage::Cancel) => {
                            if let Err(e) =
                                acp_client_for_recv.cancel(&acp_session_id_for_recv).await
                            {
                                error!(error = %e, "Failed to cancel ACP session");
                            }
                        }
                        Err(e) => {
                            debug!(error = %e, text = %text_str, "Failed to parse client WebSocket message");
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = notif_task => {},
        _ = send_task => {},
        _ = recv_task => {},
    }

    info!(session_id = %session_id, "Proposal session WebSocket disconnected");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use crate::server::api::test_support::make_router;

    #[tokio::test]
    async fn test_proposal_session_ws_route_exists() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/proposal-sessions/test-session-id/ws")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "Proposal session WS route must be registered at /api/v1/proposal-sessions/{{session_id}}/ws"
        );
    }

    #[tokio::test]
    async fn test_proposal_session_ws_old_route_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-project/proposal-sessions/test-session-id/ws")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "Old project-scoped WS route should NOT exist"
        );
    }
}
