//! Remote server client for TUI
//!
//! Provides HTTP and WebSocket clients for connecting the TUI to a remote
//! Conflux server. Supports bearer token authentication.

pub mod client;
pub mod mapper;
pub mod types;
pub mod ws;

pub use client::RemoteClient;
pub use mapper::group_changes_by_project;
pub use types::RemoteStateUpdate;
