//! ACP (Agent Client Protocol) JSON-RPC over stdio client.
//!
//! Wraps a single ACP subprocess, providing typed methods for the initialize
//! handshake, session lifecycle, and message relay.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Resolve a relative command name to an absolute path via the user's login shell.
///
/// When `command` does not start with `/` (i.e. is a relative/bare name), this function
/// runs `$SHELL -l -c 'which <command>'` to locate the binary using the user's full
/// login-shell PATH (which includes directories like `~/.bun/bin`, `~/.cargo/bin`).
///
/// Returns the absolute path on success, or the original command string as fallback.
/// Resolved command info including the login shell PATH.
struct ResolvedCommand {
    command: String,
    login_shell_path: Option<String>,
}

async fn resolve_command_path(command: &str) -> ResolvedCommand {
    if command.starts_with('/') {
        debug!(command = %command, "ACP command is absolute, skipping resolution");
        return ResolvedCommand {
            command: command.to_string(),
            login_shell_path: resolve_login_shell_path().await,
        };
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let which_arg = format!("which {}", command);

    debug!(
        shell = %shell,
        command = %command,
        "Resolving ACP command via login shell"
    );

    let result = tokio::process::Command::new(&shell)
        .arg("-l")
        .arg("-c")
        .arg(&which_arg)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    let resolved_cmd = match result {
        Ok(output) if output.status.success() => {
            let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if resolved.is_empty() {
                warn!(
                    command = %command,
                    "Login shell 'which' returned empty output, falling back to original"
                );
                command.to_string()
            } else {
                info!(
                    command = %command,
                    resolved = %resolved,
                    "Resolved ACP command via login shell"
                );
                resolved
            }
        }
        Ok(output) => {
            warn!(
                command = %command,
                exit_code = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                "Failed to resolve ACP command via login shell, using original"
            );
            command.to_string()
        }
        Err(e) => {
            warn!(
                command = %command,
                error = %e,
                "Failed to run login shell for command resolution, using original"
            );
            command.to_string()
        }
    };

    ResolvedCommand {
        command: resolved_cmd,
        login_shell_path: resolve_login_shell_path().await,
    }
}

async fn resolve_login_shell_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let result = tokio::process::Command::new(&shell)
        .arg("-l")
        .arg("-c")
        .arg("echo $PATH")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        }
        _ => None,
    }
}

use crate::config::ProposalSessionConfig;

// ── JSON-RPC types ────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A JSON-RPC 2.0 notification (no id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// Represents a JSON-RPC notification from the ACP subprocess.
#[derive(Debug, Clone)]
pub enum AcpMessage {
    Notification(JsonRpcNotification),
}

// ── ACP update event types ────────────────────────────────────────────────

/// Wrapper for the `session/update` notification params.
///
/// ACP spec: `{ "sessionId": "...", "update": { "sessionUpdate": "...", ... } }`
#[derive(Debug, Clone, Deserialize)]
pub struct AcpUpdateParams {
    #[serde(default, rename = "sessionId")]
    #[allow(dead_code)]
    pub session_id: Option<String>,
    pub update: AcpEvent,
}

/// Events emitted by the ACP subprocess via `session/update` notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub enum AcpEvent {
    AgentMessageChunk {
        #[serde(default)]
        content: Option<AcpContent>,
    },
    AgentThoughtChunk {
        #[serde(default)]
        content: Option<AcpContent>,
    },
    ToolCall {
        #[serde(default, rename = "toolCallId")]
        tool_call_id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        kind: String,
        #[serde(default)]
        status: String,
    },
    ToolCallUpdate {
        #[serde(default, rename = "toolCallId")]
        tool_call_id: String,
        #[serde(default)]
        status: String,
        #[serde(default)]
        content: Vec<Value>,
    },
    Elicitation {
        #[serde(default, rename = "requestId")]
        request_id: String,
        #[serde(default)]
        mode: String,
        #[serde(default)]
        message: String,
        #[serde(default)]
        schema: Option<Value>,
    },
    TurnComplete {
        #[serde(default, rename = "stopReason")]
        stop_reason: String,
    },
    #[serde(other)]
    Unknown,
}

/// Content block in ACP messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpContent {
    #[serde(default, rename = "type")]
    pub content_type: String,
    #[serde(default)]
    pub text: String,
}

// ── AcpClient ─────────────────────────────────────────────────────────────

