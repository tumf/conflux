//! HTTP client for the remote Conflux server
//!
//! Provides GET/POST operations with bearer token authentication.

use std::future::Future;

use crate::error::{OrchestratorError, Result};

use super::types::RemoteProject;

// ─────────────────────────────── URL parsing ──────────────────────────────

/// Parse a project URL into `(base_url, Option<branch>)`.
///
/// Supports two branch-embedding conventions:
/// - `/tree/<branch>` path suffix (GitHub tree URLs)
/// - `#<branch>` URL fragment
///
/// The returned `base_url` is the normalized repository root URL (no trailing
/// slash, no branch suffix/fragment).
///
/// # Examples
///
/// ```ignore
/// let (url, branch) = parse_project_url("https://github.com/org/repo/tree/develop");
/// assert_eq!(url, "https://github.com/org/repo");
/// assert_eq!(branch, Some("develop".to_string()));
///
/// let (url, branch) = parse_project_url("https://github.com/org/repo#main");
/// assert_eq!(url, "https://github.com/org/repo");
/// assert_eq!(branch, Some("main".to_string()));
///
/// let (url, branch) = parse_project_url("https://github.com/org/repo");
/// assert_eq!(url, "https://github.com/org/repo");
/// assert_eq!(branch, None);
/// ```
pub fn parse_project_url(url: &str) -> (String, Option<String>) {
    // Handle fragment (#branch) first
    let (url_no_frag, frag_branch) = match url.split_once('#') {
        Some((base, frag)) if !frag.is_empty() => (base, Some(frag.to_string())),
        _ => (url, None),
    };

    // Handle /tree/<branch> path segment
    if let Some(branch) = extract_tree_branch(url_no_frag) {
        // Remove the "/tree/<branch>" suffix from the URL
        let suffix_len = "/tree/".len() + branch.len();
        let base = url_no_frag[..url_no_frag.len() - suffix_len]
            .trim_end_matches('/')
            .to_string();
        return (base, Some(branch));
    }

    (url_no_frag.trim_end_matches('/').to_string(), frag_branch)
}

/// Extract the branch name from a `/tree/<branch>` path segment.
///
/// Only matches `/tree/` that appears after at least three slash-separated
/// path components beyond the scheme (i.e., after `host/org/repo`).
fn extract_tree_branch(url: &str) -> Option<String> {
    // Skip the scheme (e.g., "https://")
    let scheme_end = url.find("://")? + 3;
    let after_scheme = &url[scheme_end..];

    // Walk through the path counting slashes to reach the 3rd one
    // (which marks the boundary between "repo" and further sub-paths)
    let mut slash_count = 0;
    for (i, c) in after_scheme.char_indices() {
        if c == '/' {
            slash_count += 1;
            if slash_count == 3 {
                let suffix = &after_scheme[i..]; // starts with "/"
                if let Some(branch_part) = suffix.strip_prefix("/tree/") {
                    // Branch name ends at the next "/" (or end of string)
                    let branch = branch_part.split('/').next().unwrap_or("").to_string();
                    if !branch.is_empty() {
                        return Some(branch);
                    }
                }
                return None;
            }
        }
    }
    None
}

// ─────────────────────────────── Default branch resolution ────────────────

/// Resolve the default branch for the given remote URL using `git ls-remote --symref`.
///
/// Runs `git ls-remote --symref <url> HEAD` and parses the symbolic ref line.
///
/// # Errors
///
/// Returns an error if:
/// - `git` is not available
/// - the remote is unreachable
/// - no symbolic HEAD ref can be found in the output
pub async fn resolve_default_branch(url: &str) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(["ls-remote", "--symref", url, "HEAD"])
        .output()
        .await
        .map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to run git ls-remote: {}",
                e
            )))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to resolve default branch for '{}': {}",
            url,
            stderr.trim()
        ))));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Expected output line: "ref: refs/heads/main\tHEAD"
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("ref: refs/heads/") {
            let branch = rest.split('\t').next().unwrap_or("").trim().to_string();
            if !branch.is_empty() {
                return Ok(branch);
            }
        }
    }

    Err(OrchestratorError::Io(std::io::Error::other(format!(
        "Could not determine default branch for '{}': symref not found in git ls-remote output",
        url
    ))))
}

// ─────────────────────────────── Combined resolver ────────────────────────

