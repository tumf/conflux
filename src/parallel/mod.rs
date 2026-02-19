//! Parallel execution coordinator for VCS workspace-based parallel change application.
//!
//! This module is the entry point for the parallel execution subsystem. It re-exports
//! the public API and declares the submodules, each with a focused responsibility:
//!
//! - [`builder`]: Construction and initialization of [`ParallelExecutor`]
//! - [`queue_state`]: Queue state management, debounce logic, and dispatch coordination
//! - [`executor`]: Apply/acceptance/archive command execution in workspaces
//! - [`merge`]: Branch merge and conflict resolution
//! - [`orchestration`]: Order-based re-analysis scheduler loop
//! - [`dispatch`]: Change dispatch to workspaces (apply+acceptance+archive pipeline)
//! - [`workspace`]: Workspace lifecycle management helpers
//! - [`cleanup`]: Workspace cleanup guards
//! - [`conflict`]: Conflict detection and resolution
//! - [`dynamic_queue`]: Dynamic queue (TUI mode) support
//! - [`events`]: Event sending utilities
//! - [`output_bridge`]: Output line bridging
//! - [`types`]: Shared type definitions

mod builder;
mod cleanup;
mod conflict;
mod dispatch;
mod dynamic_queue;
mod events;
mod executor;
mod merge;
mod orchestration;
mod output_bridge;
mod queue_state;
mod types;
mod workspace;

// Re-export ExecutionEvent as ParallelEvent for backward compatibility
pub use crate::events::ExecutionEvent as ParallelEvent;
pub use merge::{base_dirty_reason, resolve_deferred_merge};
pub use types::{FailedChangeTracker, WorkspaceResult};

// Re-exports used in tests via `use super::super::*`
#[cfg(test)]
pub use crate::vcs::Workspace;
#[cfg(test)]
pub use merge::MergeAttempt;

use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::config::OrchestratorConfig;
use crate::vcs::WorkspaceManager;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use crate::hooks::HookRunner;

const DEFAULT_MAX_CONFLICT_RETRIES: u32 = 3;

/// Global lock for serializing all merge/resolve operations to base branch.
///
/// This ensures that only one merge operation can modify the base branch
/// at any given time, regardless of which ParallelExecutor instance
/// initiates the operation.
static GLOBAL_MERGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Get the global merge lock, initializing it if necessary.
fn global_merge_lock() -> &'static Mutex<()> {
    GLOBAL_MERGE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Parallel executor for running changes in git worktrees
pub struct ParallelExecutor {
    /// Workspace manager (VCS-agnostic)
    workspace_manager: Box<dyn WorkspaceManager>,
    /// Configuration (used for AgentRunner and resolve operations)
    config: OrchestratorConfig,
    /// Apply command template
    apply_command: String,
    /// Archive command template
    archive_command: String,
    /// Event sender
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    /// Maximum retries for conflict resolution
    max_conflict_retries: u32,
    /// Repository root path for archive operations
    repo_root: PathBuf,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
    /// Tracker for failed changes to enable skipping dependent changes
    failed_tracker: FailedChangeTracker,
    /// Change-level dependencies (change_id -> dependency ids)
    change_dependencies: HashMap<String, Vec<String>>,
    /// Changes waiting for merge resolution
    merge_deferred_changes: HashSet<String>,
    /// Changes that previously had unresolved dependencies (for worktree recreation tracking)
    #[allow(dead_code)]
    previously_blocked_changes: HashSet<String>,
    /// Changes that need forced worktree recreation (dependency just resolved)
    force_recreate_worktree: HashSet<String>,
    /// Hook runner for executing hooks (optional)
    hooks: Option<Arc<HookRunner>>,
    /// Cancellation token for force stop cleanup
    cancel_token: Option<CancellationToken>,
    /// Last queue change timestamp for debouncing re-analysis
    last_queue_change_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Dynamic queue for runtime change additions (TUI mode)
    dynamic_queue: Option<Arc<crate::tui::queue::DynamicQueue>>,
    /// Shared AI command runner for stagger coordination
    ai_runner: AiCommandRunner,
    /// Shared stagger state for resolve operations
    #[allow(dead_code)] // Reserved for future resolve integration
    shared_stagger_state: SharedStaggerState,
    /// History of apply attempts per change for context injection
    apply_history: Arc<Mutex<crate::history::ApplyHistory>>,
    /// History of archive attempts per change for context injection
    archive_history: Arc<Mutex<crate::history::ArchiveHistory>>,
    /// History of acceptance attempts per change for context injection
    acceptance_history: Arc<Mutex<crate::history::AcceptanceHistory>>,
    /// Tracks which changes have had acceptance tail injected (to prevent re-injection)
    acceptance_tail_injected: Arc<Mutex<std::collections::HashMap<String, bool>>>,
    /// Flag to trigger re-analysis on next loop iteration
    needs_reanalysis: bool,
    /// Counter for active manual resolve operations (TUI mode)
    /// This allows manual resolves to consume parallel execution slots
    manual_resolve_count: Option<Arc<std::sync::atomic::AtomicUsize>>,
    /// Counter for active automatic resolve operations
    /// This allows automatic resolves to consume parallel execution slots
    auto_resolve_count: Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(test)]
mod tests;
