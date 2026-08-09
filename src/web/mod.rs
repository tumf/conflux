//! Web monitoring module for Conflux.
//!
//! Serves the embedded `/api/v2` operator console and the versioned
//! remote-control API for one running process. There is no second, unversioned
//! browser contract: the console is a first-class v2 client, so bearer
//! authentication, exact-origin policy, optimistic revisions, idempotency, and
//! typed errors apply to the browser exactly as they do to any other controller.

mod url;

#[cfg(feature = "web-monitoring")]
pub mod openapi;
#[cfg(feature = "web-monitoring")]
pub mod operator_facts;
#[cfg(feature = "web-monitoring")]
pub mod remote_control_api;
#[cfg(feature = "web-monitoring")]
pub mod state;
#[cfg(all(unix, feature = "web-monitoring"))]
pub mod unix_socket;

#[cfg(all(test, unix, feature = "web-monitoring"))]
mod listener_tests;

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
use tower_http::trace::TraceLayer;
#[cfg(feature = "web-monitoring")]
use tracing::{debug, info};

#[cfg(feature = "web-monitoring")]
pub use state::WebState;

pub use url::build_access_url;

/// Web server configuration
#[derive(Debug, Clone)]
pub struct WebConfig {
    /// Whether the browser-facing TCP listener participates.
    ///
    /// The Unix listener is configured separately, so this flag now means
    /// exactly "`--web` was supplied" and is what makes the routable-bind
    /// credential rule apply.
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

    /// Select whether the browser-facing TCP listener participates.
    pub fn with_tcp_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
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
        // The routable-bind rule guards the TCP listener specifically. A
        // UDS-only process never becomes reachable from the network, so a bind
        // address it will not use cannot make it unsafe.
        if self.enabled
            && !self.is_loopback_bind()
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

/// Serve the embedded console assets.
///
/// Static delivery carries no CORS layer of its own: these three files are the
/// whole browser surface, and everything they talk to is `/api/v2`, which
/// applies its own exact-origin policy.
#[cfg(feature = "web-monitoring")]
fn static_router() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
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
        // The runtime holds the late-bound application gate, so the router
        // serializes submissions through the same gate a keypress takes as soon
        // as an orchestration runtime binds one.
        .with_gate(runtime.gate())
        // Execution facts and scheduler liveness are late-bound with the
        // orchestration runtime that produces them, so the status resource and
        // the command path observe the same process.
        .with_execution_facts(runtime.execution_facts())
        // The runtime is both the command target and the worktree read port, so
        // both halves of the API see the same late binding.
        .with_worktrees(runtime),
    ))
}

/// Assemble the single-instance app: embedded console assets plus `/api/v2`.
#[cfg(feature = "web-monitoring")]
fn build_app(
    config: &WebConfig,
    state: Arc<WebState>,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    Ok(static_router()
        .merge(remote_control_router(config, state)?)
        .layer(TraceLayer::new_for_http()))
}

/// The full single-instance app, for route-surface tests.
#[cfg(all(test, feature = "web-monitoring"))]
pub(crate) fn build_app_for_test(config: &WebConfig, state: Arc<WebState>) -> Router {
    build_app(config, state).expect("test configuration must be valid")
}

/// Which listeners a local orchestration-owning process must start.
///
/// The Unix listener is the default local API surface; TCP is the browser-facing
/// opt-in. Both are described up front so startup can be all-or-nothing: a
/// process that advertised one endpoint and silently failed the other would be
/// worse than one that refused to start.
#[cfg(feature = "web-monitoring")]
#[derive(Debug, Clone, Default)]
pub struct ListenerPlan {
    /// Path for the Unix-domain listener, or `None` when it is opted out.
    #[cfg(unix)]
    pub unix_socket: Option<std::path::PathBuf>,
    /// Whether `--web` asked for the retained TCP/Web UI listener.
    pub tcp: bool,
}

#[cfg(feature = "web-monitoring")]
impl ListenerPlan {
    /// True when no listener was requested at all.
    pub fn is_empty(&self) -> bool {
        #[cfg(unix)]
        let unix = self.unix_socket.is_none();
        #[cfg(not(unix))]
        let unix = true;
        unix && !self.tcp
    }
}

/// A running local API server: what it actually bound, and how to stop it.
///
/// Endpoints are recorded only after their listener bound, so nothing here ever
/// describes an address a client cannot reach. Dropping the handle removes the
/// owned socket even on an abrupt error path; [`ServerHandle::shutdown`] also
/// waits for the listener tasks to finish.
#[cfg(feature = "web-monitoring")]
#[derive(Debug)]
pub struct ServerHandle {
    endpoints: Vec<String>,
    tcp_url: Option<String>,
    shutdown: tokio_util::sync::CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    #[cfg(unix)]
    socket: Option<unix_socket::SocketGuard>,
}

