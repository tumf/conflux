//! Shared orchestration logic for CLI and TUI modes.
//!
//! This module provides common implementations for:
//! - Archive operations
//! - Apply operations
//! - Acceptance test operations
//! - State management
//! - Hook context helpers
//! - Change selection logic
//!
//! Both CLI (`src/orchestrator.rs`) and TUI (`src/tui/orchestrator.rs`)
//! use these shared functions to avoid code duplication.
//!
//! Note: Some items are currently unused as CLI/TUI integration is in progress.
//! These will be used as the refactoring continues in subsequent phases.

pub mod acceptance;
pub mod apply;
pub mod archive;
pub mod hooks;
pub mod output;
pub mod rejection;
pub mod selection;
pub mod state;

// Re-exports for convenient access.
// Some exports are unused until TUI integration is complete.
#[allow(unused_imports)]
pub use acceptance::{acceptance_test_streaming, build_acceptance_tail_findings, AcceptanceResult};
#[allow(unused_imports)]
pub use apply::{apply_change, apply_change_streaming, ApplyContext, ApplyResult};
#[allow(unused_imports)]
pub use archive::{archive_change, archive_change_streaming, ArchiveContext, ArchiveResult};
#[allow(unused_imports)]
pub use output::{
    ChannelOutputHandler, ContextualOutputHandler, LogOutputHandler, OutputHandler, OutputMessage,
};
#[allow(unused_imports)]
pub use rejection::{execute_rejection_flow, has_rejection_proposal};
#[allow(unused_imports)]
pub use state::OrchestratorState;
