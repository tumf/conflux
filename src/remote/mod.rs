//! Remote server client for TUI
//!
//! Provides HTTP and WebSocket clients for connecting the TUI to a remote
//! Conflux server. Supports bearer token authentication.

pub mod client;
pub mod mapper;
pub mod types;
pub mod ws;

#[cfg(test)]
pub mod test_helpers;

pub use client::RemoteClient;
// Re-export project URL helpers used by the binary (main.rs).
// The lib target doesn't reference them directly, so silence the warning.
#[allow(unused_imports)]
pub use client::{resolve_default_branch, resolve_project_url_and_branch};
pub use mapper::group_changes_by_project;
pub use types::RemoteStateUpdate;