#[cfg(feature = "web-monitoring")]
impl ServerHandle {
    /// Every endpoint whose listener completed startup, in publication order.
    pub fn endpoints(&self) -> &[String] {
        &self.endpoints
    }

    /// The browser-facing TCP URL, when `--web` started one.
    ///
    /// A `unix://` endpoint is never returned here: it is discovery information
    /// for local clients and reverse proxies, not something a browser or a QR
    /// code can open.
    pub fn tcp_url(&self) -> Option<&str> {
        self.tcp_url.as_deref()
    }

    /// Stop every listener and refresh task, then remove the owned socket.
    ///
    /// Finite `run` completion and graceful TUI termination both call this, so
    /// neither needs a second Ctrl+C to give the endpoint back.
    pub async fn shutdown(mut self) {
        self.shutdown.cancel();
        for task in std::mem::take(&mut self.tasks) {
            let _ = task.await;
        }
        #[cfg(unix)]
        if let Some(socket) = &self.socket {
            socket.release();
        }
    }
}

/// Stop every listener this failed startup transaction already started.
///
/// "Start none of them" has to mean the tasks too, not just the pathname: a
/// leaked `axum::serve` task would keep answering the endpoint it bound while
/// the caller believes nothing started. Cancelling and then awaiting is what
/// makes the failure return only once no listener is serving any more.
#[cfg(feature = "web-monitoring")]
async fn abort_started_listeners(
    shutdown: &tokio_util::sync::CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
) {
    shutdown.cancel();
    for task in tasks {
        let _ = task.await;
    }
}

/// Start every requested listener before returning, or start none of them.
///
/// Ordering is the point: the caller runs this before lifecycle adapters, AI
/// subprocesses, and orchestration, so a process that cannot serve its required
/// API never reaches the work that assumes the API is there.
#[cfg(feature = "web-monitoring")]
pub async fn start_listeners(
    config: WebConfig,
    plan: ListenerPlan,
    state: Arc<WebState>,
) -> Result<ServerHandle, Box<dyn std::error::Error + Send + Sync>> {
    // Reject an unsafe or contradictory configuration before a socket exists,
    // not after.
    config.validate()?;

    // One router, one `WebState`: every listener reaches the same projection,
    // command registry, executor binding, and authentication policy.
    let app = build_app(&config, state.clone())?;
    let shutdown = tokio_util::sync::CancellationToken::new();

    let mut endpoints = Vec::new();
    let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    #[cfg(unix)]
    let mut socket_guard = None;
    #[cfg(unix)]
    if let Some(path) = plan.unix_socket.as_deref() {
        let (listener, guard) = unix_socket::bind_unix_listener(path).await?;
        endpoints.push(unix_socket::unix_endpoint(path));
        socket_guard = Some(guard);
        info!(
            "Local API listening on {}",
            unix_socket::unix_endpoint(path)
        );

        let app = app.clone();
        let token = shutdown.clone();
        tasks.push(tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move { token.cancelled().await })
                .await
            {
                debug!("Unix API listener stopped: {}", e);
            }
        }));
    }

    let mut tcp_url = None;
    if plan.tcp {
        // A TCP failure after the socket bound must not leave a half-started
        // process behind: the rollback stops the listener task already spawned,
        // and returning then drops `socket_guard`, which removes the socket this
        // transaction created.
        // The actual address includes an OS-assigned port when 0 was requested,
        // so what gets published is what a client can actually connect to.
        let bound = async {
            let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let actual_port = listener.local_addr()?.port();
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((listener, actual_port))
        }
        .await;
        let (listener, actual_port) = match bound {
            Ok(bound) => bound,
            Err(error) => {
                abort_started_listeners(&shutdown, tasks).await;
                return Err(error);
            }
        };
        let url = build_access_url(&config.bind, actual_port);
        info!("Web monitoring server listening on {}", url);
        endpoints.push(url.clone());
        tcp_url = Some(url);

        let app = app.clone();
        let token = shutdown.clone();
        tasks.push(tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(async move { token.cancelled().await })
                .await
            {
                debug!("TCP API listener stopped: {}", e);
            }
        }));
    }

    // Every requested listener is bound, so the repository lock owner can now
    // advertise endpoints that are genuinely reachable. A no-op when this
    // process holds no lock.
    crate::repo_lock::publish_endpoints(&endpoints);

    if config.refresh_interval_secs > 0 && !endpoints.is_empty() {
        let state = state.clone();
        let interval_secs = config.refresh_interval_secs;
        let token = shutdown.clone();
        tasks.push(tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = interval.tick() => {
                        if let Err(e) = state.refresh_from_disk().await {
                            debug!("Periodic refresh failed: {}", e);
                        }
                    }
                }
            }
        }));
    }

    Ok(ServerHandle {
        endpoints,
        tcp_url,
        shutdown,
        tasks,
        #[cfg(unix)]
        socket: socket_guard,
    })
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
