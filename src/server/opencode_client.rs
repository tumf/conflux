//! OpenCode Server HTTP/SSE client.
//!
//! Spawns a single `opencode serve` subprocess, discovers its bound URL,
//! performs session/message HTTP operations, and subscribes to server-sent
//! events for streaming updates.

use std::path::Path;
use std::sync::Arc;

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::config::ProposalSessionConfig;

/// Resolve a relative command name to an absolute path via the user's login shell.
struct ResolvedCommand {
    command: String,
    login_shell_path: Option<String>,
}

async fn resolve_command_path(command: &str) -> ResolvedCommand {
    if command.starts_with('/') {
        debug!(command = %command, "OpenCode command is absolute, skipping resolution");
        return ResolvedCommand {
            command: command.to_string(),
            login_shell_path: resolve_login_shell_path().await,
        };
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let which_arg = format!("which {}", command);

    let result = Command::new(&shell)
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
                warn!(command = %command, "Login shell returned empty path, using original");
                command.to_string()
            } else {
                info!(command = %command, resolved = %resolved, "Resolved OpenCode command via login shell");
                resolved
            }
        }
        Ok(output) => {
            warn!(
                command = %command,
                exit_code = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr).trim(),
                "Failed to resolve OpenCode command via login shell, using original"
            );
            command.to_string()
        }
        Err(e) => {
            warn!(command = %command, error = %e, "Failed to resolve OpenCode command via login shell, using original");
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
    let result = Command::new(&shell)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageWithParts {
    pub id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub parts: Vec<MessagePart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePart {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "type")]
    pub part_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub content: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusPayload {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartUpdatedPayload {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub part: Option<MessagePart>,
    #[serde(default)]
    pub delta: Option<MessagePartDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePartDelta {
    #[serde(default, rename = "type")]
    pub part_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub content: Option<Vec<Value>>,
}

#[derive(Debug, Clone)]
pub enum OpencodeEvent {
    MessagePartUpdated(MessagePartUpdatedPayload),
    SessionStatus(SessionStatusPayload),
    Unknown { event_type: String, data: Value },
}

pub struct OpencodeServer {
    child: Mutex<Option<Child>>,
    pub base_url: String,
    client: reqwest::Client,
}

impl OpencodeServer {
    pub async fn spawn(
        config: &ProposalSessionConfig,
        working_dir: &Path,
    ) -> Result<Arc<Self>, OpencodeError> {
        let resolved = resolve_command_path(&config.transport_command).await;

        info!(
            cmd = %config.transport_command,
            resolved_cmd = %resolved.command,
            cwd = %working_dir.display(),
            "Spawning OpenCode server"
        );

        let mut cmd = Command::new(&resolved.command);
        cmd.args(&config.transport_args)
            .current_dir(working_dir)
            .arg("serve")
            .arg("--port")
            .arg("0")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--print-logs")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

        if let Some(ref login_path) = resolved.login_shell_path {
            cmd.env("PATH", login_path);
        }

        for (key, value) in &config.transport_env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| OpencodeError::SpawnFailed {
            command: resolved.command.clone(),
            reason: e.to_string(),
        })?;

        let stderr = child.stderr.take().ok_or(OpencodeError::StdioPipeMissing)?;
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut base_url = None;

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(500), stderr_lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    debug!(target: "opencode_stderr", "{}", line);
                    if let Some(url) = extract_listening_url(&line) {
                        base_url = Some(url);
                        break;
                    }
                }
                Ok(Ok(None)) => break,
                Ok(Err(e)) => {
                    return Err(OpencodeError::Protocol(format!(
                        "Failed reading OpenCode stderr: {}",
                        e
                    )));
                }
                Err(_) => {
                    if let Some(status) = child.try_wait().map_err(|e| OpencodeError::Protocol(e.to_string()))? {
                        return Err(OpencodeError::Protocol(format!(
                            "OpenCode server exited before reporting listening URL: {}",
                            status
                        )));
                    }
                }
            }
        }

        let base_url = base_url.ok_or_else(|| {
            OpencodeError::Protocol("OpenCode server did not report a listening URL".to_string())
        })?;

        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| OpencodeError::Protocol(format!("Failed to build HTTP client: {}", e)))?;

        let server = Arc::new(Self {
            child: Mutex::new(Some(child)),
            base_url,
            client,
        });

        server.wait_until_healthy().await?;
        Ok(server)
    }

    async fn wait_until_healthy(&self) -> Result<(), OpencodeError> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            match self.health().await {
                Ok(resp) if resp.healthy => return Ok(()),
                Ok(_) => {}
                Err(e) => debug!(error = %e, base_url = %self.base_url, "OpenCode health check not ready yet"),
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        Err(OpencodeError::Timeout {
            operation: "health".to_string(),
        })
    }

    pub async fn health(&self) -> Result<HealthResponse, OpencodeError> {
        self.client
            .get(format!("{}/global/health", self.base_url))
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?
            .json::<HealthResponse>()
            .await
            .map_err(OpencodeError::Http)
    }

    pub async fn create_session(&self, title: Option<&str>) -> Result<Session, OpencodeError> {
        let body = serde_json::json!({
            "title": title,
        });
        self.client
            .post(format!("{}/session", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?
            .json::<Session>()
            .await
            .map_err(OpencodeError::Http)
    }

    pub async fn send_prompt_async(
        &self,
        session_id: &str,
        text: &str,
        model: Option<&str>,
        agent: Option<&str>,
    ) -> Result<(), OpencodeError> {
        let body = serde_json::json!({
            "text": text,
            "model": model,
            "agent": agent,
        });
        self.client
            .post(format!("{}/session/{}/prompt_async", self.base_url, session_id))
            .json(&body)
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?;
        Ok(())
    }

    pub async fn list_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<MessageWithParts>, OpencodeError> {
        self.client
            .get(format!("{}/session/{}/message", self.base_url, session_id))
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?
            .json::<Vec<MessageWithParts>>()
            .await
            .map_err(OpencodeError::Http)
    }

    pub async fn abort_session(&self, session_id: &str) -> Result<(), OpencodeError> {
        self.client
            .post(format!("{}/session/{}/abort", self.base_url, session_id))
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?;
        Ok(())
    }

    pub async fn subscribe_events(
        &self,
    ) -> Result<impl Stream<Item = Result<OpencodeEvent, OpencodeError>>, OpencodeError> {
        let response = self
            .client
            .get(format!("{}/event", self.base_url))
            .send()
            .await
            .map_err(OpencodeError::Http)?
            .error_for_status()
            .map_err(OpencodeError::Http)?;

        let mut byte_stream = response.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<OpencodeEvent, OpencodeError>>(256);

        tokio::spawn(async move {
            let mut buffer = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        buffer.push_str(&String::from_utf8_lossy(&chunk));

                        while let Some(separator_idx) = buffer.find("\n\n") {
                            let frame = buffer[..separator_idx].to_string();
                            buffer = buffer[separator_idx + 2..].to_string();
                            if tx.send(parse_sse_frame(&frame)).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(OpencodeError::Http(err))).await;
                        return;
                    }
                }
            }

            if !buffer.trim().is_empty() {
                let _ = tx.send(parse_sse_frame(&buffer)).await;
            }
        });

        Ok(futures_util::stream::unfold(rx, |mut rx| async {
            rx.recv().await.map(|item| (item, rx))
        }))
    }

    pub async fn kill(&self) {
        let mut child = self.child.lock().await;
        if let Some(ref mut c) = *child {
            match c.kill().await {
                Ok(()) => info!(base_url = %self.base_url, "OpenCode server killed"),
                Err(e) => warn!(error = %e, base_url = %self.base_url, "Failed to kill OpenCode server"),
            }
        }
        *child = None;
    }
}

