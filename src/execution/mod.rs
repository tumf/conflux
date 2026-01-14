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
//! - `apply` - Common apply operation logic (iteration control, progress commits)

pub mod apply;
pub mod archive;
pub mod types;

// Re-export apply items for convenience.
// Note: Some items are currently unused but will be used as the refactoring progresses
// in parallel/executor.rs and other modules.
// Allow unused_imports as these are public API exports for future use.
#[allow(unused_imports)]
pub use apply::{
    check_task_progress, create_final_commit, create_progress_commit, is_progress_complete,
    progress_increased, ApplyConfig, ApplyIterationResult, DEFAULT_MAX_ITERATIONS,
};

// Re-export archive items for convenience.
// Note: Some items are imported directly by modules (tui/orchestrator, parallel/executor)
// to avoid circular dependency issues and to make the import intent clearer.
#[allow(unused_imports)]
pub use archive::{
    build_archive_error_message, ensure_archive_commit, get_task_progress,
    verify_archive_completion, verify_task_completion, ArchiveVerificationResult,
};

// Re-export type items for convenience.
// Note: These are foundation types that are used by apply, archive, and external modules.
#[allow(unused_imports)]
pub use types::{ExecutionContext, ExecutionResult, ProgressInfo};
