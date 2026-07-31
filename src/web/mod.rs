//! Web monitoring module for Conflux.
//!
//! Provides an optional HTTP server with REST API and WebSocket support
//! for monitoring orchestration state remotely via web browser.

mod url;

#[cfg(feature = "web-monitoring")]
pub mod api;
#[cfg(feature = "web-monitoring")]
pub mod openapi;
#[cfg(feature = "web-monitoring")]
pub mod remote_control_api;
#[cfg(feature = "web-monitoring")]
pub mod state;
#[cfg(feature = "web-monitoring")]
pub mod websocket;

#[cfg(feature = "web-monitoring")]
use axum::{
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
#[cfg(feature = "web-monitoring")]
use std::net::SocketAddr;
#[cfg(feature = "web-monitoring")]
use std::sync::Arc;
#[cfg(feature = "web-monitoring")]
use tower_http::cors::{Any, CorsLayer};
#[cfg(feature = "web-monitoring")]
use tower_http::trace::TraceLayer;
#[cfg(feature = "web-monitoring")]
use tracing::{debug, info};

#[cfg(feature = "web-monitoring")]
pub use state::WebState;

pub use url::build_access_url;

/// Web server configuration
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Whether web monitoring is enabled
    #[allow(dead_code)]
    pub enabled: bool,
    /// Port to bind the HTTP server
    pub port: u16,
    /// Address to bind the HTTP server
    pub bind: String,
    /// Interval in seconds for periodic state refresh from disk (0 to disable)
    pub refresh_interval_secs: u64,
    /// Literal bearer token for `/api/v2`.
    ///
    /// Mutually exclusive with [`Self::auth_token_env`]. A literal value can be
    /// visible to anything that can inspect process arguments, so the
    /// environment form is preferred.
    pub auth_token: Option<String>,
    /// Name of the environment variable holding the bearer token for `/api/v2`.
    pub auth_token_env: Option<String>,
    /// Exact additional origins allowed to make cross-origin `/api/v2` requests.
    ///
    /// Exact values only: wildcards are rejected, and forwarded headers are
    /// never used to widen this list.
    pub allowed_origins: Vec<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 0, // Auto-assign by OS
            bind: "127.0.0.1".to_string(),
            refresh_interval_secs: 5, // Default: refresh every 5 seconds
            auth_token: None,
            auth_token_env: None,
            allowed_origins: Vec::new(),
        }
    }
}

impl WebConfig {
    /// Create a new WebConfig with web monitoring enabled
    pub fn enabled(port: u16, bind: String) -> Self {
        Self {
            enabled: true,
            port,
            bind,
            ..Self::default()
        }
    }

    /// Set the refresh interval
    #[allow(dead_code)]
    pub fn with_refresh_interval(mut self, secs: u64) -> Self {
        self.refresh_interval_secs = secs;
        self
    }

    /// Configure `/api/v2` bearer authentication and exact allowed origins.
    pub fn with_auth(
        mut self,
        auth_token: Option<String>,
        auth_token_env: Option<String>,
        allowed_origins: Vec<String>,
    ) -> Self {
        self.auth_token = auth_token;
        self.auth_token_env = auth_token_env;
        self.allowed_origins = allowed_origins;
        self
    }

    /// True when the bind address is a loopback address.
    pub fn is_loopback_bind(&self) -> bool {
        let host = self
            .bind
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_ascii_lowercase();
        if host == "localhost" || host == "::1" {
            return true;
        }
        host.parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
    }

    /// Resolve the effective bearer token.
    ///
    /// The environment form wins when configured, and an unset or empty variable
    /// resolves to no token rather than to the literal — otherwise a typo in the
    /// variable name would silently downgrade a deployment to the wrong secret.
    pub fn resolve_auth_token(&self) -> Option<String> {
        if let Some(var) = self.auth_token_env.as_deref() {
            return std::env::var(var).ok().filter(|value| !value.is_empty());
        }
        self.auth_token.clone().filter(|value| !value.is_empty())
    }

