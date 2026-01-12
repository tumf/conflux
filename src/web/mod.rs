//! Web monitoring module for OpenSpec Orchestrator.
//!
//! Provides an optional HTTP server with REST API and WebSocket support
//! for monitoring orchestration state remotely via web browser.

#[cfg(feature = "web-monitoring")]
pub mod api;
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
use tracing::info;

#[cfg(feature = "web-monitoring")]
pub use state::WebState;

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
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8080,
            bind: "127.0.0.1".to_string(),
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
        }
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

/// Start the web monitoring server
#[cfg(feature = "web-monitoring")]
pub async fn start_server(
    config: WebConfig,
    state: Arc<WebState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{}:{}", config.bind, config.port).parse()?;

    // CORS configuration for local development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router with API and static routes
    let app = Router::new()
        // Static file routes
        .route("/", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        // API routes
        .route("/api/health", get(api::health))
        .route("/api/state", get(api::get_state))
        .route("/api/changes", get(api::list_changes))
        .route("/api/changes/{id}", get(api::get_change))
        // WebSocket route
        .route("/ws", get(websocket::ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("Starting web monitoring server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
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
pub fn spawn_server(
    config: WebConfig,
    state: Arc<WebState>,
) -> tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    tokio::spawn(async move { start_server(config, state).await })
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
        assert_eq!(config.port, 8080);
        assert_eq!(config.bind, "127.0.0.1");
    }

    #[test]
    fn test_web_config_enabled() {
        let config = WebConfig::enabled(9000, "0.0.0.0".to_string());
        assert!(config.enabled);
        assert_eq!(config.port, 9000);
        assert_eq!(config.bind, "0.0.0.0");
    }
}