/// ACP client wrapping a subprocess communicating via JSON-RPC over stdio.
pub struct AcpClient {
    /// Sender half for writing JSON-RPC messages to the subprocess stdin.
    stdin_tx: mpsc::Sender<String>,
    /// Sender for synthetic notifications generated from request responses.
    notification_tx: mpsc::Sender<AcpMessage>,
    /// Receiver half for incoming notifications from the subprocess stdout.
    notification_rx: Mutex<mpsc::Receiver<AcpMessage>>,
    /// Monotonically incrementing request ID counter.
    next_id: AtomicU64,
    /// Channel for receiving responses keyed by request ID.
    response_rx: Mutex<mpsc::Receiver<JsonRpcResponse>>,
    /// Handle to the child process.
    child: Mutex<Option<Child>>,
    /// Whether the ACP session has been initialized.
    initialized: Mutex<bool>,
    /// Working directory for this ACP subprocess.
    working_dir: PathBuf,
}

impl AcpClient {
    /// Spawn a new ACP subprocess in the given working directory.
    pub async fn spawn(
        config: &ProposalSessionConfig,
        working_dir: &Path,
    ) -> Result<Arc<Self>, AcpError> {
        // Resolve relative command names to absolute paths via login shell PATH.
        let resolved = resolve_command_path(&config.transport_command).await;

        info!(
            cmd = %config.transport_command,
            resolved_cmd = %resolved.command,
            args = ?config.transport_args,
            cwd = %working_dir.display(),
            "Spawning ACP subprocess"
        );

        let mut cmd = Command::new(&resolved.command);
        cmd.args(&config.transport_args)
            .current_dir(working_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(ref login_path) = resolved.login_shell_path {
            cmd.env("PATH", login_path);
        }

        for (key, value) in &config.transport_env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| AcpError::SpawnFailed {
            command: resolved.command.clone(),
            reason: e.to_string(),
        })?;

        let child_stdin = child.stdin.take().ok_or(AcpError::StdioPipeMissing)?;
        let child_stdout = child.stdout.take().ok_or(AcpError::StdioPipeMissing)?;

        // Channel for sending lines to stdin
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(64);

        // Channel for notifications going to the WebSocket relay
        let (notif_tx, notif_rx) = mpsc::channel::<AcpMessage>(256);

        // Channel for request/response correlation
        let (response_tx, response_rx) = mpsc::channel::<JsonRpcResponse>(64);

        // Spawn stdin writer task
        let mut writer = child_stdin;
        tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if let Err(e) = writer.write_all(line.as_bytes()).await {
                    error!(error = %e, "Failed to write to ACP stdin");
                    break;
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    error!(error = %e, "Failed to write newline to ACP stdin");
                    break;
                }
                if let Err(e) = writer.flush().await {
                    error!(error = %e, "Failed to flush ACP stdin");
                    break;
                }
            }
            debug!("ACP stdin writer task ended");
        });

        // Spawn stdout reader task
        let response_tx_clone = response_tx.clone();
        let notif_tx_clone = notif_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(child_stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                debug!(line = %line, "ACP stdout");

                match serde_json::from_str::<Value>(&line) {
                    Ok(val) => {
                        // Check if it's a response (has "id" and either "result" or "error")
                        if val.get("id").is_some()
                            && (val.get("result").is_some() || val.get("error").is_some())
                        {
                            match serde_json::from_value::<JsonRpcResponse>(val) {
                                Ok(resp) => {
                                    if response_tx_clone.send(resp).await.is_err() {
                                        debug!("Response channel closed");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "Failed to parse JSON-RPC response");
                                }
                            }
                        } else {
                            // Treat as notification
                            match serde_json::from_value::<JsonRpcNotification>(val) {
                                Ok(notif) => {
                                    let msg = AcpMessage::Notification(notif);
                                    if notif_tx_clone.send(msg).await.is_err() {
                                        debug!("Notification channel closed");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, "Failed to parse JSON-RPC notification");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!(error = %e, line = %line, "Non-JSON line from ACP stdout");
                    }
                }
            }
            debug!("ACP stdout reader task ended");
        });

        // Spawn stderr reader (log only)
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(target: "acp_stderr", "{}", line);
                }
            });
        }

        let client = Arc::new(Self {
            stdin_tx,
            notification_tx: notif_tx.clone(),
            notification_rx: Mutex::new(notif_rx),
            next_id: AtomicU64::new(1),
            response_rx: Mutex::new(response_rx),
            child: Mutex::new(Some(child)),
            initialized: Mutex::new(false),
            working_dir: working_dir.to_path_buf(),
        });

        Ok(client)
    }

    /// Perform the ACP `initialize` handshake.
    pub async fn initialize(&self) -> Result<Value, AcpError> {
        let params = serde_json::json!({
            "protocolVersion": 1,
            "clientInfo": {
                "name": "conflux-dashboard",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "elicitation": {
                    "form": {}
                }
            }
        });

        let result = self.send_request("initialize", Some(params)).await?;

        let mut init = self.initialized.lock().await;
        *init = true;

        info!("ACP initialize handshake completed");
        Ok(result)
    }

    /// Create a new ACP session, returning the session ID.
    pub async fn create_session(&self) -> Result<String, AcpError> {
        let cwd = self.working_dir.to_str().unwrap_or(".").to_string();

        let result = self
            .send_request(
                "session/new",
                Some(serde_json::json!({
                    "cwd": cwd,
                    "mcpServers": []
                })),
            )
            .await?;

        let session_id = result
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcpError::Protocol("session/new response missing 'sessionId'".into()))?
            .to_string();

        info!(acp_session_id = %session_id, "ACP session created");
        Ok(session_id)
    }

    /// Send a prompt to the ACP session.
    ///
    /// ACP spec: `session/prompt` is a request (has `id`). The response carries
    /// the `stopReason` when the turn finishes.  We spawn the request in a
    /// background task so the caller is not blocked – streaming updates arrive
    /// via `session/update` notifications on the notification channel.
    pub async fn send_prompt(
        self: &Arc<Self>,
        session_id: &str,
        text: &str,
    ) -> Result<(), AcpError> {
        let params = serde_json::json!({
            "sessionId": session_id,
            "prompt": [
                {
                    "type": "text",
                    "text": text
                }
            ]
        });
        let client = Arc::clone(self);
        let method = "session/prompt".to_string();
        let notif_tx = client.notification_tx.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            match client.send_request(&method, Some(params)).await {
                Ok(result) => {
                    debug!("session/prompt completed");
                    let stop_reason = result
                        .get("stopReason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("end_turn")
                        .to_string();
                    let synthetic = AcpMessage::Notification(JsonRpcNotification {
                        jsonrpc: "2.0".to_string(),
                        method: "session/update".to_string(),
                        params: Some(serde_json::json!({
                            "sessionId": sid,
                            "update": {
                                "sessionUpdate": "turn_complete",
                                "stopReason": stop_reason
                            }
                        })),
                    });
                    let _ = notif_tx.send(synthetic).await;
                }
                Err(e) => warn!(error = %e, "session/prompt request failed"),
            }
        });
        Ok(())
    }

    /// Send a cancel signal to the ACP session.
    pub async fn cancel(&self, session_id: &str) -> Result<(), AcpError> {
        self.send_notification(
            "session/cancel",
            Some(serde_json::json!({
                "sessionId": session_id
            })),
        )
        .await
    }

    /// Respond to an elicitation request.
    pub async fn respond_elicitation(
        &self,
        request_id: &str,
        action: &str,
        content: Option<Value>,
    ) -> Result<(), AcpError> {
        let mut params = serde_json::json!({
            "requestId": request_id,
            "action": action
        });

        if let Some(c) = content {
            params["content"] = c;
        }

        // Elicitation response is a JSON-RPC response to the pending elicitation request
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::String(request_id.to_string())),
            result: Some(params),
            error: None,
        };

        let line = serde_json::to_string(&response)
            .map_err(|e| AcpError::Protocol(format!("Failed to serialize response: {}", e)))?;

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| AcpError::ProcessExited)?;

        Ok(())
    }

    /// Receive the next notification from the ACP subprocess.
    /// Returns `None` when the subprocess has exited and all buffered messages are consumed.
    pub async fn recv_notification(&self) -> Option<AcpMessage> {
        let mut rx = self.notification_rx.lock().await;
        let result = rx.recv().await;
        if let Some(AcpMessage::Notification(ref notif)) = result {
            debug!(method = %notif.method, "recv_notification received");
        }
        result
    }

    /// Kill the ACP subprocess.
    pub async fn kill(&self) {
        let mut child = self.child.lock().await;
        if let Some(ref mut c) = *child {
            match c.kill().await {
                Ok(()) => info!("ACP subprocess killed"),
                Err(e) => warn!(error = %e, "Failed to kill ACP subprocess"),
            }
        }
        *child = None;
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, AcpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(id.into())),
            method: method.to_string(),
            params,
        };

        let line = serde_json::to_string(&request)
            .map_err(|e| AcpError::Protocol(format!("Failed to serialize request: {}", e)))?;

        debug!(method = %method, id = %id, "Sending ACP request");

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| AcpError::ProcessExited)?;

        // Wait for the response with matching ID
        let mut rx = self.response_rx.lock().await;
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
                Ok(Some(resp)) => {
                    if resp.id == Some(Value::Number(id.into())) {
                        if let Some(err) = resp.error {
                            return Err(AcpError::RpcError {
                                code: err.code,
                                message: err.message,
                            });
                        }
                        return Ok(resp.result.unwrap_or(Value::Null));
                    }
                    // Not our response; this shouldn't happen in practice with
                    // a single-consumer pattern, but we handle it gracefully.
                    debug!(
                        expected_id = %id,
                        got_id = ?resp.id,
                        "Received response with unexpected ID, dropping"
                    );
                }
                Ok(None) => {
                    return Err(AcpError::ProcessExited);
                }
                Err(_) => {
                    return Err(AcpError::Timeout {
                        method: method.to_string(),
                    });
                }
            }
        }
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<(), AcpError> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let line = serde_json::to_string(&notification)
            .map_err(|e| AcpError::Protocol(format!("Failed to serialize notification: {}", e)))?;

        debug!(method = %method, "Sending ACP notification");

        self.stdin_tx
            .send(line)
            .await
            .map_err(|_| AcpError::ProcessExited)?;

        Ok(())
    }
}

