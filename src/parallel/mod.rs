//! Parallel execution coordinator for VCS workspace-based parallel change application.
//!
//! This module is the entry point for the parallel execution subsystem. It defines the
//! shared state container (`ParallelExecutor`) and re-exports the public API.
//!
//! Implementation is split into focused submodules:
//! - `builder`: construction and initialization
//! - `queue_state`: queue management and dispatch coordination
//! - `executor`: apply/acceptance/archive execution in workspaces
//! - `merge`: branch merge and conflict resolution
//! - `dispatch`: per-change dispatch logic
//! - `orchestration`: order-based re-analysis scheduler loop

pub(crate) mod acceptance_state;
pub(crate) mod analysis_signature;
mod archive_state;
mod builder;
mod cleanup;
mod conflict;
pub(crate) mod dedup;
mod dependency;
mod dispatch;
mod dynamic_queue;
mod events;
mod executor;
mod merge;
mod orchestration;
mod output_bridge;
pub(super) mod queue_state;
mod types;
pub(crate) mod upstream_bridge;
mod upstream_lane;
mod workspace;

// Re-export unified event type as ParallelEvent for backward compatibility.
pub use crate::events::ExecutionEvent as ParallelEvent;

#[cfg(all(test, feature = "heavy-tests"))]
#[allow(unused_imports)]
pub use merge::{base_dirty_reason, resolve_deferred_merge};
pub use types::{
    FailedChangeTracker, MergeResult, MergeResultOrigin, MergeTaskOutcome, WorkspaceResult,
};

// Re-exports used in tests via `use super::super::*`.
#[cfg(test)]
pub use crate::vcs::Workspace;
#[cfg(all(test, feature = "heavy-tests"))]
#[allow(unused_imports)]
pub use merge::MergeAttempt;

use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::config::OrchestratorConfig;
use crate::hooks::HookRunner;
use crate::parallel::analysis_signature::{
    AnalysisInputProbe, BoundedAnalysisRetry, CompletedAnalysisInput,
};
use crate::parallel::dedup::{DiagnosticDeduplicationKey, DiagnosticDeduplicationStore};
use crate::vcs::WorkspaceManager;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::orchestration::state::OrchestratorState;

type DependencyBlockerFingerprint = Vec<(String, String)>;

const DEFAULT_MAX_CONFLICT_RETRIES: u32 = 3;

/// Defines when the parallel scheduler should terminate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerLifetime {
    /// Finite execution (CLI `run`): stop once no queued/in-flight work remains.
    Finite,
    /// Persistent execution (loop-based/TUI): keep waiting for queue notifications until stopped.
    Persistent,
}

/// Terminal action after a change has been archived in parallel mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PostArchiveAction {
    #[default]
    MergeToBase,
    PushToRemote {
        remote: String,
    },
}

/// Global lock for serializing all merge/resolve operations to base branch.
///
/// This ensures that only one merge operation can modify the base branch
/// at any given time, regardless of which `ParallelExecutor` instance initiates it.
static GLOBAL_MERGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Runtime-only set of post-archive merge tasks currently owning a change.
///
/// This is intentionally not durable workflow state. It only prevents the live
/// scheduler from redispatching the same archived dirty workspace while an
/// already-spawned post-archive merge task is still deriving the authoritative
/// outcome from repository/workspace git state.
static ACTIVE_POST_ARCHIVE_MERGES: OnceLock<StdMutex<HashSet<String>>> = OnceLock::new();

/// Get the global merge lock, initializing it if necessary.
fn global_merge_lock() -> &'static Mutex<()> {
    GLOBAL_MERGE_LOCK.get_or_init(|| Mutex::new(()))
}

/// Test-only serialization mutex for suites that manipulate the
/// [`global_merge_lock`] directly (acquiring it to exercise contention paths).
///
/// Such tests race against each other when the harness runs them concurrently:
/// one test holding the global lock makes another's `try_lock` fail. All tests
/// that touch the global merge lock must hold this mutex first so they run
/// serially regardless of which module they live in.
#[cfg(test)]
pub(crate) fn merge_lock_test_mutex() -> &'static Mutex<()> {
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_MUTEX.get_or_init(|| Mutex::new(()))
}

fn active_post_archive_merges() -> &'static StdMutex<HashSet<String>> {
    ACTIVE_POST_ARCHIVE_MERGES.get_or_init(|| StdMutex::new(HashSet::new()))
}

