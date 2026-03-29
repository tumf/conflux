//! Server daemon module for multi-project management.
//!
//! The server daemon listens for HTTP API requests to manage multiple projects
//! identified by (remote_url, branch) pairs.
//!
//! # Security Policy
//! - When binding to a non-loopback address, bearer token authentication is REQUIRED.
//! - This module deliberately does NOT reference or execute `~/.wt/setup`.
//!   The server is directory-independent and uses only its configured data_dir.

pub mod acp_client;
pub mod active_commands;
pub mod opencode_client;
pub mod api;
pub mod proposal_session;
pub mod registry;
pub mod runner;
pub mod terminal;

use std::io::Write;
use std::net::SocketAddr;

use tracing::info;

use crate::config::{ProposalSessionConfig, ServerConfig};
use crate::error::Result;

fn server_base_url(bind: &str, actual_port: u16) -> String {
    crate::web::build_access_url(bind, actual_port)
}

/// Run the server daemon.
///
/// # Arguments
/// * `config` - Resolved server configuration (bind, port, auth, data_dir, max_concurrent_total).
/// * `resolve_command` - Top-level resolve_command from global config (used for auto_resolve).
pub async fn run_server(
    config: ServerConfig,
    resolve_command: Option<String>,
    proposal_session_config: ProposalSessionConfig,
) -> Result<()> {
    // Validate config (enforces auth for non-loopback binds, and rejects deprecated server.resolve_command).
    config.validate()?;

    // Build the shared registry.
    let registry = registry::create_shared_registry(&config.data_dir, config.max_concurrent_total)?;

    // In-memory runners map (per-project execution).
    let runners = runner::create_shared_runners();

    // Build app state.
    // Resolve the effective auth token: token_env takes precedence over token.
    let auth_token = match &config.auth.mode {
        crate::config::ServerAuthMode::BearerToken => config.auth.resolve_token(),
        crate::config::ServerAuthMode::None => None,
    };

    // Create log broadcast channel for streaming execution logs to WebSocket clients.
    let (log_tx, _) = tokio::sync::broadcast::channel(crate::server::api::SERVER_LOG_BUFFER_SIZE);

    let proposal_session_manager =
        proposal_session::create_proposal_session_manager(proposal_session_config);

    let app_state = api::AppState {
        registry,
        runners,
        auth_token,
        max_concurrent_total: config.max_concurrent_total,
        resolve_command,
        log_tx,
        orchestration_status: std::sync::Arc::new(tokio::sync::RwLock::new(
            registry::OrchestrationStatus::default(),
        )),
        terminal_manager: terminal::create_terminal_manager(),
        active_commands: active_commands::create_shared_active_commands(),
        proposal_session_manager,
    };

    // Build router.
    let router = api::build_router(app_state.clone());

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

    let actual_addr = listener.local_addr().map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to resolve bound address: {}",
            e
        )))
    })?;

    let url = server_base_url(&config.bind, actual_addr.port());

    // Print the accessible base URL to stdout for easy copy-paste.
    // Keep this output minimal (URL only) to support scripts.
    println!("{}", url);
    let _ = std::io::stdout().flush();

    info!("Server daemon listening on {}", url);

    // Start proposal session inactivity timeout scanner
    let psm_for_scanner = app_state.proposal_session_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut manager = psm_for_scanner.write().await;
            manager.scan_timeouts().await;
        }
    });

    // Run the server with graceful shutdown support
    let psm_for_shutdown = app_state.proposal_session_manager.clone();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("Shutdown signal received, cleaning up proposal sessions...");
        let mut manager = psm_for_shutdown.write().await;
        manager.cleanup_all(None).await;
        info!("Proposal session cleanup complete");
    });

    server.await.map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!("Server error: {}", e)))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::server_base_url;

    #[test]
    fn test_server_base_url_localhost() {
        let url = server_base_url("127.0.0.1", 39876);
        assert_eq!(url, "http://localhost:39876");

        let url = server_base_url("localhost", 39876);
        assert_eq!(url, "http://localhost:39876");
    }

    #[test]
    fn test_server_base_url_specific_address() {
        let url = server_base_url("192.168.1.50", 9000);
        assert_eq!(url, "http://192.168.1.50:9000");
    }

    #[test]
    fn test_server_base_url_zero_address_does_not_expose_zeros() {
        let url = server_base_url("0.0.0.0", 8080);
        assert!(url.starts_with("http://"));
        assert!(url.ends_with(":8080"));
        assert!(!url.contains("0.0.0.0"));
    }
}