// ── AcpError ──────────────────────────────────────────────────────────────

/// Errors from ACP client operations.
#[derive(Debug, thiserror::Error)]
pub enum AcpError {
    #[error("Failed to spawn ACP command '{command}': {reason}")]
    SpawnFailed { command: String, reason: String },

    #[error("ACP subprocess stdio pipe not available")]
    StdioPipeMissing,

    #[error("ACP subprocess has exited unexpectedly")]
    ProcessExited,

    #[error("ACP protocol error: {0}")]
    Protocol(String),

    #[error("ACP JSON-RPC error (code={code}): {message}")]
    RpcError { code: i64, message: String },

    #[error("Timeout waiting for ACP response to '{method}'")]
    Timeout { method: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_command_path_absolute_unchanged() {
        let result = resolve_command_path("/usr/bin/echo").await;
        assert_eq!(result.command, "/usr/bin/echo");
    }

    #[tokio::test]
    async fn test_resolve_command_path_relative_resolves() {
        let result = resolve_command_path("cat").await;
        assert!(
            result.command.starts_with('/'),
            "Expected absolute path for 'cat', got: {}",
            result.command
        );
    }

    #[tokio::test]
    async fn test_resolve_command_path_fallback() {
        let result = resolve_command_path("nonexistent-binary-xyz-12345").await;
        assert_eq!(result.command, "nonexistent-binary-xyz-12345");
    }

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_jsonrpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"id":"session-123"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(Value::Number(1.into())));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_error_response_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32600);
    }

    #[test]
    fn test_jsonrpc_notification_serialization() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: Some(serde_json::json!({"type": "agent_message_chunk", "text": "hello"})),
        };
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("\"method\":\"session/update\""));
        // Notifications should not have an "id" field
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_acp_event_deserialization() {
        let json = r#"{"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Hello"}}"#;
        let event: AcpEvent = serde_json::from_str(json).unwrap();
        match event {
            AcpEvent::AgentMessageChunk { content } => {
                assert_eq!(content.unwrap().text, "Hello");
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_acp_event_tool_call() {
        let json = r#"{"sessionUpdate": "tool_call", "toolCallId": "tc1", "title": "Read file", "kind": "read", "status": "pending"}"#;
        let event: AcpEvent = serde_json::from_str(json).unwrap();
        match event {
            AcpEvent::ToolCall {
                tool_call_id,
                title,
                ..
            } => {
                assert_eq!(tool_call_id, "tc1");
                assert_eq!(title, "Read file");
            }
            _ => panic!("Expected ToolCall"),
        }
    }

    #[test]
    fn test_acp_event_turn_complete() {
        let json = r#"{"sessionUpdate": "turn_complete", "stopReason": "end_turn"}"#;
        let event: AcpEvent = serde_json::from_str(json).unwrap();
        match event {
            AcpEvent::TurnComplete { stop_reason } => {
                assert_eq!(stop_reason, "end_turn");
            }
            _ => panic!("Expected TurnComplete"),
        }
    }

    #[test]
    fn test_acp_event_elicitation() {
        let json = r#"{"sessionUpdate": "elicitation", "requestId": "req1", "mode": "form", "message": "Choose option", "schema": {"type": "object"}}"#;
        let event: AcpEvent = serde_json::from_str(json).unwrap();
        match event {
            AcpEvent::Elicitation {
                request_id,
                mode,
                message,
                schema,
            } => {
                assert_eq!(request_id, "req1");
                assert_eq!(mode, "form");
                assert_eq!(message, "Choose option");
                assert!(schema.is_some());
            }
            _ => panic!("Expected Elicitation"),
        }
    }
}
