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

pub mod types;

// Re-export commonly used items for convenience.
// Note: These are currently unused but will be used by subsequent changes:
// - refactor-archive-common
// - refactor-apply-common
#[allow(unused_imports)]
pub use types::{ExecutionContext, ExecutionResult, ProgressInfo};
