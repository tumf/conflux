//! TUI Dashboard for OpenSpec Orchestrator
//!
//! This module provides an interactive terminal user interface for:
//! - Selecting changes to process
//! - Monitoring execution progress
//! - Dynamic queue management
//! - Auto-refresh of change list

mod command_handlers;
mod events;
mod key_handlers;
pub mod log_deduplicator;
mod orchestrator;
mod qr;
pub mod queue;
mod render;
mod runner;
mod state;
mod terminal;
mod type_impls;
pub mod types;
mod utils;
mod worktrees;

// Public API re-exports
pub use runner::run_tui;