/// Parallel executor for running changes in VCS workspaces (git worktrees today).
///
/// All execution logic lives in submodules as `impl ParallelExecutor` blocks.
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
    /// Consume a resumable acceptance stall after resolving its workspace.
    explicit_retry: bool,
    /// Override for the acceptance stall state root.
    ///
    /// `None` uses Conflux's XDG state area. Tests set an isolated temporary
    /// root so runtime stall state never leaks into a developer's real state
    /// directory and concurrent tests cannot see each other's holds.
    acceptance_stall_state_root: Option<PathBuf>,
    /// Tracker for failed changes to enable skipping dependent changes
    failed_tracker: FailedChangeTracker,
    /// Change-level dependencies (change_id -> dependency ids)
    change_dependencies: HashMap<String, Vec<String>>,
    /// Changes waiting for auto-resumable resolve retry (ResolveWait)
    resolve_wait_changes: HashSet<String>,
    /// Changes waiting for rejection review to run once the base-mutating lane is free (RejectWait)
    reject_wait_changes: HashSet<String>,
    /// Changes waiting for manual user intervention before merge can continue (MergeWait)
    merge_wait_changes: HashSet<String>,
    /// Last emitted dependency blocker fingerprint per change.
    ///
    /// This runtime-only observability state drives blocked/resolved diagnostics and
    /// worktree recreation after dependency resolution. It MUST NOT decide dispatch
    /// eligibility; `select_changes_for_dispatch` still derives executable work from
    /// the current analysis result and repository/workspace state on every pass.
    dependency_blocker_fingerprints: HashMap<String, DependencyBlockerFingerprint>,
    /// Changes that need forced worktree recreation (dependency just resolved)
    force_recreate_worktree: HashSet<String>,
    /// Hook runner for executing hooks (optional)
    hooks: Option<Arc<HookRunner>>,
    /// Cancellation token for force stop cleanup
    cancel_token: Option<CancellationToken>,
    /// Last queue change timestamp for debouncing re-analysis
    last_queue_change_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Last observed number of available execution slots.
    ///
    /// Used to bypass queue-edit debounce when capacity recovers from zero to positive,
    /// so queued changes dispatch immediately after a running task or manual resolve frees a slot.
    last_available_slots: Option<usize>,
    /// Dynamic queue for runtime change additions (TUI mode)
    dynamic_queue: Option<Arc<crate::tui::queue::DynamicQueue>>,
    /// Shared AI command runner for stagger coordination
    ai_runner: AiCommandRunner,
    /// Shared stagger state for resolve operations
    #[allow(dead_code)]
    shared_stagger_state: SharedStaggerState,
    /// History of apply attempts per change for context injection
    apply_history: Arc<Mutex<crate::history::ApplyHistory>>,
    /// History of archive attempts per change for context injection
    archive_history: Arc<Mutex<crate::history::ArchiveHistory>>,
    /// History of acceptance attempts per change for context injection
    acceptance_history: Arc<Mutex<crate::history::AcceptanceHistory>>,
    /// Tracks which changes have had acceptance tail injected (to prevent re-injection)
    acceptance_tail_injected: Arc<Mutex<std::collections::HashMap<String, bool>>>,
    /// Counter for active manual resolve operations (TUI mode)
    manual_resolve_count: Option<Arc<std::sync::atomic::AtomicUsize>>,
    /// Counter for active automatic resolve operations
    auto_resolve_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Counter for background merge tasks that have been spawned but not yet handled by scheduler.
    pending_merge_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Scheduler lifetime policy (finite for CLI run, persistent for loop-based frontends).
    scheduler_lifetime: SchedulerLifetime,
    /// Post-archive terminal action.
    post_archive_action: PostArchiveAction,
    /// Optional reducer shared state used for scheduler-owned resolve/merge retry intent.
    shared_orchestrator_state: Option<Arc<RwLock<OrchestratorState>>>,
    /// Last resolve-wait snapshot that was dispatched via scheduler retry.
    last_dispatched_resolve_wait_changes: HashSet<String>,
    /// Last reject-wait snapshot that was dispatched via scheduler retry.
    last_dispatched_reject_wait_changes: HashSet<String>,
    /// One-shot flag to allow retry dispatch on explicit wake/completion triggers.
    resolve_wait_retry_triggered: bool,
    /// Last scheduler-observed base dirtiness while reducer-owned base-lane waiters existed.
    ///
    /// This is runtime-only dedupe state. It is not durable workflow state; retry routing is
    /// recalculated from reducer state plus current base git/workspace state on each scheduler run.
    last_resolve_wait_base_dirty: Option<bool>,
    /// Runtime-only observability dedupe for operator-visible diagnostics.
    ///
    /// This state is intentionally in-memory and MUST NOT participate in scheduling decisions.
    diagnostic_dedup: DiagnosticDeduplicationStore<DiagnosticDeduplicationKey>,
    /// Last dependency-analysis input that completed with a usable result.
    ///
    /// Ordinary timer-driven analysis is skipped while the current input still matches this
    /// record, which is what stops an unchanged scheduler state from relaunching costly
    /// analysis agents forever. It is deliberately process-local: it is never persisted, so a
    /// restart always performs an initial analysis, and it never authorizes dispatch — the
    /// previous `AnalysisResult` is not retained.
    last_completed_analysis_input: Option<CompletedAnalysisInput>,
    /// Earliest instant at which a suppressed input may probe repository-visible signature
    /// material again.
    ///
    /// The scheduler wakes every 500 ms; without this bound each wake would re-read proposal
    /// files and spawn a VCS revision subprocess just to confirm nothing changed.
    next_analysis_signature_probe_at: Option<tokio::time::Instant>,
    /// Bounded retry deadline for an ordinary timer evaluation that established no completed
    /// input.
    ///
    /// This is deliberately separate from `last_completed_analysis_input`: a failed signature
    /// probe or an unusable analyzer result proves nothing was analyzed, so it must never be
    /// recorded as a completion. It only rate-limits the retry, so fail-open stays open without
    /// becoming a 500 ms probe/analyzer loop.
    analysis_retry_throttle: Option<BoundedAnalysisRetry>,
    /// Invocation-scoped upstream integration coordinator.
    ///
    /// `None` is the hard compatibility boundary: no checkpoint runs, no
    /// upstream fetch/merge/verification/push happens, and no upstream lifecycle
    /// evidence is emitted. It is installed only from an explicit `cflx run`
    /// `-u`/`--integrate-upstream` invocation, never from persistent config.
    upstream: Option<Arc<Mutex<crate::upstream::UpstreamCoordinator>>>,
    /// Override for dependency-analysis signature probing.
    ///
    /// `None` uses the real repository probe (VCS revision plus
    /// `openspec/changes/<id>/proposal.md` content). Tests inject a deterministic double so
    /// suppression coverage does not depend on real VCS subprocesses, and so probe counts and
    /// probe failures can be observed directly.
    analysis_input_probe: Option<Arc<dyn AnalysisInputProbe>>,
}

#[cfg(test)]
mod tests;
