//! Execution module for OpenSpec Orchestrator.
//!
//! Provides common execution logic shared between serial and parallel modes.
//! This module serves as a foundation for consolidating duplicate implementations
//! of archive, apply, and progress tracking operations.
//!
//! # Module Structure
//!
//! - `types` - Common type definitions (`ExecutionContext`, `ExecutionResult`, `ProgressInfo`)
//! - `archive` - Common archive operation logic (path verification, task completion)
//!
//! # Future Additions
//!
//! This module will be extended with:
//! - `apply` - Common apply operation logic (from `refactor-apply-common`)

pub mod archive;
pub mod types;

// Re-export archive items for convenience.
// Note: Some items are imported directly by modules (tui/orchestrator, parallel/executor)
// to avoid circular dependency issues and to make the import intent clearer.
#[allow(unused_imports)]
pub use archive::{
    build_archive_error_message, get_task_progress, verify_archive_completion,
    verify_task_completion, ArchiveVerificationResult,
};

// Re-export type items for convenience.
// Note: These are foundation types that will be used by subsequent changes
// (refactor-apply-common) and external modules.
#[allow(unused_imports)]
pub use types::{ExecutionContext, ExecutionResult, ProgressInfo};
