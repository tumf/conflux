#![allow(dead_code)]

use std::{
    collections::VecDeque,
    path::Path,
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, Mutex},
    task::JoinHandle,
    time,
};
use tracing::{debug, error, info, warn};

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, OpencodeError>;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    #[serde(default)]
    pub healthy: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    #[serde(rename = "sessionId", default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageWithParts {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub parts: Vec<Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpencodeEvent {
    MessagePartUpdated {
        session_id: Option<String>,
        part: Value,
    },
    SessionStatus {
        session_id: Option<String>,
        status: Option<String>,
    },
    Unknown {
        event_type: String,
        data: Value,
    },
}

#[allow(dead_code)]
pub struct OpencodeServer {
    child: Mutex<Option<Child>>,
    pub base_url: String,
    client: reqwest::Client,
    _stderr_task: Option<JoinHandle<()>>,
}

impl OpencodeServer {
    pub async fn spawn(command: &str, working_dir: &Path) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.arg("serve")
            .arg("--port")
            .arg("0")
            .arg("--hostname")
            .arg("127.0.0.1")
            .arg("--print-logs")
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| OpencodeError::SpawnFailed {
            command: command.to_string(),
            reason: e.to_string(),
        })?;

        let stderr = child
            .stderr
            .take()
            .ok_or(OpencodeError::MissingStderrPipe)?;

        let (url_tx, mut url_rx) = mpsc::channel::<String>(1);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    debug!(line = %trimmed, "opencode stderr");
                }
                if let Some(url) = parse_listening_url(trimmed) {
                    let _ = url_tx.send(url).await;
                }
            }
        });

        let base_url = time::timeout(Duration::from_secs(20), async {
            loop {
                match url_rx.try_recv() {
                    Ok(url) => return Ok(url),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        return Err(OpencodeError::TimedOut { phase: "spawn" });
                    }
                }

                if let Some(status) =
                    child
                        .try_wait()
                        .map_err(|e| OpencodeError::UnexpectedExit {
                            code: None,
                            message: e.to_string(),
                        })?
                {
                    return Err(OpencodeError::UnexpectedExit {
                        code: status.code(),
                        message: "process exited before listening URL was reported".to_string(),
                    });
                }

                time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .map_err(|_| OpencodeError::TimedOut { phase: "spawn" })??;

        let server = Self {
            child: Mutex::new(Some(child)),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: Some(stderr_task),
        };

        server.wait_for_health().await?;
        Ok(server)
    }

    async fn wait_for_health(&self) -> Result<()> {
        for _ in 0..30 {
            match self.health().await {
                Ok(h) if h.healthy => {
                    info!(url = %self.base_url, "OpenCode server is healthy");
                    return Ok(());
                }
                Ok(_) => time::sleep(Duration::from_millis(200)).await,
                Err(_) => time::sleep(Duration::from_millis(200)).await,
            }
        }

        Err(OpencodeError::HealthCheckFailed)
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let resp = self
            .client
            .get(format!("{}/global/health", self.base_url))
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /global/health".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /global/health".into(),
                message: e.to_string(),
            })?;

        resp.json::<HealthResponse>()
            .await
            .map_err(|e| OpencodeError::ResponseDecode {
                operation: "GET /global/health".into(),
                message: e.to_string(),
            })
    }

    pub async fn create_session(&self, title: Option<&str>) -> Result<Session> {
        let payload = match title {
            Some(title) => serde_json::json!({ "title": title }),
            None => serde_json::json!({}),
        };

        let resp = self
            .client
            .post(format!("{}/session", self.base_url))
            .json(&payload)
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session".into(),
                message: e.to_string(),
            })?;

        resp.json::<Session>()
            .await
            .map_err(|e| OpencodeError::ResponseDecode {
                operation: "POST /session".into(),
                message: e.to_string(),
            })
    }

    pub async fn send_prompt_async(
        &self,
        session_id: &str,
        text: &str,
        model: Option<&str>,
        agent: Option<&str>,
    ) -> Result<()> {
        let mut payload = serde_json::Map::new();
        payload.insert("text".to_string(), Value::String(text.to_string()));
        if let Some(model) = model {
            payload.insert("model".to_string(), Value::String(model.to_string()));
        }
        if let Some(agent) = agent {
            payload.insert("agent".to_string(), Value::String(agent.to_string()));
        }

        self.client
            .post(format!(
                "{}/session/{}/prompt_async",
                self.base_url, session_id
            ))
            .json(&Value::Object(payload))
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session/:id/prompt_async".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session/:id/prompt_async".into(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<MessageWithParts>> {
        let resp = self
            .client
            .get(format!("{}/session/{}/message", self.base_url, session_id))
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /session/:id/message".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /session/:id/message".into(),
                message: e.to_string(),
            })?;

        resp.json::<Vec<MessageWithParts>>()
            .await
            .map_err(|e| OpencodeError::ResponseDecode {
                operation: "GET /session/:id/message".into(),
                message: e.to_string(),
            })
    }

    pub async fn abort_session(&self, session_id: &str) -> Result<()> {
        self.client
            .post(format!("{}/session/{}/abort", self.base_url, session_id))
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session/:id/abort".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "POST /session/:id/abort".into(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    pub async fn subscribe_events(&self) -> Result<impl Stream<Item = OpencodeEvent>> {
        let resp = self
            .client
            .get(format!("{}/event", self.base_url))
            .send()
            .await
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /event".into(),
                message: e.to_string(),
            })?
            .error_for_status()
            .map_err(|e| OpencodeError::RequestFailed {
                operation: "GET /event".into(),
                message: e.to_string(),
            })?;

        let mut byte_stream = resp.bytes_stream();
        let (tx, rx) = mpsc::channel::<OpencodeEvent>(256);

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "failed to read SSE chunk");
                        break;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(idx) = buffer.find("\n\n") {
                    let event_block = buffer.drain(..idx + 2).collect::<String>();
                    if let Some(parsed) = parse_sse_event(&event_block) {
                        if tx.send(parsed).await.is_err() {
                            return;
                        }
                    }
                }
            }
        });

        Ok(OpencodeEventStream { rx })
    }

    pub async fn kill(&mut self) {
        let mut guard = self.child.lock().await;
        if let Some(child) = guard.as_mut() {
            if let Err(e) = child.kill().await {
                warn!(error = %e, "failed to kill opencode child process");
            }
        }
        *guard = None;
    }
}

