//! HTTP client for the remote Conflux server
//!
//! Provides GET/POST operations with bearer token authentication.

use crate::error::{OrchestratorError, Result};

use super::types::RemoteProject;

/// HTTP client for connecting to a remote Conflux server.
///
/// Supports bearer token authentication via the `Authorization: Bearer <token>` header.
#[derive(Debug, Clone)]
pub struct RemoteClient {
    /// Base URL of the remote server (e.g., "http://host:9876")
    base_url: String,
    /// Optional bearer token for authentication
    token: Option<String>,
    /// Underlying HTTP client
    http: reqwest::Client,
}

impl RemoteClient {
    /// Create a new remote client.
    ///
    /// # Arguments
    /// * `base_url` – Base URL of the remote server (e.g., `"http://host:9876"`)
    /// * `token` – Optional bearer token. When `Some`, an `Authorization: Bearer <token>`
    ///   header is added to every request.
    pub fn new(base_url: impl Into<String>, token: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            token,
            http: reqwest::Client::new(),
        }
    }

    /// Resolve the bearer token from the supplied value or an environment variable.
    ///
    /// Priority:
    /// 1. `token` argument (explicit value from `--server-token`)
    /// 2. Environment variable named by `token_env` (from `--server-token-env`)
    pub fn resolve_token(token: Option<String>, token_env: Option<&str>) -> Option<String> {
        if let Some(t) = token {
            return Some(t);
        }
        if let Some(env_name) = token_env {
            return std::env::var(env_name).ok();
        }
        None
    }

    /// Add authorization header if a token is present.
    fn authorized(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.token {
            builder.header("Authorization", format!("Bearer {}", token))
        } else {
            builder
        }
    }

    /// Fetch the list of projects (and their changes) from the remote server.
    ///
    /// Calls `GET /api/v1/projects/state` and returns the parsed list of [`RemoteProject`]s.
    pub async fn list_projects(&self) -> Result<Vec<RemoteProject>> {
        let url = format!("{}/api/v1/projects/state", self.base_url);
        let req = self.http.get(&url);
        let req = self.authorized(req);

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to remote server '{}': {}",
                self.base_url, e
            )))
        })?;

        if !response.status().is_success() {
            return Err(OrchestratorError::Io(std::io::Error::other(format!(
                "Remote server returned status {}: {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown")
            ))));
        }

        let projects: Vec<RemoteProject> = response.json().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to parse remote server response: {}",
                e
            )))
        })?;

        Ok(projects)
    }

    /// Build the WebSocket URL from the base HTTP URL.
    ///
    /// Converts `http://` → `ws://` and `https://` → `wss://`.
    pub fn ws_url(&self) -> String {
        let base = self.base_url.as_str();
        if let Some(rest) = base.strip_prefix("https://") {
            format!("wss://{}/api/v1/ws", rest)
        } else if let Some(rest) = base.strip_prefix("http://") {
            format!("ws://{}/api/v1/ws", rest)
        } else {
            // Unknown scheme – append as-is, let the WS layer handle it
            format!("{}/api/v1/ws", base)
        }
    }

    /// Returns the bearer token (if any) for use when opening the WebSocket connection.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Start processing changes for a project on the remote server.
    ///
    /// Calls `POST /api/v1/projects/{id}/control/run`.
    pub async fn control_run(&self, project_id: &str, changes: Option<Vec<String>>) -> Result<()> {
        let url = format!(
            "{}/api/v1/projects/{}/control/run",
            self.base_url, project_id
        );
        let req = self.http.post(&url);
        let req = self.authorized(req);

        let req = if let Some(changes) = changes {
            req.json(&serde_json::json!({"changes": changes}))
        } else {
            req
        };

        let resp = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to call remote run control: {}",
                e
            )))
        })?;

        if !resp.status().is_success() {
            return Err(OrchestratorError::Io(std::io::Error::other(format!(
                "Remote server returned status {} for run",
                resp.status()
            ))));
        }

        Ok(())
    }

    /// Stop processing for a project on the remote server.
    ///
    /// Calls `POST /api/v1/projects/{id}/control/stop`.
    #[allow(dead_code)]
    pub async fn control_stop(&self, project_id: &str) -> Result<()> {
        let url = format!(
            "{}/api/v1/projects/{}/control/stop",
            self.base_url, project_id
        );
        let req = self.http.post(&url);
        let req = self.authorized(req);

        let resp = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to call remote stop control: {}",
                e
            )))
        })?;

        if !resp.status().is_success() {
            return Err(OrchestratorError::Io(std::io::Error::other(format!(
                "Remote server returned status {} for stop",
                resp.status()
            ))));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn test_ws_url_http() {
        let client = RemoteClient::new("http://localhost:9876", None);
        assert_eq!(client.ws_url(), "ws://localhost:9876/api/v1/ws");
    }

    #[test]
    fn test_ws_url_https() {
        let client = RemoteClient::new("https://example.com", None);
        assert_eq!(client.ws_url(), "wss://example.com/api/v1/ws");
    }

    #[test]
    fn test_ws_url_no_scheme() {
        let client = RemoteClient::new("localhost:9876", None);
        assert_eq!(client.ws_url(), "localhost:9876/api/v1/ws");
    }

    #[test]
    fn test_resolve_token_explicit() {
        let token = RemoteClient::resolve_token(Some("mytoken".to_string()), Some("SOME_ENV_VAR"));
        assert_eq!(token, Some("mytoken".to_string()));
    }

    #[test]
    fn test_resolve_token_from_env() {
        std::env::set_var("CFLX_TEST_TOKEN_12345", "env_token");
        let token = RemoteClient::resolve_token(None, Some("CFLX_TEST_TOKEN_12345"));
        assert_eq!(token, Some("env_token".to_string()));
        std::env::remove_var("CFLX_TEST_TOKEN_12345");
    }

    #[test]
    fn test_resolve_token_missing_env() {
        std::env::remove_var("CFLX_NONEXISTENT_TOKEN_99999");
        let token = RemoteClient::resolve_token(None, Some("CFLX_NONEXISTENT_TOKEN_99999"));
        assert_eq!(token, None);
    }

    #[test]
    fn test_resolve_token_none() {
        let token = RemoteClient::resolve_token(None, None);
        assert_eq!(token, None);
    }

    /// Verify that the Authorization header is correctly constructed.
    /// This test checks the internal logic rather than making a real HTTP request.
    #[test]
    fn test_client_with_token_stores_token() {
        let client = RemoteClient::new("http://localhost:9876", Some("secret123".to_string()));
        assert_eq!(client.token(), Some("secret123"));
    }

    #[test]
    fn test_client_without_token() {
        let client = RemoteClient::new("http://localhost:9876", None);
        assert_eq!(client.token(), None);
    }

    /// Spawn a minimal mock HTTP server that captures the raw request and returns 200 OK.
    ///
    /// Returns `(addr, received_request_rx)` where the receiver yields the raw HTTP
    /// request bytes sent by the client.
    async fn spawn_mock_http_server(
    ) -> (std::net::SocketAddr, tokio::sync::oneshot::Receiver<String>) {
        use tokio::io::AsyncWriteExt;
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

                // Send minimal HTTP 200 response with JSON body
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n[]";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        (addr, rx)
    }

    /// Verify that the `Authorization: Bearer <token>` header is present in the HTTP
    /// request when a token is configured.
    #[tokio::test]
    async fn test_authorization_header_sent_with_token() {
        let (addr, req_rx) = spawn_mock_http_server().await;
        let client = RemoteClient::new(
            format!("http://{}", addr),
            Some("my-secret-token".to_string()),
        );

        // list_projects() will fail (server returns "[]" which parses fine as empty vec)
        // but the HTTP request with Authorization header should have been sent
        let _ = client.list_projects().await;

        let raw_request = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out waiting for request")
            .expect("Server did not receive request");

        // HTTP header names are case-insensitive; reqwest may send lowercase "authorization"
        let raw_lower = raw_request.to_lowercase();
        assert!(
            raw_lower.contains("authorization: bearer my-secret-token"),
            "Expected 'authorization: bearer my-secret-token' in request headers, got:\n{}",
            raw_request
        );
    }

    /// Verify that NO `Authorization` header is sent when no token is configured.
    #[tokio::test]
    async fn test_no_authorization_header_without_token() {
        let (addr, req_rx) = spawn_mock_http_server().await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client.list_projects().await;

        let raw_request = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out waiting for request")
            .expect("Server did not receive request");

        let raw_lower = raw_request.to_lowercase();
        assert!(
            !raw_lower.contains("authorization:"),
            "Did not expect 'Authorization' header when no token is set, got:\n{}",
            raw_request
        );
    }
}
