//! HTTP client for the remote Conflux server
//!
//! Provides GET/POST operations with bearer token authentication.

use crate::error::{OrchestratorError, Result};

use super::types::{ProjectEntry, RemoteProject};

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

    /// Fetch the flat list of all registered projects from the remote server.
    ///
    /// Calls `GET /api/v1/projects` and returns the parsed list of [`ProjectEntry`]s.
    pub async fn list_all_projects(&self) -> Result<Vec<ProjectEntry>> {
        let url = format!("{}/api/v1/projects", self.base_url);
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

        let projects: Vec<ProjectEntry> = response.json().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to parse remote server response: {}",
                e
            )))
        })?;

        Ok(projects)
    }

    /// Trigger git sync for a project on the remote server.
    ///
    /// Calls `POST /api/v1/projects/{id}/git/sync`.
    pub async fn sync_project(&self, project_id: &str) -> Result<()> {
        let url = format!(
            "{}/api/v1/projects/{}/git/sync",
            self.base_url, project_id
        );
        let req = self.http.post(&url);
        let req = self.authorized(req);

        let resp = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to sync project '{}': {}",
                project_id, e
            )))
        })?;

        if !resp.status().is_success() {
            return Err(OrchestratorError::Io(std::io::Error::other(format!(
                "Remote server returned status {} for project sync '{}'",
                resp.status(),
                project_id
            ))));
        }

        Ok(())
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
    use super::super::test_helpers::{spawn_mock_http_server, spawn_mock_http_server_ordered};
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

    /// Verify that list_all_projects calls GET /api/v1/projects (not /api/v1/projects/state).
    #[tokio::test]
    async fn test_list_all_projects_calls_correct_endpoint() {
        let project_json = r#"[{"id":"proj-1","remote_url":"https://github.com/a/b","branch":"main","status":"idle","created_at":"2024-01-01T00:00:00Z"}]"#;
        let responses = vec![(200, project_json.to_string()), (200, "{}".to_string())];
        let (addr, mut path_rx) = spawn_mock_http_server_ordered(responses).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let projects = client.list_all_projects().await.expect("list_all_projects should succeed");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "proj-1");

        let method_path = tokio::time::timeout(
            tokio::time::Duration::from_secs(3),
            path_rx.recv(),
        )
        .await
        .expect("Timed out")
        .expect("No request captured");
        assert_eq!(method_path, "GET /api/v1/projects");
    }

    /// Verify that sync_project calls POST /api/v1/projects/{id}/git/sync.
    #[tokio::test]
    async fn test_sync_project_calls_correct_endpoint() {
        let responses = vec![(200, "{}".to_string())];
        let (addr, mut path_rx) = spawn_mock_http_server_ordered(responses).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        client.sync_project("proj-abc").await.expect("sync_project should succeed");

        let method_path = tokio::time::timeout(
            tokio::time::Duration::from_secs(3),
            path_rx.recv(),
        )
        .await
        .expect("Timed out")
        .expect("No request captured");
        assert_eq!(method_path, "POST /api/v1/projects/proj-abc/git/sync");
    }

    /// Verify that list_all_projects is called before sync_project when syncing all projects.
    /// Task 3.2: GET /api/v1/projects precedes POST /api/v1/projects/{id}/git/sync.
    #[tokio::test]
    async fn test_list_then_sync_ordering() {
        let project_json = r#"[{"id":"proj-1","remote_url":"https://github.com/a/b","branch":"main","status":"idle","created_at":"2024-01-01T00:00:00Z"},{"id":"proj-2","remote_url":"https://github.com/c/d","branch":"dev","status":"idle","created_at":"2024-01-01T00:00:00Z"}]"#;
        let responses = vec![
            (200, project_json.to_string()),    // GET /api/v1/projects
            (200, "{}".to_string()),             // POST .../proj-1/git/sync
            (200, "{}".to_string()),             // POST .../proj-2/git/sync
        ];
        let (addr, mut path_rx) = spawn_mock_http_server_ordered(responses).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let projects = client.list_all_projects().await.expect("list should succeed");
        assert_eq!(projects.len(), 2);
        for project in &projects {
            client.sync_project(&project.id).await.expect("sync should succeed");
        }

        let first = tokio::time::timeout(tokio::time::Duration::from_secs(3), path_rx.recv())
            .await.expect("timeout").expect("no msg");
        let second = tokio::time::timeout(tokio::time::Duration::from_secs(3), path_rx.recv())
            .await.expect("timeout").expect("no msg");
        let third = tokio::time::timeout(tokio::time::Duration::from_secs(3), path_rx.recv())
            .await.expect("timeout").expect("no msg");

        assert_eq!(first, "GET /api/v1/projects");
        assert_eq!(second, "POST /api/v1/projects/proj-1/git/sync");
        assert_eq!(third, "POST /api/v1/projects/proj-2/git/sync");
    }

    /// Verify that sync_project returns an error when the server responds with a non-200 status.
    /// Task 3.3: failure path used to verify non-zero exit code logic.
    #[tokio::test]
    async fn test_sync_project_error_on_non_200() {
        let responses = vec![(500, r#"{"error":"internal error"}"#.to_string())];
        let (addr, _) = spawn_mock_http_server_ordered(responses).await;
        let client = RemoteClient::new(format!("http://{}", addr), None);

        let result = client.sync_project("failing-project").await;
        assert!(result.is_err(), "sync_project should return Err on 500 response");
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
}