fn extract_listening_url(line: &str) -> Option<String> {
    let marker = "listening on ";
    let index = line.find(marker)?;
    let rest = &line[index + marker.len()..];
    rest.split_whitespace().next().map(str::to_string)
}

fn parse_sse_frame(frame: &str) -> Result<OpencodeEvent, OpencodeError> {
    let mut event_type = None;
    let mut data_lines = Vec::new();

    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim().to_string());
        }
    }

    let event_type = event_type.ok_or_else(|| OpencodeError::Protocol("SSE frame missing event type".to_string()))?;
    let data = data_lines.join("\n");
    let json = if data.is_empty() {
        Value::Null
    } else {
        serde_json::from_str::<Value>(&data)
            .map_err(|e| OpencodeError::Protocol(format!("Failed to parse SSE JSON payload: {}", e)))?
    };

    match event_type.as_str() {
        "message.part.updated" => Ok(OpencodeEvent::MessagePartUpdated(
            serde_json::from_value::<MessagePartUpdatedPayload>(json).map_err(|e| {
                OpencodeError::Protocol(format!("Failed to decode message.part.updated payload: {}", e))
            })?,
        )),
        "session.status" => Ok(OpencodeEvent::SessionStatus(
            serde_json::from_value::<SessionStatusPayload>(json).map_err(|e| {
                OpencodeError::Protocol(format!("Failed to decode session.status payload: {}", e))
            })?,
        )),
        _ => Ok(OpencodeEvent::Unknown { event_type, data: json }),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OpencodeError {
    #[error("Failed to spawn OpenCode command '{command}': {reason}")]
    SpawnFailed { command: String, reason: String },

    #[error("OpenCode subprocess stdio pipe not available")]
    StdioPipeMissing,

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OpenCode protocol error: {0}")]
    Protocol(String),

    #[error("Timeout waiting for OpenCode operation '{operation}'")]
    Timeout { operation: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_listening_url() {
        let url = extract_listening_url("info: listening on http://127.0.0.1:43210");
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:43210"));
    }

    #[test]
    fn test_parse_message_part_updated_event() {
        let event = parse_sse_frame(
            "event: message.part.updated\ndata: {\"session_id\":\"s1\",\"delta\":{\"type\":\"text\",\"text\":\"hello\"}}\n",
        )
        .unwrap();

        match event {
            OpencodeEvent::MessagePartUpdated(payload) => {
                assert_eq!(payload.session_id.as_deref(), Some("s1"));
                assert_eq!(payload.delta.and_then(|d| d.text).as_deref(), Some("hello"));
            }
            _ => panic!("expected MessagePartUpdated"),
        }
    }

    #[test]
    fn test_parse_session_status_event() {
        let event = parse_sse_frame(
            "event: session.status\ndata: {\"session_id\":\"s1\",\"status\":\"completed\",\"stop_reason\":\"end_turn\"}\n",
        )
        .unwrap();

        match event {
            OpencodeEvent::SessionStatus(payload) => {
                assert_eq!(payload.session_id.as_deref(), Some("s1"));
                assert_eq!(payload.status, "completed");
                assert_eq!(payload.stop_reason.as_deref(), Some("end_turn"));
            }
            _ => panic!("expected SessionStatus"),
        }
    }
}
