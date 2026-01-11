//! TUI Dashboard for OpenSpec Orchestrator
//!
//! This module provides an interactive terminal user interface for:
//! - Selecting changes to process
//! - Monitoring execution progress
//! - Dynamic queue management
//! - Auto-refresh of change list

mod events;
mod orchestrator;
mod parallel_event_bridge;
mod queue;
mod render;
mod runner;
mod state;
mod types;
mod utils;

// Public API re-exports
pub use runner::run_tui;