/// Resolve the final `(base_url, branch)` pair for a `project add` invocation.
///
/// Priority:
/// 1. `explicit_branch` argument (highest – overrides everything)
/// 2. Branch embedded in `raw_url` via `/tree/<branch>` or `#<branch>`
/// 3. Default branch resolved by calling `resolver(base_url)` (lowest)
///
/// The `resolver` parameter receives the base URL as an **owned** `String` and
/// returns a `Future<Output = Result<String>>`. This makes the function testable:
/// in production code pass a closure wrapping [`resolve_default_branch`]; in tests
/// pass a closure that returns a fixed value without running `git`.
///
/// # Example
/// ```ignore
/// // Production usage
/// resolve_project_url_and_branch(&url, branch.as_deref(), |u| async move {
///     resolve_default_branch(&u).await
/// }).await?
///
/// // Test usage
/// resolve_project_url_and_branch("https://github.com/org/repo", None, |_| async {
///     Ok("main".to_string())
/// }).await?
/// ```
pub async fn resolve_project_url_and_branch<F, Fut>(
    raw_url: &str,
    explicit_branch: Option<&str>,
    resolver: F,
) -> Result<(String, String)>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Result<String>>,
{
    let (base_url, url_branch) = parse_project_url(raw_url);

    let branch = if let Some(b) = explicit_branch {
        // Explicit argument has highest priority
        b.to_string()
    } else if let Some(b) = url_branch {
        // Branch embedded in URL
        b
    } else {
        // Fall back to remote default branch resolution
        resolver(base_url.clone()).await?
    };

    Ok((base_url, branch))
}

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

    /// Add a project to the remote server.
    ///
    /// Calls `POST /api/v1/projects` with `{ remote_url, branch }` and returns
    /// the parsed JSON response body on success.
    pub async fn add_project(&self, remote_url: &str, branch: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/projects", self.base_url);
        let req = self.http.post(&url);
        let req = self.authorized(req);
        let req = req.json(&serde_json::json!({
            "remote_url": remote_url,
            "branch": branch,
        }));

        let resp = req.send().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to add project: {}",
                e
            )))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(OrchestratorError::Io(std::io::Error::other(format!(
                "Server returned {} when adding project: {}",
                status, body
            ))));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to parse server response: {}",
                e
            )))
        })?;

        Ok(json)
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

    // ── Task 3.1: parse tests for /tree/<branch> and #<branch> ──────────────

    #[test]
    fn test_parse_url_tree_branch() {
        let (url, branch) = parse_project_url("https://github.com/org/repo/tree/develop");
        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, Some("develop".to_string()));
    }

    #[test]
    fn test_parse_url_tree_branch_main() {
        let (url, branch) = parse_project_url("https://github.com/org/repo/tree/main");
        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_url_fragment_branch() {
        let (url, branch) = parse_project_url("https://github.com/org/repo#develop");
        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, Some("develop".to_string()));
    }

    #[test]
    fn test_parse_url_no_branch() {
        let (url, branch) = parse_project_url("https://github.com/org/repo");
        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, None);
    }

    #[test]
    fn test_parse_url_trailing_slash_stripped() {
        let (url, branch) = parse_project_url("https://github.com/org/repo/");
        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, None);
    }

    // ── Task 3.2: default branch used when omitted (mock resolver) ───────────

    #[tokio::test]
    async fn test_default_branch_used_when_omitted() {
        // When no branch is embedded in the URL and no explicit branch is given,
        // the resolver is called and its return value is used as the branch.
        let (url, branch) =
            resolve_project_url_and_branch("https://github.com/org/repo", None, |_url| async {
                Ok("main".to_string())
            })
            .await
            .expect("should succeed");

        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, "main");
    }

    #[tokio::test]
    async fn test_url_branch_used_when_no_explicit_branch() {
        // When a branch is embedded in the URL and no explicit branch is given,
        // the URL branch is used (resolver is NOT called).
        let (url, branch) = resolve_project_url_and_branch(
            "https://github.com/org/repo/tree/develop",
            None,
            |_url| async {
                // Should never be called when URL branch is present
                panic!("resolver should not be called when URL has a branch");
                #[allow(unreachable_code)]
                Ok(String::new())
            },
        )
        .await
        .expect("should succeed");

        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, "develop");
    }

    // ── Task 3.3: explicit branch overrides URL branch ───────────────────────

    #[tokio::test]
    async fn test_explicit_branch_overrides_url_branch() {
        // When an explicit branch is provided, it takes precedence over any
        // branch embedded in the URL.
        let (url, branch) = resolve_project_url_and_branch(
            "https://github.com/org/repo/tree/develop",
            Some("main"),
            |_url| async {
                panic!("resolver should not be called when explicit branch is given");
                #[allow(unreachable_code)]
                Ok(String::new())
            },
        )
        .await
        .expect("should succeed");

        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, "main");
    }

    #[tokio::test]
    async fn test_explicit_branch_overrides_fragment_branch() {
        let (url, branch) = resolve_project_url_and_branch(
            "https://github.com/org/repo#develop",
            Some("main"),
            |_url| async {
                panic!("resolver should not be called when explicit branch is given");
                #[allow(unreachable_code)]
                Ok(String::new())
            },
        )
        .await
        .expect("should succeed");

        assert_eq!(url, "https://github.com/org/repo");
        assert_eq!(branch, "main");
    }
}
