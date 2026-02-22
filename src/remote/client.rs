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

    /// Get a specific project from the server's management API (unauthenticated).
    ///
    /// Calls `GET /api/v1/projects/{id}` and returns raw JSON value.
    pub async fn get_project(&self, project_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects/{}", self.base_url, project_id);
        let req = self.http.get(&url);
        // No authorization header – project commands are unauthenticated

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to server '{}': {}",
                self.base_url, e
            )))
        })?;

        Self::check_project_response(response).await
    }

    /// List all projects from the server's management API (unauthenticated).
    ///
    /// Calls `GET /api/v1/projects` and returns raw JSON value.
    pub async fn list_projects_management(&self) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects", self.base_url);
        let req = self.http.get(&url);
        // No authorization header – project commands are unauthenticated

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to server '{}': {}",
                self.base_url, e
            )))
        })?;

        Self::check_project_response(response).await
    }

    /// Add a project to the server (unauthenticated).
    ///
    /// Calls `POST /api/v1/projects` with `{remote_url, branch}`.
    pub async fn add_project(&self, remote_url: &str, branch: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects", self.base_url);
        let body = serde_json::json!({
            "remote_url": remote_url,
            "branch": branch,
        });
        let req = self.http.post(&url).json(&body);
        // No authorization header – project commands are unauthenticated

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to server '{}': {}",
                self.base_url, e
            )))
        })?;

        Self::check_project_response(response).await
    }

    /// Remove a project from the server (unauthenticated).
    ///
    /// Calls `DELETE /api/v1/projects/{id}`.
    pub async fn delete_project(&self, project_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects/{}", self.base_url, project_id);
        let req = self.http.delete(&url);
        // No authorization header – project commands are unauthenticated

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to server '{}': {}",
                self.base_url, e
            )))
        })?;

        Self::check_project_response(response).await
    }

    /// Trigger a git sync for a project (unauthenticated).
    ///
    /// Calls `POST /api/v1/projects/{id}/git/sync`.
    pub async fn git_sync(&self, project_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects/{}/git/sync", self.base_url, project_id);
        let req = self.http.post(&url);
        // No authorization header – project commands are unauthenticated

        let response = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to connect to server '{}': {}",
                self.base_url, e
            )))
        })?;

        Self::check_project_response(response).await
    }

    /// Common response handling for project management API calls.
    ///
    /// Returns the JSON body on success, or a formatted error for well-known HTTP status codes.
    async fn check_project_response(response: reqwest::Response) -> Result<serde_json::Value> {
        let status = response.status();
        if status.is_success() {
            // Try to parse as JSON; fall back to null if body is empty
            let text = response.text().await.unwrap_or_default();
            if text.is_empty() {
                return Ok(serde_json::Value::Null);
            }
            serde_json::from_str(&text).map_err(|e| {
                OrchestratorError::Io(std::io::Error::other(format!(
                    "Failed to parse server response: {}",
                    e
                )))
            })
        } else {
            // Attempt to extract error message from JSON body
            let text = response.text().await.unwrap_or_default();
            let detail = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                v.get("error")
                    .or_else(|| v.get("message"))
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or(text)
            } else {
                text
            };
            let label = match status.as_u16() {
                401 => "Unauthorized (401)",
                403 => "Forbidden (403)",
                404 => "Not found (404)",
                409 => "Conflict (409)",
                422 => "Unprocessable entity (422)",
                _ => status.canonical_reason().unwrap_or("Error"),
            };
            Err(OrchestratorError::Io(std::io::Error::other(format!(
                "{}: {}",
                label, detail
            ))))
        }
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
    use super::super::test_helpers::spawn_mock_http_server;
    use super::*;

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

    /// Verify that the `Authorization: Bearer <token>` header is present in the HTTP
    /// request when a token is configured.
    #[tokio::test]
    async fn test_authorization_header_sent_with_token() {
        let (addr, req_rx) = spawn_mock_http_server().await;
        let client = RemoteClient::new(
            format!("http://{}", addr),
            Some("my-secret-token".to_string()),
        );

        // list_projects() will succeed (server returns "[]" which parses as empty vec)
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

    // ── Project management API tests (unauthenticated) ────────────────────────

    /// `get_project` must GET `/api/v1/projects/:id` with NO auth header.
    #[tokio::test]
    async fn test_get_project_no_auth_header() {
        use super::super::test_helpers::spawn_flexible_mock_http_server;

        let response_json = r#"{"id":"proj-abc123","remote_url":"https://example.com/repo.git","branch":"main","status":"idle","created_at":"2024-01-01T00:00:00Z"}"#;
        let (addr, req_rx) = spawn_flexible_mock_http_server(response_json.to_string()).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client.get_project("proj-abc123").await;

        let captured = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out")
            .expect("Server did not receive request");

        assert_eq!(captured.method, "GET");
        assert_eq!(captured.path, "/api/v1/projects/proj-abc123");
        assert!(
            !captured.raw.contains("authorization:"),
            "get_project must not send Authorization header; got:\n{}",
            captured.raw
        );
    }

    /// `list_projects_management` must NOT send an Authorization header.
    #[tokio::test]
    async fn test_list_projects_management_no_auth_header() {
        use super::super::test_helpers::spawn_flexible_mock_http_server;

        let (addr, req_rx) = spawn_flexible_mock_http_server("[]".to_string()).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client.list_projects_management().await;

        let captured = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out")
            .expect("Server did not receive request");

        assert_eq!(captured.method, "GET");
        assert_eq!(captured.path, "/api/v1/projects");
        assert!(
            !captured.raw.contains("authorization:"),
            "list_projects_management must not send Authorization header; got:\n{}",
            captured.raw
        );
    }

    /// `add_project` must POST to `/api/v1/projects` with correct body and NO auth header.
    #[tokio::test]
    async fn test_add_project_no_auth_header() {
        use super::super::test_helpers::spawn_flexible_mock_http_server;

        let response_json = r#"{"id":"proj-1","remote_url":"https://example.com/repo.git","branch":"main","status":"idle","created_at":"2024-01-01T00:00:00Z"}"#;
        let (addr, req_rx) = spawn_flexible_mock_http_server(response_json.to_string()).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client
            .add_project("https://example.com/repo.git", "main")
            .await;

        let captured = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out")
            .expect("Server did not receive request");

        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/api/v1/projects");
        assert!(
            !captured.raw.contains("authorization:"),
            "add_project must not send Authorization header; got:\n{}",
            captured.raw
        );
        // Verify the request body contains remote_url and branch
        assert!(
            captured.body.contains("remote_url"),
            "Request body should contain remote_url; got: {}",
            captured.body
        );
        assert!(
            captured.body.contains("branch"),
            "Request body should contain branch; got: {}",
            captured.body
        );
    }

    /// `delete_project` must DELETE `/api/v1/projects/:id` with NO auth header.
    #[tokio::test]
    async fn test_delete_project_no_auth_header() {
        use super::super::test_helpers::spawn_flexible_mock_http_server;

        let (addr, req_rx) = spawn_flexible_mock_http_server("{}".to_string()).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client.delete_project("proj-abc123").await;

        let captured = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out")
            .expect("Server did not receive request");

        assert_eq!(captured.method, "DELETE");
        assert_eq!(captured.path, "/api/v1/projects/proj-abc123");
        assert!(
            !captured.raw.contains("authorization:"),
            "delete_project must not send Authorization header; got:\n{}",
            captured.raw
        );
    }

    /// `git_sync` must POST to `/api/v1/projects/:id/git/sync` with NO auth header.
    #[tokio::test]
    async fn test_git_sync_no_auth_header() {
        use super::super::test_helpers::spawn_flexible_mock_http_server;

        let (addr, req_rx) = spawn_flexible_mock_http_server("{}".to_string()).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let _ = client.git_sync("proj-abc123").await;

        let captured = tokio::time::timeout(tokio::time::Duration::from_secs(3), req_rx)
            .await
            .expect("Timed out")
            .expect("Server did not receive request");

        assert_eq!(captured.method, "POST");
        assert_eq!(captured.path, "/api/v1/projects/proj-abc123/git/sync");
        assert!(
            !captured.raw.contains("authorization:"),
            "git_sync must not send Authorization header; got:\n{}",
            captured.raw
        );
    }
}
