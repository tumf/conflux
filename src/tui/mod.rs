//! TUI Dashboard for OpenSpec Orchestrator
//!
//! This module provides an interactive terminal user interface for:
//! - Selecting changes to process
//! - Monitoring execution progress
//! - Dynamic queue management
//! - Auto-refresh of change list

mod events;
pub mod log_deduplicator;
mod orchestrator;
mod qr;
mod queue;
mod render;
mod runner;
mod state;
pub mod types;
mod utils;

// Public API re-exports
pub use runner::run_tui;