    /// Validate the configuration before anything binds a socket.
    ///
    /// Failing here rather than at first request is the point: a process that
    /// listens on a routable address without credentials has already been
    /// reachable, and no later check can take that back.
    pub fn validate(&self) -> Result<(), String> {
        if self.auth_token.is_some() && self.auth_token_env.is_some() {
            return Err(
                "--web-auth-token and --web-auth-token-env are mutually exclusive".to_string(),
            );
        }
        #[cfg(feature = "web-monitoring")]
        for origin in &self.allowed_origins {
            if remote_control_api::auth::normalize_origin(origin).is_none() {
                return Err(format!(
                    "invalid --web-allowed-origin '{origin}': expected an exact \
                     http(s)://host[:port] value with no wildcard or path"
                ));
            }
        }
        if !self.is_loopback_bind()
            && self
                .resolve_auth_token()
                .as_deref()
                .unwrap_or("")
                .is_empty()
        {
            return Err(format!(
                "web monitoring on non-loopback address '{}' requires \
                 --web-auth-token or --web-auth-token-env",
                self.bind
            ));
        }
        Ok(())
    }
}

/// Embedded static files
#[cfg(feature = "web-monitoring")]
mod static_files {
    pub const INDEX_HTML: &str = include_str!("../../web/index.html");
    pub const STYLE_CSS: &str = include_str!("../../web/style.css");
    pub const APP_JS: &str = include_str!("../../web/app.js");
}

#[cfg(feature = "web-monitoring")]
async fn serve_index() -> Html<&'static str> {
    Html(static_files::INDEX_HTML)
}

#[cfg(feature = "web-monitoring")]
async fn serve_css() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css")],
        static_files::STYLE_CSS,
    )
        .into_response()
}

#[cfg(feature = "web-monitoring")]
async fn serve_js() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript")],
        static_files::APP_JS,
    )
        .into_response()
}

/// Build the legacy single-instance monitoring router.
///
/// Kept as one function so both entry points serve exactly the same legacy
/// surface: `/api/*`, `/ws`, and the dashboard files must keep behaving as they
/// always have once `/api/v2` is mounted next to them.
#[cfg(feature = "web-monitoring")]
fn legacy_router(state: Arc<WebState>) -> Router {
    // Permissive CORS on the legacy surface, unchanged: the dashboard depends on
    // it. The v2 router applies its own exact-origin policy instead.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Static file routes
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        // API routes
        .route("/api/health", get(api::health))
        .route("/api/state", get(api::get_state))
        .route("/api/changes", get(api::list_changes))
        .route("/api/changes/{id}", get(api::get_change))
        // Control API routes
        .route(
            "/api/control/start",
            axum::routing::post(api::control_start),
        )
        .route("/api/control/stop", axum::routing::post(api::control_stop))
        .route(
            "/api/control/cancel-stop",
            axum::routing::post(api::control_cancel_stop),
        )
        .route(
            "/api/control/force-stop",
            axum::routing::post(api::control_force_stop),
        )
        .route(
            "/api/control/retry",
            axum::routing::post(api::control_retry),
        )
        // Worktree API routes
        .route("/api/worktrees", get(api::list_worktrees))
        .route(
            "/api/worktrees/refresh",
            axum::routing::post(api::refresh_worktrees),
        )
        .route(
            "/api/worktrees/create",
            axum::routing::post(api::create_worktree),
        )
        .route(
            "/api/worktrees/delete",
            axum::routing::post(api::delete_worktree),
        )
        .route(
            "/api/worktrees/merge",
            axum::routing::post(api::merge_worktree),
        )
        .route(
            "/api/worktrees/command",
            axum::routing::post(api::execute_worktree_command),
        )
        // WebSocket route
        .route("/ws", get(websocket::ws_handler))
        .layer(cors)
        .with_state(state)
}

/// Build the `/api/v2` router for a single-instance web server.
///
/// Returns an error when the auth/origin policy itself is unusable, so a broken
/// policy stops the process instead of quietly serving a narrower one.
#[cfg(feature = "web-monitoring")]
pub fn remote_control_router(
    config: &WebConfig,
    state: Arc<WebState>,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let auth = remote_control_api::auth::RemoteControlAuth::new(
        config.resolve_auth_token(),
        &config.allowed_origins,
    )?;
    let runtime = state.remote_control();
    Ok(remote_control_api::router(
        remote_control_api::RemoteControlState::new(
            runtime.projection(),
            Arc::new(auth),
            runtime.clone(),
        )
        // The runtime is both the command target and the worktree read port, so
        // both halves of the API see the same late binding.
        .with_worktrees(runtime),
    ))
}

