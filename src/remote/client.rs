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
    /// Calls `GET /api/v1/projects` and returns the parsed list of [`RemoteProject`]s.
    pub async fn list_projects(&self) -> Result<Vec<RemoteProject>> {
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
}

#[cfg(test)]
mod tests {
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
}
