//! Execution module for OpenSpec Orchestrator.
//!
//! Provides common execution logic shared between serial and parallel modes.
//! This module serves as a foundation for consolidating duplicate implementations
//! of archive, apply, and progress tracking operations.
//!
//! # Module Structure
//!
//! - `types` - Common type definitions (`ExecutionContext`, `ExecutionResult`, `ProgressInfo`)
//!
//! # Future Additions
//!
//! This module will be extended with:
//! - `archive` - Common archive operation logic (from `refactor-archive-common`)
//! - `apply` - Common apply operation logic (from `refactor-apply-common`)

pub mod apply;
pub mod types;

// Re-export commonly used items for convenience.
// Note: Some items are currently unused but will be used as the refactoring progresses
// in parallel/executor.rs and other modules.
// Allow unused_imports as these are public API exports for future use.
#[allow(unused_imports)]
pub use apply::{
    check_task_progress, create_final_commit, create_progress_commit, is_progress_complete,
    progress_increased, ApplyConfig, ApplyIterationResult, DEFAULT_MAX_ITERATIONS,
};
#[allow(unused_imports)]
pub use types::{ExecutionContext, ExecutionResult, ProgressInfo};
