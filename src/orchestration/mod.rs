//! Shared orchestration operations for CLI and TUI modes.
//!
//! Contains acceptance, apply, archive, rejection, state, hook, and output helpers.

pub mod acceptance;
pub mod apply;
pub mod archive;
pub mod hooks;
pub mod output;
pub mod rejection;
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
pub use rejection::{
    execute_rejection_flow, handle_blocked_from_rejecting, handle_resume_apply_from_rejecting,
    has_rejection_proposal, run_rejection_review, RejectionReviewVerdict,
};
#[allow(unused_imports)]
pub use state::OrchestratorState;