#[allow(dead_code)]
struct OpencodeEventStream {
    rx: mpsc::Receiver<OpencodeEvent>,
}

impl Stream for OpencodeEventStream {
    type Item = OpencodeEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.rx).poll_recv(cx)
    }
}

#[allow(dead_code)]
fn parse_sse_event(raw: &str) -> Option<OpencodeEvent> {
    let mut event_type: Option<String> = None;
    let mut data_lines: VecDeque<String> = VecDeque::new();

    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("event:") {
            event_type = Some(v.trim().to_string());
        }
        if let Some(v) = line.strip_prefix("data:") {
            data_lines.push_back(v.trim().to_string());
        }
    }

    if data_lines.is_empty() {
        return None;
    }

    let data = serde_json::from_str::<Value>(&data_lines.make_contiguous().join("\n"))
        .unwrap_or(Value::Null);

    match event_type.as_deref() {
        Some("message.part.updated") => Some(OpencodeEvent::MessagePartUpdated {
            session_id: data
                .get("sessionId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            part: data.get("part").cloned().unwrap_or(Value::Null),
        }),
        Some("session.status") => Some(OpencodeEvent::SessionStatus {
            session_id: data
                .get("sessionId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            status: data
                .get("status")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        }),
        Some(other) => Some(OpencodeEvent::Unknown {
            event_type: other.to_string(),
            data,
        }),
        None => Some(OpencodeEvent::Unknown {
            event_type: "unknown".to_string(),
            data,
        }),
    }
}

#[allow(dead_code)]
fn parse_listening_url(line: &str) -> Option<String> {
    ["http://", "https://"].iter().find_map(|marker| {
        line.find(marker).map(|idx| {
            let suffix = &line[idx..];
            let end = suffix.find(char::is_whitespace).unwrap_or(suffix.len());
            suffix[..end].to_string()
        })
    })
}

#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum OpencodeError {
    #[error("Failed to spawn opencode command '{command}': {reason}")]
    SpawnFailed { command: String, reason: String },

    #[error("opencode serve stderr pipe was not available")]
    MissingStderrPipe,

    #[error("timed out while waiting for {phase}")]
    TimedOut { phase: &'static str },

    #[error("OpenCode server exited unexpectedly (code={code:?}): {message}")]
    UnexpectedExit { code: Option<i32>, message: String },

    #[error("OpenCode server failed health check")]
    HealthCheckFailed,

    #[error("HTTP request failed during {operation}: {message}")]
    RequestFailed { operation: String, message: String },

    #[error("failed to decode response for {operation}: {message}")]
    ResponseDecode { operation: String, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        process::Command,
        sync::oneshot,
    };

    async fn spawn_single_response_server(
        status: u16,
        content_type: &str,
        body: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            content_type,
            body.len(),
            body
        );

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        });

        format!("http://{}", addr)
    }

    #[test]
    fn test_parse_listening_url() {
        assert_eq!(
            parse_listening_url("listening on http://127.0.0.1:33123"),
            Some("http://127.0.0.1:33123".to_string())
        );
    }

    #[test]
    fn test_parse_sse_event() {
        let event = parse_sse_event(
            "event: message.part.updated\ndata: {\"sessionId\":\"s1\",\"part\":{\"text\":\"x\"}}\n\n",
        )
        .unwrap();

        match event {
            OpencodeEvent::MessagePartUpdated { session_id, part } => {
                assert_eq!(session_id.as_deref(), Some("s1"));
                assert_eq!(part.get("text").and_then(Value::as_str), Some("x"));
            }
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn test_health() {
        let base_url =
            spawn_single_response_server(200, "application/json", r#"{"healthy":true}"#).await;
        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        let health = server.health().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_create_session() {
        let base_url = spawn_single_response_server(
            200,
            "application/json",
            r#"{"sessionId":"sess-1","title":"demo"}"#,
        )
        .await;

        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        let session = server.create_session(Some("demo")).await.unwrap();
        assert_eq!(session.id.as_deref(), Some("sess-1"));
    }

    #[tokio::test]
    async fn test_list_messages() {
        let base_url = spawn_single_response_server(
            200,
            "application/json",
            r#"[{"id":"m1","role":"assistant","parts":[{"type":"text","text":"hello"}]}]"#,
        )
        .await;

        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        let messages = server.list_messages("sess-1").await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id.as_deref(), Some("m1"));
    }

    async fn spawn_request_capture_server(
        response_status: u16,
        response_body: &'static str,
    ) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel::<String>();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let size = socket.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..size]).to_string();
            let _ = tx.send(req);

            let response = format!(
                "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_status,
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        });

        (format!("http://{}", addr), rx)
    }

    async fn spawn_sse_server(event_payload: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;

            let response_header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                event_payload.len()
            );
            socket.write_all(response_header.as_bytes()).await.unwrap();
            socket.write_all(event_payload.as_bytes()).await.unwrap();
            socket.shutdown().await.unwrap();
        });

        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn test_send_prompt_async_posts_expected_payload() {
        let (base_url, request_rx) = spawn_request_capture_server(200, "{}").await;

        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        server
            .send_prompt_async("sess-1", "hello", Some("gpt-4.1"), Some("coder"))
            .await
            .unwrap();

        let req = request_rx.await.unwrap();
        assert!(req.starts_with("POST "), "request method mismatch: {req}");
        assert!(
            req.contains("/session/sess-1/prompt_async"),
            "request path mismatch: {req}"
        );
        let separator = "\r\n\r\n";
        let req_body = req
            .find(separator)
            .map(|idx| &req[idx + separator.len()..])
            .unwrap_or_default();
        assert!(
            req_body.contains("\"text\":\"hello\""),
            "request body mismatch: {req}"
        );
        assert!(
            req_body.contains("\"model\":\"gpt-4.1\""),
            "request body missing model: {req}"
        );
        assert!(
            req_body.contains("\"agent\":\"coder\""),
            "request body missing agent: {req}"
        );
    }

    #[tokio::test]
    async fn test_abort_session_posts_expected_endpoint() {
        let (base_url, request_rx) = spawn_request_capture_server(200, "{}").await;

        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        server.abort_session("sess-1").await.unwrap();

        let req = request_rx.await.unwrap();
        assert!(req.starts_with("POST "), "request method mismatch: {req}");
        assert!(
            req.contains("/session/sess-1/abort"),
            "request path mismatch: {req}"
        );
    }

    #[tokio::test]
    async fn test_subscribe_events_streams_typed_events() {
        let base_url = spawn_sse_server(
            "event: message.part.updated\ndata: {\"sessionId\":\"sess-1\",\"part\":{\"text\":\"hello\"}}\n\n",
        )
        .await;

        let server = OpencodeServer {
            child: Mutex::new(None),
            base_url,
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        let mut stream = server.subscribe_events().await.unwrap();
        let event = stream.next().await.expect("event stream ended");

        match event {
            OpencodeEvent::MessagePartUpdated { session_id, part } => {
                assert_eq!(session_id.as_deref(), Some("sess-1"));
                assert_eq!(part.get("text").and_then(Value::as_str), Some("hello"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_kill_clears_child() {
        #[cfg(unix)]
        let command = "sh";
        #[cfg(windows)]
        let command = "cmd";

        let mut child_cmd = Command::new(command);
        #[cfg(unix)]
        child_cmd.arg("-c").arg("sleep 30");
        #[cfg(windows)]
        child_cmd.arg("/C").arg("ping -n 30 127.0.0.1 > NUL");

        let child = child_cmd.spawn().unwrap();
        let mut server = OpencodeServer {
            child: Mutex::new(Some(child)),
            base_url: "http://127.0.0.1:0".to_string(),
            client: reqwest::Client::new(),
            _stderr_task: None,
        };

        server.kill().await;

        let guard = server.child.lock().await;
        assert!(guard.is_none());
    }

    #[tokio::test]
    #[ignore = "requires local `opencode` binary in PATH"]
    async fn test_spawn_with_real_opencode_binary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut server = OpencodeServer::spawn("opencode", temp_dir.path())
            .await
            .unwrap();
        let health = server.health().await.unwrap();
        assert!(health.healthy);
        server.kill().await;
    }
}