/// Assemble the full single-instance app: legacy surface plus `/api/v2`.
#[cfg(feature = "web-monitoring")]
fn build_app(
    config: &WebConfig,
    state: Arc<WebState>,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    Ok(legacy_router(state.clone())
        .merge(remote_control_router(config, state)?)
        .layer(TraceLayer::new_for_http()))
}

/// The full single-instance app, for compatibility tests.
#[cfg(all(test, feature = "web-monitoring"))]
pub(crate) fn build_app_for_test(config: &WebConfig, state: Arc<WebState>) -> Router {
    build_app(config, state).expect("test configuration must be valid")
}

/// Start the web monitoring server
#[cfg(feature = "web-monitoring")]
#[allow(dead_code)]
pub async fn start_server(
    config: WebConfig,
    state: Arc<WebState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    config.validate()?;
    let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
    let app = build_app(&config, state)?;

    // Bind to the specified address (port 0 = OS auto-assign)
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Get the actual bound address (includes OS-assigned port if port was 0)
    let actual_addr = listener.local_addr()?;
    info!("Web monitoring server listening on http://{}", actual_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[cfg(feature = "web-monitoring")]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    info!("Shutting down web server...");
}

/// Start the web server in a background task (non-blocking)
#[cfg(feature = "web-monitoring")]
#[allow(dead_code)]
pub fn spawn_server(
    config: WebConfig,
    state: Arc<WebState>,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    tokio::spawn(async move { start_server(config, state).await })
}

/// Start the web server and return the accessible URL.
///
/// This function binds to the specified address, determines the actual port
/// (important when port 0 is used for auto-assignment), and constructs an
/// accessible URL that can be used for QR code generation.
///
/// Returns a tuple of (JoinHandle, accessible_url).
#[cfg(feature = "web-monitoring")]
pub async fn spawn_server_with_url(
    config: WebConfig,
    state: Arc<WebState>,
) -> Result<
    (
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
        String,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Reject an unsafe configuration before a socket exists, not after.
    config.validate()?;
    let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
    let app = build_app(&config, state.clone())?;

    // Bind to the specified address (port 0 = OS auto-assign)
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Get the actual bound address (includes OS-assigned port if port was 0)
    let actual_addr = listener.local_addr()?;
    let actual_port = actual_addr.port();

    // Build accessible URL using local IP for 0.0.0.0 bind
    let url = build_access_url(&config.bind, actual_port);
    info!("Web monitoring server listening on {}", url);

    // Spawn periodic refresh task if enabled
    if config.refresh_interval_secs > 0 {
        let state_clone = state;
        let interval = config.refresh_interval_secs;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                if let Err(e) = state_clone.refresh_from_disk().await {
                    debug!("Periodic refresh failed: {}", e);
                }
            }
        });
    }

    // Spawn the server in a background task
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    });

    Ok((handle, url))
}

// Stub implementations for when web-monitoring feature is disabled
#[cfg(not(feature = "web-monitoring"))]
pub async fn start_server(
    _config: WebConfig,
    _state: (),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("Web monitoring feature is not enabled. Compile with --features web-monitoring".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_config_default() {
        let config = WebConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.port, 0); // Auto-assign by OS
        assert_eq!(config.bind, "127.0.0.1");
    }

    #[test]
    fn test_web_config_enabled() {
        let config = WebConfig::enabled(9000, "0.0.0.0".to_string());
        assert!(config.enabled);
        assert_eq!(config.port, 9000);
        assert_eq!(config.bind, "0.0.0.0");
    }

    #[test]
    fn test_web_config_auto_assign_port() {
        // When port is 0, OS will auto-assign an available port
        let config = WebConfig::enabled(0, "127.0.0.1".to_string());
        assert!(config.enabled);
        assert_eq!(config.port, 0);
        assert_eq!(config.bind, "127.0.0.1");
    }
}
