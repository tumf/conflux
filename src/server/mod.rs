//! Server daemon module for multi-project management.
//!
//! The server daemon listens for HTTP API requests to manage multiple projects
//! identified by (remote_url, branch) pairs.
//!
//! # Security Policy
//! - When binding to a non-loopback address, bearer token authentication is REQUIRED.
//! - This module deliberately does NOT reference or execute `~/.wt/setup`.
//!   The server is directory-independent and uses only its configured data_dir.

pub mod api;
pub mod registry;

use std::net::SocketAddr;

use tracing::info;

use crate::config::ServerConfig;
use crate::error::Result;

/// Run the server daemon.
///
/// # Arguments
/// * `config` - Resolved server configuration (bind, port, auth, data_dir, max_concurrent_total).
pub async fn run_server(config: ServerConfig) -> Result<()> {
    // Validate config (enforces auth for non-loopback binds).
    config.validate()?;

    // Build the shared registry.
    let registry = registry::create_shared_registry(&config.data_dir, config.max_concurrent_total)?;

    // Build app state.
    // Resolve the effective auth token: token_env takes precedence over token.
    let auth_token = match &config.auth.mode {
        crate::config::ServerAuthMode::BearerToken => config.auth.resolve_token(),
        crate::config::ServerAuthMode::None => None,
    };

    let app_state = api::AppState {
        registry,
        auth_token,
        max_concurrent_total: config.max_concurrent_total,
    };

    // Build router.
    let router = api::build_router(app_state);

    // Bind and serve.
    let addr: SocketAddr = format!("{}:{}", config.bind, config.port)
        .parse()
        .map_err(|e| {
            crate::error::OrchestratorError::ConfigLoad(format!(
                "Invalid server address '{}:{}': {}",
                config.bind, config.port, e
            ))
        })?;

    info!("Starting server daemon on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to bind to {}: {}",
            addr, e
        )))
    })?;

    info!("Server daemon listening on http://{}", addr);

    axum::serve(listener, router).await.map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!("Server error: {}", e)))
    })?;

    Ok(())
}
