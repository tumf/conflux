//! Execution module for OpenSpec Orchestrator.
//!
//! Provides the common execution logic managed-worktree orchestration uses.
//! This module serves as a foundation for consolidating duplicate implementations
//! of archive, apply, and progress tracking operations.
//!
//! # Module Structure
//!
//! - `types` - Common type definitions (`ExecutionContext`, `ExecutionResult`, `ProgressInfo`)
//! - `archive` - Common archive operation logic (path verification, task completion)
//! - `apply` - Common apply operation logic (iteration control, progress commits)
//! - `index_lock` - Shared managed-worktree `index.lock` evidence primitives
//! - `index_lock_reclaim` - Same-dispatch post-quiescence `index.lock` reclamation
//! - `stage_gate` - Task-complete Apply finalization staging classification
//! - `wip_lock_retry` - Transient WIP snapshot `index.lock` retry policy
//! - `final_commit_lock_retry` - Transient final Apply commit `index.lock` retry policy

pub mod apply;
pub mod archive;
pub mod final_commit_lock_retry;
pub(crate) mod index_lock;
pub(crate) mod index_lock_reclaim;
pub mod stage_gate;
pub mod state;
pub mod types;
pub mod wip_lock_retry;

// Re-export apply items for convenience.
// Note: Some items are currently unused but will be used as the refactoring progresses
// in parallel/executor.rs and other modules.
// Allow unused_imports as these are public API exports for future use.
#[allow(unused_imports)]
pub use apply::{
    check_task_progress, create_final_commit, create_progress_commit, is_progress_complete,
    progress_increased, ApplyBudget, ApplyBudgetReservation, ApplyConfig, ApplyIterationResult,
    DEFAULT_MAX_ITERATIONS,
};

// Re-export archive items for convenience.
// Note: Some items are imported directly by modules (tui/orchestrator, parallel/executor)
// to avoid circular dependency issues and to make the import intent clearer.
#[allow(unused_imports)]
pub use archive::{
    build_archive_error_message, ensure_archive_commit, get_task_progress,
    verify_archive_completion, verify_task_completion, ArchiveVerificationResult,
};

// Re-export state items for convenience.
#[allow(unused_imports)]
pub use state::{detect_workspace_state, WorkspaceState};

// Re-export type items for convenience.
// Note: These are foundation types that are used by apply, archive, and external modules.
#[allow(unused_imports)]
pub use types::{ExecutionContext, ExecutionResult, ProgressInfo};
