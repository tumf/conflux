//! TUI Dashboard for OpenSpec Orchestrator
//!
//! This module provides an interactive terminal user interface for:
//! - Selecting changes to process
//! - Monitoring execution progress
//! - Dynamic queue management
//! - Auto-refresh of change list

pub mod clipboard;
mod command_handlers;
pub mod config;
mod events;
mod key_handlers;
pub mod lifecycle;
pub mod log_deduplicator;
pub(crate) mod orchestrator;
mod qr;
pub mod queue;
mod render;
pub mod run_supervisor;
mod runner;
mod state;
pub mod stop_classification;
mod terminal;
mod type_impls;
pub mod types;
mod utils;
mod worktrees;

// Public API re-exports
pub use runner::run_tui;
/// Local-shutdown surface reused by the heavy process-cleanup regressions.
///
/// The TUI's quit path is where a run-owned process group can survive a task
/// abort, so the deterministic Unix coverage for it drives this exact function
/// rather than a reimplementation of it.
#[allow(unused_imports)] // The binary target compiles this tree privately.
pub use runner::{
    shutdown_local_orchestrator_task, LocalOrchestratorShutdownOutcome,
    LOCAL_ORCHESTRATOR_SHUTDOWN_GRACE,
};

/// The TUI's periodic worktree refresh, for the regressions that must drive the
/// real entry point rather than the shared layer underneath it.
#[allow(unused_imports)] // The binary target compiles this tree privately.
pub use worktrees::load_worktrees_with_conflict_check;
