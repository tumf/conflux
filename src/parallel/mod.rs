//! Parallel execution coordinator for VCS workspace-based parallel change application.
//!
//! This module manages the parallel execution of changes using Git worktrees,
//! including workspace creation, apply command execution, merge, and cleanup.

mod cleanup;
mod conflict;
mod dynamic_queue;
mod events;
mod executor;
mod merge;
mod output_bridge;
mod types;
mod workspace;

// Re-export ExecutionEvent as ParallelEvent for backward compatibility
pub use crate::events::ExecutionEvent as ParallelEvent;
pub use dynamic_queue::ReanalysisReason;
pub use merge::{base_dirty_reason, MergeAttempt};
pub use types::{FailedChangeTracker, WorkspaceResult};

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::analyzer::ParallelGroup;
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::events::LogEntry;
use crate::execution::archive::ensure_archive_commit;
use crate::execution::state::{detect_workspace_state, WorkspaceState};
use crate::merge_stall_monitor::MergeStallMonitor;
use crate::vcs::git::commands as git_commands;
use crate::vcs::{
    GitWorkspaceManager, VcsBackend, VcsError, Workspace, WorkspaceManager, WorkspaceStatus,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use cleanup::WorkspaceCleanupGuard;
use events::send_event;
use executor::{
    execute_acceptance_in_workspace, execute_apply_in_workspace, execute_archive_in_workspace,
    ParallelHookContext,
};

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
    /// Serialize merges into the target branch.
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
    /// Flag to trigger re-analysis on next loop iteration
    needs_reanalysis: bool,
}

// Re-exported from merge module (see above pub use statement)

impl ParallelExecutor {
    /// Create a new parallel executor with automatic VCS detection
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
    ) -> Self {
        // Auto-detect VCS backend
        let vcs_backend = config.get_vcs_backend();
        Self::with_backend(repo_root, config, event_tx, vcs_backend)
    }

    /// Create a new parallel executor with a specific VCS backend and optional shared queue change timestamp
    pub fn with_backend_and_queue_state(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
        shared_queue_change: Option<Arc<Mutex<Option<std::time::Instant>>>>,
    ) -> Self {
        Self::with_backend_and_queue_and_stagger(
            repo_root,
            config,
            event_tx,
            vcs_backend,
            shared_queue_change,
            None,
        )
    }

    /// Create a new parallel executor with a specific VCS backend, optional shared queue change timestamp,
    /// and optional shared stagger state
    pub fn with_backend_and_queue_and_stagger(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
        shared_queue_change: Option<Arc<Mutex<Option<std::time::Instant>>>>,
        shared_stagger_state: Option<SharedStaggerState>,
    ) -> Self {
        // Resolve workspace base directory
        let base_dir = if let Some(configured_dir) = config.get_workspace_base_dir() {
            // User configured a specific directory
            PathBuf::from(configured_dir)
        } else {
            // Use OS-specific default workspace directory
            crate::config::defaults::default_workspace_base_dir(Some(&repo_root))
        };
        info!("Using workspace base directory: {:?}", base_dir);

        let max_concurrent = config.get_max_concurrent_workspaces();
        let apply_command = config.get_apply_command().to_string();
        let archive_command = config.get_archive_command().to_string();

        // Resolve the VCS backend (handle Auto)
        let resolved_backend = Self::resolve_backend(vcs_backend, &repo_root);
        info!("Using VCS backend: {:?}", resolved_backend);

        let workspace_manager: Box<dyn WorkspaceManager> = match resolved_backend {
            VcsBackend::Git | VcsBackend::Auto => Box::new(GitWorkspaceManager::new(
                base_dir,
                repo_root.clone(),
                max_concurrent,
                config.clone(),
            )),
        };

        let last_queue_change_at =
            shared_queue_change.unwrap_or_else(|| Arc::new(Mutex::new(None)));

        // Use provided shared stagger state or create a new one
        let shared_stagger_state =
            shared_stagger_state.unwrap_or_else(|| Arc::new(Mutex::new(None)));

        // Build CommandQueue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };

        // Create shared AI command runner
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        Self {
            workspace_manager,
            config,
            apply_command,
            archive_command,
            event_tx,
            max_conflict_retries: DEFAULT_MAX_CONFLICT_RETRIES,
            repo_root,
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            previously_blocked_changes: HashSet::new(),
            force_recreate_worktree: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at,
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
            apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
            archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
            needs_reanalysis: false,
        }
    }

    /// Create a new parallel executor with a specific VCS backend
    pub fn with_backend(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
    ) -> Self {
        Self::with_backend_and_queue_state(repo_root, config, event_tx, vcs_backend, None)
    }

    /// Set the hook runner for executing hooks during parallel execution.
    #[allow(dead_code)] // Public API for future integration with CLI/TUI
    pub fn set_hooks(&mut self, hooks: HookRunner) {
        self.hooks = Some(Arc::new(hooks));
    }

    /// Set whether to disable automatic workspace resume.
    ///
    /// When `no_resume` is true, existing workspaces are always deleted
    /// and new ones are created. When false (default), existing workspaces
    /// are reused to resume interrupted work.
    pub fn set_no_resume(&mut self, no_resume: bool) {
        self.no_resume = no_resume;
    }

    /// Set the cancellation token for force stop cleanup.
    pub fn set_cancel_token(&mut self, cancel_token: CancellationToken) {
        self.cancel_token = Some(cancel_token);
    }

    /// Set the dynamic queue for runtime change additions (TUI mode).
    pub fn set_dynamic_queue(&mut self, dynamic_queue: Arc<crate::tui::queue::DynamicQueue>) {
        self.dynamic_queue = Some(dynamic_queue);
    }

    /// Check if debounce period has elapsed for queue changes.
    ///
    /// Returns `true` if:
    /// - No recent queue changes, OR
    /// - 10 seconds have passed since the last queue change
    ///
    /// This prevents immediate re-analysis when the queue changes, giving time for
    /// multiple changes to be queued before triggering expensive re-analysis.
    ///
    /// Note: This is now separated from slot availability check. Re-analysis can
    /// proceed even when available_slots == 0, and the next dispatch will happen
    /// when slots become available.
    pub async fn should_reanalyze(&self) -> bool {
        dynamic_queue::should_reanalyze_queue(&self.last_queue_change_at).await
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
    }

    #[cfg(test)]
    fn has_merge_deferred(&self) -> bool {
        !self.merge_deferred_changes.is_empty()
    }

    fn should_skip_due_to_merge_wait(&self, change_id: &str) -> Option<String> {
        if let Some(deps) = self.change_dependencies.get(change_id) {
            for dep in deps {
                if self.merge_deferred_changes.contains(dep) {
                    return Some(dep.clone());
                }
            }
        }
        None
    }

    fn skip_reason_for_change(&self, change_id: &str) -> Option<String> {
        if let Some(failed_dep) = self.failed_tracker.should_skip(change_id) {
            return Some(format!("Dependency '{}' failed", failed_dep));
        }
        if let Some(deferred_dep) = self.should_skip_due_to_merge_wait(change_id) {
            return Some(format!("Dependency '{}' awaiting merge", deferred_dep));
        }
        None
    }

    /// Check if a dependency is resolved (merged to base branch).
    ///
    /// A dependency is considered resolved if its archive commit is present in the base branch.
    /// This indicates that the dependency's artifacts are available for dependent changes.
    async fn is_dependency_resolved(&self, dep_id: &str) -> bool {
        let original_branch = match self.workspace_manager.original_branch() {
            Some(branch) => branch,
            None => {
                warn!("Original branch not initialized, assuming dependency not resolved");
                return false;
            }
        };

        // Check if the archive commit for this dependency exists in the base branch
        match crate::execution::state::is_merged_to_base(dep_id, &self.repo_root, &original_branch)
            .await
        {
            Ok(is_merged) => is_merged,
            Err(e) => {
                warn!(
                    "Failed to check if dependency '{}' is merged to base: {}, assuming not resolved",
                    dep_id, e
                );
                false
            }
        }
    }

    /// Resolve VCS backend (convert Auto to concrete backend)
    fn resolve_backend(backend: VcsBackend, _repo_root: &Path) -> VcsBackend {
        match backend {
            VcsBackend::Auto => VcsBackend::Git,
            other => other,
        }
    }

    /// Get the VCS backend type
    #[allow(dead_code)] // Public API for external callers
    pub fn backend_type(&self) -> VcsBackend {
        self.workspace_manager.backend_type()
    }

    /// Check if VCS is available for parallel execution
    #[allow(dead_code)] // Public API, used via ParallelRunService
    pub async fn check_vcs_available(&self) -> Result<bool> {
        self.workspace_manager
            .check_available()
            .await
            .map_err(Into::into)
    }

    /// Execute changes with order-based dependency analysis and concurrent re-analysis.
    ///
    /// This method uses a `tokio::select!` based scheduler loop that:
    /// - Does NOT block on dispatch (spawn tasks into JoinSet)
    /// - Continues re-analysis even when apply commands are running
    /// - Tracks in-flight changes to calculate available slots
    /// - Responds to queue notifications, debounce timers, and task completions
    ///
    /// # Arguments
    /// * `changes` - Initial list of changes to execute
    /// * `analyzer` - Async function that returns AnalysisResult (order + dependencies)
    pub async fn execute_with_order_based_reanalysis<F>(
        &mut self,
        changes: Vec<crate::openspec::Change>,
        analyzer: F,
    ) -> Result<()>
    where
        F: Fn(
                &[crate::openspec::Change],
                u32,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::analyzer::AnalysisResult> + Send + '_>,
            > + Send
            + Sync,
    {
        if changes.is_empty() {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!(
            "Starting order-based execution with re-analysis for {} changes",
            changes.len()
        );

        // Start merge stall monitor if enabled
        let merge_stall_monitor_handle = if let Some(cancel_token) = &self.cancel_token {
            let merge_stall_config = self.config.get_merge_stall_detection();
            if merge_stall_config.enabled {
                if let Some(original_branch) = self.workspace_manager.original_branch() {
                    info!(
                        threshold_minutes = merge_stall_config.threshold_minutes,
                        check_interval_seconds = merge_stall_config.check_interval_seconds,
                        base_branch = %original_branch,
                        "Starting merge stall monitor for parallel execution"
                    );
                    let monitor = MergeStallMonitor::new(
                        merge_stall_config,
                        &self.repo_root,
                        original_branch.to_string(),
                    );
                    Some(monitor.spawn_monitor(cancel_token.clone()))
                } else {
                    warn!("Cannot start merge stall monitor: base branch not initialized");
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Prepare for parallel execution (clean check for git)
        info!("Preparing for parallel execution...");
        match self.workspace_manager.prepare_for_parallel().await {
            Ok(Some(warning)) => {
                warn!("{}", warning.message);
                send_event(
                    &self.event_tx,
                    ParallelEvent::Warning {
                        title: warning.title,
                        message: warning.message,
                    },
                )
                .await;
            }
            Ok(None) => {}
            Err(e) => {
                let error_msg = format!("Failed to prepare for parallel execution: {}", e);
                error!("{}", error_msg);
                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                return Err(e.into());
            }
        }
        info!("Preparation complete");

        // Initialize scheduler state
        let max_parallelism = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_parallelism));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let mut in_flight: HashSet<String> = HashSet::new();
        let mut queued: Vec<crate::openspec::Change> = changes;
        let mut iteration = 1u32;
        let mut cleanup_guard = WorkspaceCleanupGuard::new(
            self.workspace_manager.backend_type(),
            self.repo_root.clone(),
        );

        // Set needs_reanalysis to trigger first analysis
        self.needs_reanalysis = true;
        let mut reanalysis_reason = ReanalysisReason::Initial;

        // Main scheduler loop: wait for triggers and dispatch changes
        loop {
            // Check for cancellation
            if self.is_cancelled() {
                let remaining_changes: Vec<String> = queued.iter().map(|c| c.id.clone()).collect();
                let cancel_msg = format!(
                    "Cancelled parallel execution ({} queued, {} in-flight: queued=[{}], in-flight=[{}])",
                    remaining_changes.len(),
                    in_flight.len(),
                    remaining_changes.join(", "),
                    in_flight.iter().cloned().collect::<Vec<_>>().join(", ")
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn(&cancel_msg)),
                )
                .await;
                break;
            }

            // Step 1: Check dynamic queue for newly added changes (TUI mode)
            if let Some(queue) = &self.dynamic_queue {
                let mut queue_changed = false;
                while let Some(dynamic_id) = queue.pop().await {
                    if !queued.iter().any(|c| c.id == dynamic_id)
                        && !in_flight.contains(&dynamic_id)
                    {
                        match crate::openspec::list_changes_native() {
                            Ok(all_changes) => {
                                if let Some(new_change) =
                                    all_changes.into_iter().find(|c| c.id == dynamic_id)
                                {
                                    info!("Dynamically adding change to execution: {}", dynamic_id);
                                    send_event(
                                        &self.event_tx,
                                        ParallelEvent::Log(LogEntry::info(format!(
                                            "Dynamically added to parallel execution: {}",
                                            dynamic_id
                                        ))),
                                    )
                                    .await;
                                    queued.push(new_change);
                                    queue_changed = true;
                                } else {
                                    warn!(
                                        "Dynamically added change '{}' not found in openspec",
                                        dynamic_id
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to load dynamically added change '{}': {}",
                                    dynamic_id, e
                                );
                            }
                        }
                    }
                }

                // Update queue change timestamp if items were added
                if queue_changed {
                    let mut last_change = self.last_queue_change_at.lock().await;
                    *last_change = Some(std::time::Instant::now());
                    self.needs_reanalysis = true;
                    reanalysis_reason = ReanalysisReason::QueueNotification;
                    info!("Queue changed, re-analysis triggered");
                }
            }

            // Step 2: Re-analysis if needed and debounce elapsed
            if self.needs_reanalysis && queued.is_empty() && in_flight.is_empty() {
                // All work completed
                info!("All changes completed (queued and in-flight empty), stopping");
                break;
            }

            if self.needs_reanalysis && !queued.is_empty() {
                // Gate re-analysis by available execution slots
                let available_slots = max_parallelism.saturating_sub(in_flight.len());

                if available_slots == 0 {
                    // No available slots, defer re-analysis until slots become available
                    info!(
                        "Re-analysis deferred: no available slots (max: {}, in_flight: {}, queued: {})",
                        max_parallelism,
                        in_flight.len(),
                        queued.len()
                    );
                    // Keep needs_reanalysis=true so re-analysis will run when slots free up
                    // Continue to wait for in-flight completions
                } else {
                    // Check debounce (skip on first iteration)
                    let should_analyze = if iteration == 1 {
                        info!("First iteration, skipping debounce check");
                        true
                    } else {
                        self.should_reanalyze().await
                    };

                    if should_analyze {
                        // Filter out changes that depend on failed changes
                        let mut executable_changes: Vec<crate::openspec::Change> = Vec::new();
                        let mut skipped_changes: Vec<(String, String)> = Vec::new();

                        for change in &queued {
                            if let Some(reason) = self.skip_reason_for_change(&change.id) {
                                warn!("Excluding '{}' from analysis: {}", change.id, reason);
                                skipped_changes.push((change.id.clone(), reason));
                            } else {
                                executable_changes.push(change.clone());
                            }
                        }

                        // Emit skip events
                        for (change_id, reason) in skipped_changes {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ChangeSkipped { change_id, reason },
                            )
                            .await;
                        }

                        queued = executable_changes;

                        if queued.is_empty() {
                            info!("All queued changes skipped due to failed dependencies");
                            if in_flight.is_empty() {
                                break;
                            } else {
                                // Wait for in-flight to complete
                                self.needs_reanalysis = false;
                                continue;
                            }
                        }

                        // Run dependency analysis
                        info!(
                        "Re-analysis triggered: iteration={}, queued={}, in_flight={}, trigger={}",
                        iteration,
                        queued.len(),
                        in_flight.len(),
                        reanalysis_reason
                    );
                        send_event(
                            &self.event_tx,
                            ParallelEvent::AnalysisStarted {
                                remaining_changes: queued.len(),
                            },
                        )
                        .await;

                        let analysis_result = analyzer(&queued, iteration).await;

                        if analysis_result.order.is_empty() {
                            warn!("No order returned from analysis");
                            if in_flight.is_empty() {
                                break;
                            } else {
                                self.needs_reanalysis = false;
                                continue;
                            }
                        }

                        // Update dependencies
                        self.failed_tracker
                            .set_dependencies(analysis_result.dependencies.clone());
                        self.change_dependencies = analysis_result.dependencies.clone();

                        // Recalculate available slots (may have changed during analysis if tasks completed)
                        let available_slots = max_parallelism.saturating_sub(in_flight.len());
                        info!(
                            "Available slots after analysis: {} (max: {}, in_flight: {}, queued: {})",
                            available_slots,
                            max_parallelism,
                            in_flight.len(),
                            queued.len()
                        );

                        // Select changes to dispatch based on order and available slots
                        let mut selected_changes: Vec<String> = Vec::new();
                        for change_id in &analysis_result.order {
                            if selected_changes.len() >= available_slots {
                                break;
                            }

                            // Check if change has unresolved dependencies
                            if let Some(deps) = analysis_result.dependencies.get(change_id) {
                                let mut all_resolved = true;
                                for dep_id in deps {
                                    if !self.is_dependency_resolved(dep_id).await {
                                        all_resolved = false;
                                        info!(
                                            "Change '{}' blocked: waiting for dependency '{}'",
                                            change_id, dep_id
                                        );
                                        break;
                                    }
                                }

                                if !all_resolved {
                                    continue;
                                }
                            }

                            selected_changes.push(change_id.clone());
                        }

                        // Dispatch selected changes
                        if !selected_changes.is_empty() {
                            let base_revision = self
                                .workspace_manager
                                .get_current_revision()
                                .await
                                .map_err(OrchestratorError::from)?;

                            info!(
                                "Dispatching {} changes (iteration {}): {:?}",
                                selected_changes.len(),
                                iteration,
                                selected_changes
                            );

                            for change_id in &selected_changes {
                                if let Err(e) = self
                                    .dispatch_change_to_workspace(
                                        change_id.clone(),
                                        base_revision.clone(),
                                        semaphore.clone(),
                                        &mut join_set,
                                        &mut in_flight,
                                        &mut cleanup_guard,
                                    )
                                    .await
                                {
                                    let message =
                                        format!("Failed to dispatch change '{}': {}", change_id, e);
                                    self.failed_tracker.mark_failed(change_id);
                                    send_event(
                                        &self.event_tx,
                                        ParallelEvent::ProcessingError {
                                            id: change_id.clone(),
                                            error: message.clone(),
                                        },
                                    )
                                    .await;
                                    send_event(
                                        &self.event_tx,
                                        ParallelEvent::Log(LogEntry::error(message.clone())),
                                    )
                                    .await;
                                    error!("{}", message);
                                }
                            }

                            // Remove dispatched changes from queued
                            let dispatched_set: std::collections::HashSet<_> =
                                selected_changes.iter().collect();
                            queued.retain(|c| !dispatched_set.contains(&c.id));

                            iteration += 1;
                        }

                        self.needs_reanalysis = false;
                    } else {
                        // Debounce active, wait for timer or queue notification
                        info!("Debounce active, waiting for timer or queue notification");
                    }
                }
            }

            // Step 3: Check if all work is done (before waiting on select)
            if join_set.is_empty() && queued.is_empty() {
                info!("All work completed (join_set and queued empty), exiting scheduler loop");
                break;
            }

            // Step 4: Wait for events using tokio::select!
            // This makes the loop non-blocking and responsive to multiple triggers
            tokio::select! {
                // Join completion: task finished (apply+archive)
                Some(result) = join_set.join_next() => {
                    match result {
                        Ok(workspace_result) => {
                            // Remove from in-flight
                            in_flight.remove(&workspace_result.change_id);

                            info!(
                                "Task completed: change='{}', in_flight={}, available_slots={}, error={:?}",
                                workspace_result.change_id,
                                in_flight.len(),
                                max_parallelism.saturating_sub(in_flight.len()),
                                workspace_result.error
                            );

                            // Handle result (success or failure)
                            if let Some(error) = &workspace_result.error {
                                error!("Change '{}' failed: {}", workspace_result.change_id, error);
                                self.failed_tracker.mark_failed(&workspace_result.change_id);
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::ProcessingError {
                                        id: workspace_result.change_id.clone(),
                                        error: error.clone(),
                                    },
                                )
                                .await;
                            } else {
                                info!("Change '{}' completed successfully", workspace_result.change_id);

                                // Attempt merge if archive completed successfully
                                if workspace_result.final_revision.is_some() {
                                    let revisions = vec![workspace_result.workspace_name.clone()];
                                    let change_ids = vec![workspace_result.change_id.clone()];

                                    // Find workspace path for archive verification
                                    let workspace_path = self
                                        .workspace_manager
                                        .workspaces()
                                        .iter()
                                        .find(|workspace| workspace.name == workspace_result.workspace_name)
                                        .map(|workspace| workspace.path.clone());

                                    if let Some(path) = workspace_path {
                                        let archive_paths = vec![path];

                                        info!(
                                            "Merging archived {} (workspace: {})",
                                            workspace_result.change_id, workspace_result.workspace_name
                                        );

                                        match self.attempt_merge(&revisions, &change_ids, &archive_paths).await {
                                            Ok(MergeAttempt::Merged) => {
                                                // Merge succeeded, cleanup workspace
                                                send_event(
                                                    &self.event_tx,
                                                    ParallelEvent::CleanupStarted {
                                                        workspace: workspace_result.workspace_name.clone(),
                                                    },
                                                )
                                                .await;

                                                if let Err(err) = self
                                                    .workspace_manager
                                                    .cleanup_workspace(&workspace_result.workspace_name)
                                                    .await
                                                {
                                                    warn!(
                                                        "Failed to cleanup worktree '{}' after merge: {}",
                                                        workspace_result.workspace_name, err
                                                    );
                                                } else {
                                                    send_event(
                                                        &self.event_tx,
                                                        ParallelEvent::CleanupCompleted {
                                                            workspace: workspace_result.workspace_name.clone(),
                                                        },
                                                    )
                                                    .await;
                                                }
                                            }
                                            Ok(MergeAttempt::Deferred(reason)) => {
                                                // Merge deferred, preserve workspace and transition to MergeWait
                                                self.merge_deferred_changes.insert(workspace_result.change_id.clone());

                                                // Update workspace status to MergeWait so it's no longer counted as active
                                                self.workspace_manager.update_workspace_status(
                                                    &workspace_result.workspace_name,
                                                    WorkspaceStatus::MergeWait,
                                                );

                                                // Preserve this workspace from cleanup
                                                cleanup_guard.preserve(&workspace_result.workspace_name);

                                                send_event(
                                                    &self.event_tx,
                                                    ParallelEvent::MergeDeferred {
                                                        change_id: workspace_result.change_id.clone(),
                                                        reason,
                                                    },
                                                )
                                                .await;

                                                send_event(
                                                    &self.event_tx,
                                                    ParallelEvent::WorkspaceStatusUpdated {
                                                        workspace_name: workspace_result.workspace_name.clone(),
                                                        status: WorkspaceStatus::MergeWait,
                                                    },
                                                )
                                                .await;
                                            }
                                            Err(e) => {
                                                let error_msg = format!(
                                                    "Failed to merge archived {} (workspace: {}): {}",
                                                    workspace_result.change_id, workspace_result.workspace_name, e
                                                );
                                                error!("{}", error_msg);
                                                send_event(&self.event_tx, ParallelEvent::Error { message: error_msg })
                                                    .await;
                                                // Preserve workspace on merge error to allow debugging
                                                cleanup_guard.preserve(&workspace_result.workspace_name);
                                            }
                                        }
                                    } else {
                                        warn!(
                                            "Workspace '{}' not found after archive completion, skipping merge",
                                            workspace_result.workspace_name
                                        );
                                    }
                                }
                            }

                            // Trigger re-analysis on next iteration
                            self.needs_reanalysis = true;
                            reanalysis_reason = ReanalysisReason::Completion;
                        }
                        Err(e) => {
                            error!("Task panicked: {:?}", e);
                        }
                    }
                }

                // Queue notification: dynamic queue has new items
                Some(_) = async {
                    if let Some(queue) = &self.dynamic_queue {
                        queue.notified().await;
                        Some(())
                    } else {
                        std::future::pending().await
                    }
                } => {
                    info!("Queue notification received, will check queue on next iteration");
                    // Queue check happens at loop start
                }

                // Debounce timer: wait before allowing re-analysis
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    // Timer expired, loop will re-check needs_reanalysis and debounce
                }
            }
        }

        // Clean up merge stall monitor
        if let Some(handle) = merge_stall_monitor_handle {
            handle.abort();
        }

        // Drop cleanup guard without calling commit()
        // Workspaces are preserved by default for resume/debugging
        // Cleanup is only performed explicitly after successful merge via cleanup_workspace()
        drop(cleanup_guard);

        send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        Ok(())
    }

    /// Execute a single group of changes
    #[allow(dead_code)]
    async fn execute_changes_dispatch(
        &mut self,
        group: &ParallelGroup,
        total_changes: usize,
        changes_processed: usize,
    ) -> Result<()> {
        if self.is_cancelled() {
            let cancel_msg = format!(
                "Cancelled parallel execution for group {} (changes: {})",
                group.id,
                group.changes.join(", ")
            );
            send_event(
                &self.event_tx,
                ParallelEvent::Log(LogEntry::warn(&cancel_msg)),
            )
            .await;
            return Err(OrchestratorError::AgentCommand(cancel_msg));
        }
        // First, check which changes should be skipped due to failed dependencies
        let mut changes_to_execute: Vec<String> = Vec::new();
        let mut skipped_changes: Vec<(String, String)> = Vec::new();

        for change_id in &group.changes {
            if let Some(reason) = self.skip_reason_for_change(change_id) {
                warn!("Skipping '{}' because {}", change_id, reason);
                skipped_changes.push((change_id.clone(), reason));
            } else {
                changes_to_execute.push(change_id.clone());
            }
        }

        // Emit events for skipped changes
        for (change_id, reason) in &skipped_changes {
            send_event(
                &self.event_tx,
                ParallelEvent::ChangeSkipped {
                    change_id: change_id.clone(),
                    reason: reason.clone(),
                },
            )
            .await;
        }

        // If all changes are skipped, we're done with this group
        if changes_to_execute.is_empty() {
            info!(
                "All changes in dispatch iteration {} were skipped due to blocked dependencies",
                group.id
            );
            return Ok(());
        }

        // Get current base revision for this group's workspaces
        let base_revision = self
            .workspace_manager
            .get_current_revision()
            .await
            .map_err(OrchestratorError::from)?;
        info!(
            "Executing group {} with {} changes: {:?} (base revision: {})",
            group.id,
            changes_to_execute.len(),
            changes_to_execute,
            &base_revision[..8.min(base_revision.len())]
        );

        // Create cleanup guard to ensure workspaces are cleaned up on early errors
        let mut cleanup_guard = WorkspaceCleanupGuard::new(
            self.workspace_manager.backend_type(),
            self.repo_root.clone(),
        );

        // Categorize changes by workspace state (no upfront workspace creation for apply/archive)
        // Workspace creation will happen inside execute_apply_and_archive_parallel under semaphore control
        let mut changes_for_apply: Vec<String> = Vec::new();
        let mut archived_results: Vec<WorkspaceResult> = Vec::new();
        let mut archived_workspaces: Vec<Workspace> = Vec::new();

        for change_id in &changes_to_execute {
            // Check if workspace already exists (for resume scenario)
            // Skip resume if: global no_resume flag OR change needs forced recreation
            let existing_workspace = if self.no_resume
                || self.force_recreate_worktree.contains(change_id)
            {
                if self.force_recreate_worktree.contains(change_id) {
                    info!(
                        "Forcing worktree recreation for '{}' (dependency just resolved)",
                        change_id
                    );
                }
                None
            } else {
                match self
                    .workspace_manager
                    .find_existing_workspace(change_id)
                    .await
                {
                    Ok(Some(workspace_info)) => {
                        info!(
                            "Found existing workspace for '{}' (last modified: {:?})",
                            change_id, workspace_info.last_modified
                        );
                        match self
                            .workspace_manager
                            .reuse_workspace(&workspace_info)
                            .await
                        {
                            Ok(ws) => Some(ws),
                            Err(e) => {
                                warn!(
                                    "Failed to reuse workspace for '{}': {}, will create new under semaphore",
                                    change_id, e
                                );
                                None
                            }
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        warn!(
                            "Failed to find existing workspace for '{}': {}, will create new under semaphore",
                            change_id, e
                        );
                        None
                    }
                }
            };

            // Detect workspace state for resumed workspaces
            let workspace_state = if let Some(ref ws) = existing_workspace {
                let original_branch =
                    self.workspace_manager.original_branch().ok_or_else(|| {
                        OrchestratorError::GitCommand("Original branch not initialized".to_string())
                    })?;

                match detect_workspace_state(change_id, &ws.path, &original_branch).await {
                    Ok(state) => state,
                    Err(e) => {
                        warn!(
                            "Failed to detect workspace state for '{}': {}, assuming Created",
                            change_id, e
                        );
                        WorkspaceState::Created
                    }
                }
            } else {
                WorkspaceState::Created
            };

            // Handle different workspace states
            match workspace_state {
                WorkspaceState::Merged => {
                    // Already merged to main - skip all operations and cleanup
                    let workspace = existing_workspace.unwrap();
                    cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

                    info!(
                        "Change '{}' already merged to main in workspace '{}', skipping all operations",
                        change_id, workspace.name
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeCompleted {
                            change_id: change_id.clone(),
                            revision: "already-merged".to_string(),
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupStarted {
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;
                    if let Err(err) = self
                        .workspace_manager
                        .cleanup_workspace(&workspace.name)
                        .await
                    {
                        warn!(
                            "Failed to cleanup merged workspace '{}': {}",
                            workspace.name, err
                        );
                    } else {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace.name.clone(),
                            },
                        )
                        .await;
                    }
                    continue;
                }
                WorkspaceState::Archived => {
                    // Archive already committed. Check workspace state, ensure commit, then merge.
                    let workspace = existing_workspace.unwrap();
                    cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

                    info!(
                        "Change '{}' already archived in workspace '{}', skipping apply/archive",
                        change_id, workspace.name
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceResumed {
                            change_id: change_id.clone(),
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::ArchiveStarted(change_id.clone()),
                    )
                    .await;

                    let resolve_agent = AgentRunner::new_with_shared_state(
                        self.config.clone(),
                        self.shared_stagger_state.clone(),
                    );
                    let change_id_owned = change_id.clone();
                    let event_tx = self.event_tx.clone();
                    if let Err(err) = ensure_archive_commit(
                        change_id,
                        &workspace.path,
                        &resolve_agent,
                        &self.ai_runner,
                        self.workspace_manager.backend_type(),
                        move |line| {
                            let event_tx = event_tx.clone();
                            let change_id = change_id_owned.clone();
                            async move {
                                let text = match line {
                                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                                };
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ArchiveOutput {
                                            change_id,
                                            output: text,
                                            iteration: 1,
                                        })
                                        .await;
                                }
                            }
                        },
                    )
                    .await
                    {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ArchiveFailed {
                                change_id: change_id.clone(),
                                error: err.to_string(),
                            },
                        )
                        .await;
                        // Preserve all workspaces on error to allow resume/debugging
                        cleanup_guard.preserve_all();
                        return Err(err);
                    }

                    send_event(
                        &self.event_tx,
                        ParallelEvent::ChangeArchived(change_id.clone()),
                    )
                    .await;

                    let revision = self
                        .workspace_manager
                        .get_revision_in_workspace(&workspace.path)
                        .await
                        .map_err(OrchestratorError::from)?;
                    self.workspace_manager.update_workspace_status(
                        &workspace.name,
                        WorkspaceStatus::Applied(revision.clone()),
                    );

                    archived_results.push(WorkspaceResult {
                        change_id: change_id.clone(),
                        workspace_name: workspace.name.clone(),
                        final_revision: Some(revision),
                        error: None,
                    });
                    archived_workspaces.push(workspace);
                    continue;
                }
                WorkspaceState::Archiving => {
                    // Archive files moved but commit not complete
                    // IMPORTANT: Must run acceptance before committing archive
                    // Acceptance results are not persisted, so we must re-run on resume
                    let workspace = existing_workspace.unwrap();
                    cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

                    info!(
                        "Change '{}' in archiving state (files moved, commit incomplete) in workspace '{}'. Acceptance results are not persisted; will re-run acceptance before archive commit.",
                        change_id, workspace.name
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceResumed {
                            change_id: change_id.clone(),
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::Log(
                            LogEntry::info(
                                "Archive files moved but commit incomplete. Acceptance results are not persisted, so acceptance will be re-run before archive commit."
                            )
                            .with_change_id(change_id)
                            .with_operation("resume"),
                        ),
                    )
                    .await;

                    // Step 1: Run acceptance test before archive commit
                    // NOTE: Acceptance results are NOT persisted, so we must re-run on every resume

                    // Update status to Accepting
                    self.workspace_manager
                        .update_workspace_status(&workspace.name, WorkspaceStatus::Accepting);

                    let mut agent = AgentRunner::new_with_shared_state(
                        self.config.clone(),
                        self.shared_stagger_state.clone(),
                    );
                    info!(
                        "Running acceptance test for {} before archive (resume)",
                        change_id
                    );
                    let acceptance_result = execute_acceptance_in_workspace(
                        change_id,
                        &workspace.path,
                        &mut agent,
                        self.event_tx.clone(),
                        self.cancel_token.as_ref(),
                        &self.ai_runner,
                        &self.config,
                    )
                    .await;

                    // Get the acceptance iteration number for logging (count after recording)
                    let acceptance_iteration = agent.next_acceptance_attempt_number(change_id);

                    // Handle acceptance result
                    match acceptance_result {
                        Ok(crate::orchestration::AcceptanceResult::Pass) => {
                            info!(
                                "Acceptance passed for {} on resume, proceeding to archive commit",
                                change_id
                            );
                            // Continue to archive commit below
                        }
                        Ok(crate::orchestration::AcceptanceResult::Continue) => {
                            let continue_count =
                                agent.count_consecutive_acceptance_continues(change_id);
                            let max_continues = self.config.get_acceptance_max_continues();

                            if continue_count >= max_continues {
                                warn!(
                                    "Acceptance CONTINUE limit ({}) exceeded for {} on resume, treating as FAIL",
                                    max_continues,
                                    change_id
                                );
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Acceptance CONTINUE limit exceeded ({}), archive will not be committed. Change needs to return to apply loop.",
                                            max_continues
                                        ))
                                        .with_change_id(change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ),
                                )
                                .await;
                            } else {
                                info!(
                                    "Acceptance requires continuation for {} on resume (attempt {}/{}), returning to apply loop",
                                    change_id,
                                    continue_count,
                                    max_continues
                                );
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Acceptance requires continuation on resume (attempt {}/{}), archive will not be committed. Change needs to return to apply loop.",
                                            continue_count,
                                            max_continues
                                        ))
                                        .with_change_id(change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ),
                                )
                                .await;
                            }

                            changes_for_apply.push(change_id.clone());
                            continue;
                        }
                        Ok(crate::orchestration::AcceptanceResult::Fail { findings }) => {
                            warn!(
                                "Acceptance failed for {} with {} findings on resume, will not commit archive",
                                change_id,
                                findings.len()
                            );
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Log(
                                    LogEntry::warn(format!(
                                        "Acceptance failed with {} findings on resume, archive will not be committed. Change needs to return to apply loop.",
                                        findings.len()
                                    ))
                                    .with_change_id(change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ),
                            )
                            .await;
                            // Add to changes_for_apply to retry the full cycle
                            changes_for_apply.push(change_id.clone());
                            continue;
                        }
                        Ok(crate::orchestration::AcceptanceResult::CommandFailed {
                            error,
                            findings,
                        }) => {
                            error!(
                                "Acceptance command failed for {} on resume: {}",
                                change_id, error
                            );
                            if let Err(e) =
                                crate::orchestration::update_tasks_on_acceptance_failure(
                                    change_id,
                                    &findings,
                                    Some(&workspace.path),
                                )
                                .await
                            {
                                warn!("Failed to update tasks.md for {}: {}", change_id, e);
                            }
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Log(
                                    LogEntry::error(format!(
                                        "Acceptance command failed on resume: {}",
                                        error
                                    ))
                                    .with_change_id(change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ),
                            )
                            .await;
                            // Add to changes_for_apply to retry
                            changes_for_apply.push(change_id.clone());
                            continue;
                        }
                        Ok(crate::orchestration::AcceptanceResult::Cancelled) => {
                            info!("Acceptance cancelled for {} on resume", change_id);
                            continue;
                        }
                        Err(e) => {
                            error!("Acceptance error for {} on resume: {}", change_id, e);
                            send_event(
                                &self.event_tx,
                                ParallelEvent::Log(
                                    LogEntry::error(format!("Acceptance error on resume: {}", e))
                                        .with_change_id(change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                ),
                            )
                            .await;
                            // Add to changes_for_apply to retry
                            changes_for_apply.push(change_id.clone());
                            continue;
                        }
                    }

                    // Step 2: Commit archive (acceptance passed)
                    // Update status to Archiving
                    self.workspace_manager
                        .update_workspace_status(&workspace.name, WorkspaceStatus::Archiving);

                    send_event(
                        &self.event_tx,
                        ParallelEvent::ArchiveStarted(change_id.clone()),
                    )
                    .await;

                    let resolve_agent = AgentRunner::new_with_shared_state(
                        self.config.clone(),
                        self.shared_stagger_state.clone(),
                    );
                    let change_id_owned = change_id.clone();
                    let event_tx = self.event_tx.clone();
                    if let Err(err) = ensure_archive_commit(
                        change_id,
                        &workspace.path,
                        &resolve_agent,
                        &self.ai_runner,
                        self.workspace_manager.backend_type(),
                        move |line| {
                            let event_tx = event_tx.clone();
                            let change_id = change_id_owned.clone();
                            async move {
                                let text = match line {
                                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                                };
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ArchiveOutput {
                                            change_id,
                                            output: text,
                                            iteration: 1,
                                        })
                                        .await;
                                }
                            }
                        },
                    )
                    .await
                    {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ArchiveFailed {
                                change_id: change_id.clone(),
                                error: err.to_string(),
                            },
                        )
                        .await;
                        // Preserve all workspaces on error to allow resume/debugging
                        cleanup_guard.preserve_all();
                        return Err(err);
                    }

                    send_event(
                        &self.event_tx,
                        ParallelEvent::ChangeArchived(change_id.clone()),
                    )
                    .await;

                    let revision = self
                        .workspace_manager
                        .get_revision_in_workspace(&workspace.path)
                        .await
                        .map_err(OrchestratorError::from)?;
                    self.workspace_manager.update_workspace_status(
                        &workspace.name,
                        WorkspaceStatus::Applied(revision.clone()),
                    );

                    archived_results.push(WorkspaceResult {
                        change_id: change_id.clone(),
                        workspace_name: workspace.name.clone(),
                        final_revision: Some(revision),
                        error: None,
                    });
                    archived_workspaces.push(workspace);
                    continue;
                }
                WorkspaceState::Applied => {
                    // Apply complete but archive not started/complete
                    // IMPORTANT: Must run acceptance before archive
                    // Acceptance results are not persisted, so we must re-run on resume
                    let workspace = existing_workspace.unwrap();
                    cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

                    info!(
                        "Change '{}' in applied state in workspace '{}'. Acceptance results are not persisted; will re-run acceptance before archive.",
                        change_id, workspace.name
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceResumed {
                            change_id: change_id.clone(),
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::Log(
                            LogEntry::info(
                                "Apply complete on resume. Acceptance results are not persisted, so acceptance will be re-run before archive."
                            )
                            .with_change_id(change_id)
                            .with_operation("resume"),
                        ),
                    )
                    .await;

                    // Add to changes_for_apply to go through acceptance+archive
                    // The apply loop will quickly exit (tasks already complete), then run acceptance+archive
                    // NOTE: Acceptance results are NOT persisted, so we must re-run on every resume
                    changes_for_apply.push(change_id.clone());
                    // Store the workspace for reuse
                    // Note: This is handled by existing_workspace detection above
                }
                WorkspaceState::Applying { .. } | WorkspaceState::Created => {
                    // These states require apply (or resume apply)
                    // Add change_id to list - workspace creation will happen under semaphore control
                    changes_for_apply.push(change_id.clone());
                }
            }
        }

        for result in &archived_results {
            if result.final_revision.is_some() {
                let revisions = vec![result.workspace_name.clone()];
                let change_ids = vec![result.change_id.clone()];

                info!(
                    "Merging archived {} (workspace: {})",
                    result.change_id, result.workspace_name
                );
                let workspace_path = self
                    .workspace_manager
                    .workspaces()
                    .iter()
                    .find(|workspace| workspace.name == result.workspace_name)
                    .map(|workspace| workspace.path.clone())
                    .ok_or_else(|| {
                        OrchestratorError::GitCommand(format!(
                            "Workspace not found for archive verification: {}",
                            result.workspace_name
                        ))
                    })?;
                let archive_paths = vec![workspace_path];
                let merge_result = self
                    .attempt_merge(&revisions, &change_ids, &archive_paths)
                    .await;
                match merge_result {
                    Ok(MergeAttempt::Merged) => {}
                    Ok(MergeAttempt::Deferred(reason)) => {
                        self.merge_deferred_changes.insert(result.change_id.clone());

                        // Update workspace status to MergeWait so it's no longer counted as active
                        // Per spec line 7: "merge_wait ... はアクティブとして扱ってはならない（MUST NOT）"
                        self.workspace_manager.update_workspace_status(
                            &result.workspace_name,
                            WorkspaceStatus::MergeWait,
                        );

                        send_event(
                            &self.event_tx,
                            ParallelEvent::MergeDeferred {
                                change_id: result.change_id.clone(),
                                reason,
                            },
                        )
                        .await;

                        send_event(
                            &self.event_tx,
                            ParallelEvent::WorkspaceStatusUpdated {
                                workspace_name: result.workspace_name.clone(),
                                status: WorkspaceStatus::MergeWait,
                            },
                        )
                        .await;

                        continue;
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to merge archived {} (workspace: {}): {}",
                            result.change_id, result.workspace_name, e
                        );
                        error!("{}", error_msg);
                        send_event(&self.event_tx, ParallelEvent::Error { message: error_msg })
                            .await;
                        // Preserve all workspaces on error to allow resume/debugging
                        cleanup_guard.preserve_all();
                        return Err(e);
                    }
                }

                if let Some(workspace) = archived_workspaces
                    .iter()
                    .find(|workspace| workspace.change_id == result.change_id)
                {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupStarted {
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;
                    if let Err(err) = self
                        .workspace_manager
                        .cleanup_workspace(&workspace.name)
                        .await
                    {
                        warn!(
                            "Failed to cleanup worktree '{}' after merge: {}",
                            workspace.name, err
                        );
                    } else {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace.name.clone(),
                            },
                        )
                        .await;
                    }
                }
            }
        }

        // Execute apply + archive in parallel with concurrency limit
        // Workspace creation happens inside execute_apply_and_archive_parallel under semaphore control
        let mut results = archived_results;
        if !changes_for_apply.is_empty() {
            // Create change-workspace pairs: (change_id, None) for changes that need workspace creation
            let change_workspace_pairs: Vec<(String, Option<Workspace>)> = changes_for_apply
                .iter()
                .map(|id| (id.clone(), None))
                .collect();

            let apply_results = match self
                .execute_apply_and_archive_parallel(
                    &change_workspace_pairs,
                    &base_revision,
                    Some(group.id),
                    total_changes,
                    changes_processed,
                    &mut cleanup_guard,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = format!("Failed to execute applies: {}", e);
                    error!("{}", error_msg);
                    send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
                    // Preserve all workspaces on error to allow resume/debugging
                    cleanup_guard.preserve_all();
                    return Err(e);
                }
            };
            results.extend(apply_results);
        }

        // Collect successful and failed results
        let successful: Vec<&WorkspaceResult> = results
            .iter()
            .filter(|r| r.final_revision.is_some())
            .collect();
        let failed: Vec<&WorkspaceResult> = results.iter().filter(|r| r.error.is_some()).collect();

        // Check if any result indicates cancellation (force stop)
        // If so, preserve ALL workspaces (not just failed ones)
        let has_cancellation = failed.iter().any(|r| {
            r.error
                .as_ref()
                .is_some_and(|e| e.contains("cancel") || e.contains("Cancel"))
        });

        if has_cancellation {
            // Force stop detected - preserve all tracked workspaces
            info!("Cancellation detected, preserving all workspaces");
            cleanup_guard.preserve_all();

            // Emit WorkspacePreserved events for all tracked workspaces
            for result in &results {
                send_event(
                    &self.event_tx,
                    ParallelEvent::WorkspacePreserved {
                        change_id: result.change_id.clone(),
                        workspace_name: result.workspace_name.clone(),
                    },
                )
                .await;
            }

            // Mark failed changes for dependent skipping
            for result in &failed {
                self.failed_tracker.mark_failed(&result.change_id);
            }
        } else {
            // Regular failure (not cancellation) - preserve only failed workspaces
            for result in &failed {
                if result.error.is_some() {
                    error!(
                        "Failed for {}, workspace preserved: {}",
                        result.change_id, result.workspace_name
                    );
                    info!(
                        "To resume: run with the same change_id, workspace will be automatically detected"
                    );
                    cleanup_guard.preserve(&result.workspace_name);
                }
                // Emit WorkspacePreserved event
                send_event(
                    &self.event_tx,
                    ParallelEvent::WorkspacePreserved {
                        change_id: result.change_id.clone(),
                        workspace_name: result.workspace_name.clone(),
                    },
                )
                .await;
                // Mark the failed change so dependent changes will be skipped
                self.failed_tracker.mark_failed(&result.change_id);
            }
        }

        // If all failed, we don't have an error but continue to the next group
        // The dependent changes will be skipped automatically
        if successful.is_empty() && !results.is_empty() {
            warn!(
                "All changes in dispatch iteration {} failed, dependent changes will be skipped",
                group.id
            );
            return Ok(());
        }

        // Note: Individual merging is now done in execute_apply_and_archive_parallel
        // immediately after each change is archived. Group-level merge is no longer needed.

        // Cleanup only successful workspaces (preserve failed ones)
        let failed_workspace_names: std::collections::HashSet<_> =
            failed.iter().map(|r| r.workspace_name.clone()).collect();

        // Get all workspaces from workspace_manager (includes both apply and archived workspaces)
        let all_workspaces = self.workspace_manager.workspaces();
        let workspace_statuses: std::collections::HashMap<_, _> = all_workspaces
            .iter()
            .map(|workspace| (workspace.name.clone(), workspace.status.clone()))
            .collect();

        // Combine archived workspaces with workspaces from apply results
        let mut cleanup_workspaces = archived_workspaces;
        for result in &results {
            if let Some(ws) = all_workspaces
                .iter()
                .find(|w| w.name == result.workspace_name)
            {
                if !cleanup_workspaces.iter().any(|w| w.name == ws.name) {
                    cleanup_workspaces.push(ws.clone());
                }
            }
        }

        for workspace in &cleanup_workspaces {
            // Skip cleanup for failed workspaces - they are preserved
            if failed_workspace_names.contains(&workspace.name) {
                continue;
            }
            if self.merge_deferred_changes.contains(&workspace.change_id) {
                continue;
            }
            if matches!(
                workspace_statuses.get(&workspace.name),
                Some(WorkspaceStatus::Cleaned)
            ) {
                continue;
            }
            send_event(
                &self.event_tx,
                ParallelEvent::CleanupStarted {
                    workspace: workspace.name.clone(),
                },
            )
            .await;
            if let Err(err) = self
                .workspace_manager
                .cleanup_workspace(&workspace.name)
                .await
            {
                warn!(
                    "Failed to cleanup worktree '{}' after merge: {}",
                    workspace.name, err
                );
                continue;
            }
            send_event(
                &self.event_tx,
                ParallelEvent::CleanupCompleted {
                    workspace: workspace.name.clone(),
                },
            )
            .await;
        }

        // Drop cleanup guard without calling commit()
        // Workspaces are preserved by default for resume/debugging
        // Cleanup was already performed explicitly above via cleanup_workspace()
        drop(cleanup_guard);

        // Clear force_recreate_worktree flags for changes in this group
        // (they've now been recreated or failed)
        for change_id in &group.changes {
            self.force_recreate_worktree.remove(change_id);
        }

        Ok(())
    }

    /// Dispatch a single change to workspace (helper for concurrent re-analysis loop).
    ///
    /// This method:
    /// 1. Acquires semaphore permit
    /// 2. Creates or reuses workspace
    /// 3. Spawns apply + acceptance + archive task into JoinSet
    ///
    /// The spawned task will:
    /// - Execute apply command
    /// - Execute acceptance test (with retry loop)
    /// - Execute archive command (only if acceptance passes)
    /// - Return WorkspaceResult
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_change_to_workspace(
        &mut self,
        change_id: String,
        base_revision: String,
        semaphore: Arc<Semaphore>,
        join_set: &mut JoinSet<WorkspaceResult>,
        in_flight: &mut HashSet<String>,
        cleanup_guard: &mut WorkspaceCleanupGuard,
    ) -> Result<()> {
        // Check if already in-flight (avoid duplicate dispatch)
        if in_flight.contains(&change_id) {
            warn!(
                "Change '{}' already in-flight, skipping dispatch",
                change_id
            );
            return Ok(());
        }

        // Acquire semaphore permit
        let permit = semaphore.clone().acquire_owned().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to acquire semaphore: {}", e))
        })?;

        // Create or reuse workspace
        let workspace = workspace::get_or_create_workspace(
            self.workspace_manager.as_mut(),
            &change_id,
            &base_revision,
            self.no_resume,
            &self.force_recreate_worktree,
            &self.event_tx,
        )
        .await?;

        // Track workspace for cleanup
        cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

        // Add to in-flight set
        in_flight.insert(change_id.clone());

        // Prepare context for spawned task
        let apply_command = self.apply_command.clone();
        let archive_command = self.archive_command.clone();
        let repo_root = self.repo_root.clone();
        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let vcs_backend = self.workspace_manager.backend_type();
        let ai_runner = self.ai_runner.clone();
        let apply_history = self.apply_history.clone();
        let archive_history = self.archive_history.clone();
        let cancel_token = self.cancel_token.clone();
        let shared_stagger_state = self.shared_stagger_state.clone();

        // Spawn apply + acceptance + archive task
        join_set.spawn(async move {
            let _permit = permit; // Hold permit until task completes

            // Create agent for acceptance testing
            let mut agent = AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());

            // Track apply+acceptance cycles to prevent infinite loops
            const MAX_APPLY_ACCEPTANCE_CYCLES: u32 = 10;
            let mut cycle_count = 0u32;
            let mut cumulative_iteration = 0u32; // Track total apply iterations across all cycles

            // Apply+Acceptance loop: retry apply when acceptance fails
            let _apply_revision = loop {
                cycle_count += 1;
                if cycle_count > MAX_APPLY_ACCEPTANCE_CYCLES {
                    error!(
                        "Max apply+acceptance cycles ({}) reached for {}",
                        MAX_APPLY_ACCEPTANCE_CYCLES, change_id
                    );
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!(
                            "Max apply+acceptance cycles ({}) reached",
                            MAX_APPLY_ACCEPTANCE_CYCLES
                        )),
                    };
                }

                // Step 1: Execute apply with cumulative iteration count
                let apply_result = execute_apply_in_workspace(
                    &change_id,
                    &workspace.path,
                    &apply_command,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    None, // hooks
                    None, // parallel_ctx
                    cancel_token.as_ref(),
                    &ai_runner,
                    &repo_root,
                    &apply_history,
                    cumulative_iteration, // Pass current iteration count
                )
                .await;

                let (revision, final_iteration) = match apply_result {
                    Ok((rev, iter)) => (rev, iter),
                    Err(e) => {
                        // Apply failed - return error immediately
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Apply failed: {}", e)),
                        };
                    }
                };

                // Update cumulative iteration count
                cumulative_iteration = final_iteration;

                // Send ApplyCompleted event
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ApplyCompleted {
                            change_id: change_id.clone(),
                            revision: revision.clone(),
                        })
                        .await;
                }

                // Step 2: Execute acceptance test after apply succeeds
                // IMPORTANT: Acceptance results are NOT persisted to disk or git commits.
                // This means acceptance will always run after apply completes, even on resume.
                // This ensures quality gates are enforced regardless of interruptions.

                // Update status to Accepting
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            workspace_name: workspace.name.clone(),
                            status: WorkspaceStatus::Accepting,
                        })
                        .await;
                }

                info!(
                    "Running acceptance test for {} after apply completion (cycle {})",
                    change_id, cycle_count
                );
                let acceptance_result = execute_acceptance_in_workspace(
                    &change_id,
                    &workspace.path,
                    &mut agent,
                    event_tx.clone(),
                    cancel_token.as_ref(),
                    &ai_runner,
                    &config,
                )
                .await;

                // Get the acceptance iteration number for logging (count after recording)
                let acceptance_iteration = agent.next_acceptance_attempt_number(&change_id);

                match acceptance_result {
                    Ok(crate::orchestration::AcceptanceResult::Pass) => {
                        info!("Acceptance passed for {}, proceeding to archive", change_id);
                        // Break out of loop, proceed to archive
                        break revision;
                    }
                    Ok(crate::orchestration::AcceptanceResult::Continue) => {
                        let continue_count = agent.count_consecutive_acceptance_continues(&change_id);
                        let max_continues = config.get_acceptance_max_continues();

                        if continue_count >= max_continues {
                            warn!(
                                "Acceptance CONTINUE limit ({}) exceeded for {} (cycle {}), treating as FAIL",
                                max_continues, change_id, cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Acceptance CONTINUE limit exceeded (cycle {}), change will not be archived",
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Acceptance CONTINUE limit ({}) exceeded",
                                    max_continues
                                )),
                            };
                        } else {
                            info!(
                                "Acceptance requires continuation for {} (attempt {}/{}, cycle {}), retrying acceptance",
                                change_id,
                                continue_count,
                                max_continues,
                                cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Acceptance requires continuation (attempt {}/{}, cycle {}), retrying",
                                            continue_count,
                                            max_continues,
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            // Continue the acceptance loop - retry acceptance without re-applying
                            continue;
                        }
                    }
                    Ok(crate::orchestration::AcceptanceResult::Fail { findings }) => {
                        warn!(
                            "Acceptance failed for {} with {} findings (cycle {}), returning to apply loop",
                            change_id,
                            findings.len(),
                            cycle_count
                        );
                        // Update tasks.md with acceptance findings
                        if let Err(e) =
                            crate::orchestration::update_tasks_on_acceptance_failure(
                                &change_id,
                                &findings,
                                Some(&workspace.path),
                            )
                            .await
                        {
                            warn!(
                                "Failed to update tasks.md for {}: {}",
                                change_id, e
                            );
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn(format!(
                                        "Acceptance failed with {} findings, returning to apply loop (cycle {})",
                                        findings.len(),
                                        cycle_count
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        // Continue loop - retry apply with updated tasks
                        continue;
                    }
                    Ok(crate::orchestration::AcceptanceResult::CommandFailed {
                        error,
                        findings,
                    }) => {
                        error!(
                            "Acceptance command failed for {} (cycle {}): {}",
                            change_id, cycle_count, error
                        );
                        // Update tasks.md with command failure
                        if let Err(e) = crate::orchestration::update_tasks_on_acceptance_failure(
                            &change_id,
                            &findings,
                            Some(&workspace.path),
                        )
                        .await
                        {
                            warn!(
                                "Failed to update tasks.md for {}: {}",
                                change_id, e
                            );
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::error(format!(
                                        "Acceptance command failed (cycle {}): {}",
                                        cycle_count, error
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        // Command failed - this is a critical error, don't retry
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance command failed: {}", error)),
                        };
                    }
                    Ok(crate::orchestration::AcceptanceResult::Cancelled) => {
                        info!("Acceptance cancelled for {}", change_id);
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some("Acceptance cancelled".to_string()),
                        };
                    }
                    Err(e) => {
                        error!("Acceptance error for {}: {}", change_id, e);
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance error: {}", e)),
                        };
                    }
                }
            };

            // Step 3: Execute archive after acceptance passes
            // Update status to Archiving
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::WorkspaceStatusUpdated {
                        workspace_name: workspace.name.clone(),
                        status: WorkspaceStatus::Archiving,
                    })
                    .await;
                let _ = tx
                    .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                    .await;
            }

            let archive_result = execute_archive_in_workspace(
                &change_id,
                &workspace.path,
                &archive_command,
                &config,
                event_tx.clone(),
                vcs_backend,
                None, // hooks
                None, // parallel_ctx
                cancel_token.as_ref(),
                &ai_runner,
                &archive_history,
                &apply_history,
                &shared_stagger_state,
            )
            .await;

            match archive_result {
                Ok(archive_revision) => {
                    // Clear acceptance history after successful archive
                    agent.clear_acceptance_history(&change_id);

                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ChangeArchived(change_id.clone()))
                            .await;
                    }
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: Some(archive_revision),
                        error: None,
                    }
                }
                Err(e) => {
                    warn!("Archive failed for {}: {}", change_id, e);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ArchiveFailed {
                                change_id: change_id.clone(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                    // Archive failed - do not merge unarchived changes
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!("Archive failed: {}", e)),
                    }
                }
            }
            // _permit is dropped here, releasing semaphore
        });

        Ok(())
    }

    /// Execute apply + archive in parallel with workspace creation under semaphore control.
    ///
    /// Workspaces are created sequentially under semaphore control to ensure that
    /// workspace creation + execution never exceeds max_concurrent limit.
    ///
    /// Flow for each change:
    /// 1. Acquire semaphore permit (blocks if max_concurrent limit reached)
    /// 2. Create/resume workspace (sequential, in main task with &mut self access)
    /// 3. Spawn async task for apply + archive
    /// 4. Release permit when task completes
    ///
    /// This ensures workspace creation rate is controlled by the concurrency limit.
    #[allow(dead_code)]
    async fn execute_apply_and_archive_parallel(
        &mut self,
        change_workspace_pairs: &[(String, Option<Workspace>)],
        base_revision: &str,
        group_index: Option<u32>,
        total_changes: usize,
        changes_processed: usize,
        cleanup_guard: &mut WorkspaceCleanupGuard,
    ) -> Result<Vec<WorkspaceResult>> {
        let max_concurrent = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let total_changes_in_group = change_workspace_pairs.len();

        // Create a channel for workspace status updates from spawned tasks
        let (status_tx, mut status_rx) =
            tokio::sync::mpsc::unbounded_channel::<(String, WorkspaceStatus)>();

        // Track changes we've already spawned tasks for (to avoid duplicates from dynamic queue)
        let mut spawned_changes: HashSet<String> = HashSet::new();

        for (change_id, existing_workspace) in change_workspace_pairs {
            spawned_changes.insert(change_id.clone());
            // Acquire semaphore BEFORE creating workspace to enforce concurrency limit
            let permit = semaphore.clone().acquire_owned().await.unwrap();

            // Create or reuse workspace under semaphore control
            let workspace = if let Some(ws) = existing_workspace {
                // Workspace was already resumed (passed from archived changes)
                ws.clone()
            } else {
                // Create new workspace or find existing one
                let workspace_opt = if self.no_resume {
                    None
                } else {
                    match self
                        .workspace_manager
                        .find_existing_workspace(change_id)
                        .await
                    {
                        Ok(Some(workspace_info)) => {
                            info!(
                                "Resuming existing workspace for '{}' (last modified: {:?})",
                                change_id, workspace_info.last_modified
                            );
                            match self
                                .workspace_manager
                                .reuse_workspace(&workspace_info)
                                .await
                            {
                                Ok(ws) => {
                                    send_event(
                                        &self.event_tx,
                                        ParallelEvent::WorkspaceResumed {
                                            change_id: change_id.clone(),
                                            workspace: ws.name.clone(),
                                        },
                                    )
                                    .await;
                                    Some(ws)
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to reuse workspace for '{}': {}, creating new",
                                        change_id, e
                                    );
                                    None
                                }
                            }
                        }
                        Ok(None) => None,
                        Err(e) => {
                            warn!(
                                "Failed to find existing workspace for '{}': {}, creating new",
                                change_id, e
                            );
                            None
                        }
                    }
                };

                match workspace_opt {
                    Some(ws) => ws,
                    None => {
                        // Create new workspace
                        match self
                            .workspace_manager
                            .create_workspace(change_id, Some(base_revision))
                            .await
                        {
                            Ok(ws) => {
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::WorkspaceCreated {
                                        change_id: change_id.clone(),
                                        workspace: ws.name.clone(),
                                    },
                                )
                                .await;
                                ws
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to create workspace: {}", e);
                                error!("{} for {}", error_msg, change_id);
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Error {
                                        message: format!("[{}] {}", change_id, error_msg),
                                    },
                                )
                                .await;
                                // Drop permit and return error
                                drop(permit);
                                return Err(e.into());
                            }
                        }
                    }
                }
            };

            // Track workspace in cleanup guard
            cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

            // Send ProcessingStarted event
            send_event(
                &self.event_tx,
                ParallelEvent::ProcessingStarted(change_id.clone()),
            )
            .await;

            let change_id = workspace.change_id.clone();
            let workspace_path = workspace.path.clone();
            let workspace_name = workspace.name.clone();
            let apply_cmd = self.apply_command.clone();
            let archive_cmd = self.archive_command.clone();
            let config = self.config.clone();
            let event_tx = self.event_tx.clone();
            let vcs_backend = self.workspace_manager.backend_type();
            let hooks = self.hooks.clone();
            let cancel_token = self.cancel_token.clone();
            let ai_runner = self.ai_runner.clone();
            let repo_root = self.repo_root.clone();
            let apply_history = self.apply_history.clone();
            let archive_history = self.archive_history.clone();
            let status_tx_clone = status_tx.clone();
            let shared_stagger_state = self.shared_stagger_state.clone();

            // Build parallel hook context
            let parallel_ctx = ParallelHookContext {
                workspace_path: workspace_path.to_string_lossy().to_string(),
                group_index,
                total_changes_in_group,
                total_changes,
                changes_processed,
            };

            // Update status
            self.workspace_manager
                .update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

            join_set.spawn(async move {
                // Keep permit until task completes
                let _permit = permit;

                // Create agent for this workspace
                let mut agent = AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());

                // Track apply+acceptance cycles to prevent infinite loops
                const MAX_APPLY_ACCEPTANCE_CYCLES: u32 = 10;
                let mut cycle_count = 0u32;
                let mut cumulative_iteration = 0u32; // Track total apply iterations across all cycles

                // Apply+Acceptance loop: retry apply when acceptance fails
                let _apply_revision = loop {
                    cycle_count += 1;
                    if cycle_count > MAX_APPLY_ACCEPTANCE_CYCLES {
                        error!(
                            "Max apply+acceptance cycles ({}) reached for {}",
                            MAX_APPLY_ACCEPTANCE_CYCLES, change_id
                        );
                        return WorkspaceResult {
                            change_id,
                            workspace_name,
                            final_revision: None,
                            error: Some(format!(
                                "Max apply+acceptance cycles ({}) reached",
                                MAX_APPLY_ACCEPTANCE_CYCLES
                            )),
                        };
                    }

                    // Step 1: Execute apply with cumulative iteration count
                    let apply_result = execute_apply_in_workspace(
                        &change_id,
                        &workspace_path,
                        &apply_cmd,
                        &config,
                        event_tx.clone(),
                        vcs_backend,
                        hooks.as_ref().map(|h| h.as_ref()),
                        Some(&parallel_ctx),
                        cancel_token.as_ref(),
                        &ai_runner,
                        &repo_root,
                        &apply_history,
                        cumulative_iteration, // Pass current iteration count
                    )
                    .await;

                    let (revision, final_iteration) = match apply_result {
                        Ok((rev, iter)) => (rev, iter),
                        Err(e) => {
                            // Apply failed - return error immediately
                            return WorkspaceResult {
                                change_id,
                                workspace_name,
                                final_revision: None,
                                error: Some(format!("Apply failed: {}", e)),
                            };
                        }
                    };

                    // Update cumulative iteration count
                    cumulative_iteration = final_iteration;

                    // Send ApplyCompleted event
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ApplyCompleted {
                                change_id: change_id.clone(),
                                revision: revision.clone(),
                            })
                            .await;
                    }

                    // Step 2: Execute acceptance test after apply succeeds
                    // IMPORTANT: Acceptance results are NOT persisted to disk or git commits.
                    // This means acceptance will always run after apply completes, even on resume.
                    // This ensures quality gates are enforced regardless of interruptions.

                    // Update status to Accepting
                    let _ = status_tx_clone.send((workspace_name.clone(), WorkspaceStatus::Accepting));
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::WorkspaceStatusUpdated {
                                workspace_name: workspace_name.clone(),
                                status: WorkspaceStatus::Accepting,
                            })
                            .await;
                    }

                    info!(
                        "Running acceptance test for {} after apply completion (cycle {})",
                        change_id, cycle_count
                    );
                    let acceptance_result = execute_acceptance_in_workspace(
                        &change_id,
                        &workspace_path,
                        &mut agent,
                        event_tx.clone(),
                        cancel_token.as_ref(),
                        &ai_runner,
                        &config,
                    )
                    .await;

                    // Get the acceptance iteration number for logging (count after recording)
                    let acceptance_iteration = agent.next_acceptance_attempt_number(&change_id);

                    match acceptance_result {
                        Ok(crate::orchestration::AcceptanceResult::Pass) => {
                            info!("Acceptance passed for {}, proceeding to archive", change_id);
                            // Break out of loop, proceed to archive
                            break revision;
                        }
                        Ok(crate::orchestration::AcceptanceResult::Continue) => {
                            let continue_count = agent.count_consecutive_acceptance_continues(&change_id);
                            let max_continues = config.get_acceptance_max_continues();

                            if continue_count >= max_continues {
                                warn!(
                                    "Acceptance CONTINUE limit ({}) exceeded for {} (cycle {}), treating as FAIL",
                                    max_continues, change_id, cycle_count
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::warn(format!(
                                                "Acceptance CONTINUE limit exceeded (cycle {}), change will not be archived",
                                                cycle_count
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Acceptance CONTINUE limit ({}) exceeded",
                                        max_continues
                                    )),
                                };
                            } else {
                                info!(
                                    "Acceptance requires continuation for {} (attempt {}/{}, cycle {}), retrying acceptance",
                                    change_id,
                                    continue_count,
                                    max_continues,
                                    cycle_count
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::info(format!(
                                                "Acceptance requires continuation (attempt {}/{}, cycle {}), retrying",
                                                continue_count,
                                                max_continues,
                                                cycle_count
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                // Continue the acceptance loop - retry acceptance without re-applying
                                continue;
                            }
                        }
                        Ok(crate::orchestration::AcceptanceResult::Fail { findings }) => {
                            warn!(
                                "Acceptance failed for {} with {} findings (cycle {}), returning to apply loop",
                                change_id,
                                findings.len(),
                                cycle_count
                            );
                            // Update tasks.md with acceptance findings
                            if let Err(e) =
                                crate::orchestration::update_tasks_on_acceptance_failure(
                                    &change_id,
                                    &findings,
                                    Some(&workspace_path),
                                )
                                .await
                            {
                                warn!(
                                    "Failed to update tasks.md for {}: {}",
                                    change_id, e
                                );
                            }
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Acceptance failed with {} findings, returning to apply loop (cycle {})",
                                            findings.len(),
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            // Continue loop - retry apply with updated tasks
                            continue;
                        }
                        Ok(crate::orchestration::AcceptanceResult::CommandFailed {
                            error,
                            findings,
                        }) => {
                            error!(
                                "Acceptance command failed for {} (cycle {}): {}",
                                change_id, cycle_count, error
                            );
                            // Update tasks.md with command failure
                            if let Err(e) =
                                crate::orchestration::update_tasks_on_acceptance_failure(
                                    &change_id,
                                    &findings,
                                    Some(&workspace_path),
                                )
                                .await
                            {
                                warn!(
                                    "Failed to update tasks.md for {}: {}",
                                    change_id, e
                                );
                            }
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::error(format!(
                                            "Acceptance command failed (cycle {}): {}",
                                            cycle_count, error
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            // Command failed - this is a critical error, don't retry
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance command failed: {}", error)),
                        };

                        }
                        Ok(crate::orchestration::AcceptanceResult::Cancelled) => {
                            info!("Acceptance cancelled for {}", change_id);
                            return WorkspaceResult {
                                change_id,
                                workspace_name,
                                final_revision: None,
                                error: Some("Acceptance cancelled".to_string()),
                            };
                        }
                        Err(e) => {
                            error!("Acceptance error for {}: {}", change_id, e);
                            return WorkspaceResult {
                                change_id,
                                workspace_name,
                                final_revision: None,
                                error: Some(format!("Acceptance error: {}", e)),
                            };
                        }
                    }
                };

                // Step 3: Execute archive after acceptance passes
                // Update status to Archiving
                let _ = status_tx_clone.send((workspace_name.clone(), WorkspaceStatus::Archiving));
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            workspace_name: workspace_name.clone(),
                            status: WorkspaceStatus::Archiving,
                        })
                        .await;
                    let _ = tx
                        .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                        .await;
                }

                let archive_result = execute_archive_in_workspace(
                    &change_id,
                    &workspace_path,
                    &archive_cmd,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    hooks.as_ref().map(|h| h.as_ref()),
                    Some(&parallel_ctx),
                    cancel_token.as_ref(),
                    &ai_runner,
                    &archive_history,
                    &apply_history,
                    &shared_stagger_state,
                )
                .await;

                match archive_result {
                    Ok(archive_revision) => {
                        // Clear acceptance history after successful archive
                        agent.clear_acceptance_history(&change_id);

                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ChangeArchived(change_id.clone()))
                                .await;
                        }
                        WorkspaceResult {
                            change_id,
                            workspace_name,
                            final_revision: Some(archive_revision),
                            error: None,
                        }
                    }
                    Err(e) => {
                        warn!("Archive failed for {}: {}", change_id, e);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ArchiveFailed {
                                    change_id: change_id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                        // Archive failed - do not merge unarchived changes
                        WorkspaceResult {
                            change_id,
                            workspace_name,
                            final_revision: None,
                            error: Some(format!("Archive failed: {}", e)),
                        }
                    }
                }
                // _permit is dropped here, releasing semaphore
            });
        }

        // Note: We keep status_tx alive for dynamic queue task spawning
        // It will be dropped at the end of the function automatically

        // Collect results and process status updates
        let mut results = Vec::new();
        loop {
            tokio::select! {
                // Process status updates from spawned tasks
                Some((workspace_name, status)) = status_rx.recv() => {
                    self.workspace_manager.update_workspace_status(&workspace_name, status);
                }
                // Process completed tasks
                Some(result) = join_set.join_next() => {
                    match result {
                        Ok(workspace_result) => {
                            // Update workspace status
                            if workspace_result.error.is_some() {
                                self.workspace_manager.update_workspace_status(
                                    &workspace_result.workspace_name,
                                    WorkspaceStatus::Failed(
                                        workspace_result.error.clone().unwrap_or_default(),
                                    ),
                                );
                            } else if let Some(ref rev) = workspace_result.final_revision {
                                self.workspace_manager.update_workspace_status(
                                    &workspace_result.workspace_name,
                                    WorkspaceStatus::Applied(rev.clone()),
                                );

                                // Individual merge: merge immediately after archive completes
                                let revisions = vec![workspace_result.workspace_name.clone()];
                                let change_ids = vec![workspace_result.change_id.clone()];

                                info!(
                                    "Merging {} (workspace: {})",
                                    workspace_result.change_id, workspace_result.workspace_name
                                );
                                let archive_paths = vec![self
                                    .workspace_manager
                                    .workspaces()
                                    .iter()
                                    .find(|workspace| workspace.name == workspace_result.workspace_name)
                                    .map(|workspace| workspace.path.clone())
                                    .ok_or_else(|| {
                                        OrchestratorError::GitCommand(format!(
                                            "Workspace not found for archive verification: {}",
                                            workspace_result.workspace_name
                                        ))
                                    })?];
                                let merge_result = self
                                    .attempt_merge(&revisions, &change_ids, &archive_paths)
                                    .await;
                                match merge_result {
                                    Ok(MergeAttempt::Merged) => {
                                        info!(
                                            "Successfully merged {} (workspace: {})",
                                            workspace_result.change_id, workspace_result.workspace_name
                                        );
                                        send_event(
                                            &self.event_tx,
                                            ParallelEvent::CleanupStarted {
                                                workspace: workspace_result.workspace_name.clone(),
                                            },
                                        )
                                        .await;
                                        if let Err(err) = self
                                            .workspace_manager
                                            .cleanup_workspace(&workspace_result.workspace_name)
                                            .await
                                        {
                                            warn!(
                                                "Failed to cleanup worktree '{}' after merge: {}",
                                                workspace_result.workspace_name, err
                                            );
                                        } else {
                                            send_event(
                                                &self.event_tx,
                                                ParallelEvent::CleanupCompleted {
                                                    workspace: workspace_result.workspace_name.clone(),
                                                },
                                            )
                                            .await;
                                        }
                                    }
                                    Ok(MergeAttempt::Deferred(reason)) => {
                                        self.merge_deferred_changes
                                            .insert(workspace_result.change_id.clone());

                                        // Update workspace status to MergeWait so it's no longer counted as active
                                        // Per spec line 7: "merge_wait ... はアクティブとして扱ってはならない（MUST NOT）"
                                        self.workspace_manager.update_workspace_status(
                                            &workspace_result.workspace_name,
                                            WorkspaceStatus::MergeWait,
                                        );

                                        send_event(
                                            &self.event_tx,
                                            ParallelEvent::MergeDeferred {
                                                change_id: workspace_result.change_id.clone(),
                                                reason,
                                            },
                                        )
                                        .await;

                                        send_event(
                                            &self.event_tx,
                                            ParallelEvent::WorkspaceStatusUpdated {
                                                workspace_name: workspace_result.workspace_name.clone(),
                                                status: WorkspaceStatus::MergeWait,
                                            },
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to merge {} (workspace: {}): {}",
                                            workspace_result.change_id,
                                            workspace_result.workspace_name,
                                            e
                                        );
                                        // Merge failure is critical - preserve all workspaces and return error
                                        cleanup_guard.preserve_all();
                                        return Err(e);
                                    }
                                }
                            }
                            results.push(workspace_result);

                            // Check dynamic queue for new changes when a task completes (slot available)
                            // Per spec: システムはキュー変更を実行中でも監視し、実行スロットが空いたタイミングで次の変更を選定
                            if let Some(queue) = &self.dynamic_queue {
                                // Try to acquire a semaphore permit to check if a slot is available
                                // Use try_acquire to avoid blocking
                                if let Ok(permit) = semaphore.clone().try_acquire_owned() {
                                    // Pop from queue (non-blocking)
                                    if let Some(dynamic_id) = queue.pop().await {
                                        // Check if we've already spawned this change
                                        if !spawned_changes.contains(&dynamic_id) {
                                            // Load change details from openspec
                                            match crate::openspec::list_changes_native() {
                                                Ok(all_changes) => {
                                                    if let Some(new_change) = all_changes.into_iter().find(|c| c.id == dynamic_id) {
                                                        info!("Dynamically spawning task for change '{}' in active execution (slot became available)", dynamic_id);
                                                        send_event(
                                                            &self.event_tx,
                                                            ParallelEvent::Log(LogEntry::info(format!(
                                                                "Slot available: dynamically starting '{}' during continuous execution",
                                                                dynamic_id
                                                            ))),
                                                        )
                                                        .await;

                                                        // Update queue change timestamp for debounce tracking
                                                        {
                                                            let mut last_change = self.last_queue_change_at.lock().await;
                                                            *last_change = Some(std::time::Instant::now());
                                                        }

                                                        // Mark as spawned
                                                        spawned_changes.insert(new_change.id.clone());

                                                        // Create workspace for the new change
                                                        // Note: This follows the same pattern as the initial spawn loop above
                                                        let change_id = &new_change.id;

                                                        // Check for existing workspace (resume scenario)
                                                        let workspace_opt = if self.no_resume || self.force_recreate_worktree.contains(change_id) {
                                                            None
                                                        } else {
                                                            match self.workspace_manager.find_existing_workspace(change_id).await {
                                                                Ok(Some(workspace_info)) => {
                                                                    info!("Resuming existing workspace for '{}' (dynamically added)", change_id);
                                                                    match self.workspace_manager.reuse_workspace(&workspace_info).await {
                                                                        Ok(ws) => {
                                                                            send_event(
                                                                                &self.event_tx,
                                                                                ParallelEvent::WorkspaceResumed {
                                                                                    change_id: change_id.clone(),
                                                                                    workspace: ws.name.clone(),
                                                                                },
                                                                            ).await;
                                                                            Some(ws)
                                                                        }
                                                                        Err(e) => {
                                                                            warn!("Failed to reuse workspace for '{}': {}, creating new", change_id, e);
                                                                            None
                                                                        }
                                                                    }
                                                                }
                                                                Ok(None) => None,
                                                                Err(e) => {
                                                                    warn!("Failed to find existing workspace for '{}': {}, creating new", change_id, e);
                                                                    None
                                                                }
                                                            }
                                                        };

                                                        let workspace = match workspace_opt {
                                                            Some(ws) => ws,
                                                            None => {
                                                                match self.workspace_manager.create_workspace(change_id, Some(base_revision)).await {
                                                                    Ok(ws) => {
                                                                        send_event(
                                                                            &self.event_tx,
                                                                            ParallelEvent::WorkspaceCreated {
                                                                                change_id: change_id.clone(),
                                                                                workspace: ws.name.clone(),
                                                                            },
                                                                        ).await;
                                                                        ws
                                                                    }
                                                                    Err(e) => {
                                                                        let error_msg = format!("Failed to create workspace for dynamically added change: {}", e);
                                                                        error!("{} for {}", error_msg, change_id);
                                                                        send_event(
                                                                            &self.event_tx,
                                                                            ParallelEvent::Error {
                                                                                message: format!("[{}] {}", change_id, error_msg),
                                                                            },
                                                                        ).await;
                                                                        // Drop permit and continue (don't fail entire batch)
                                                                        drop(permit);
                                                                        continue;
                                                                    }
                                                                }
                                                            }
                                                        };

                                                        // Track workspace in cleanup guard
                                                        cleanup_guard.track(workspace.name.clone(), workspace.path.clone());

                                                        // Send ProcessingStarted event
                                                        send_event(
                                                            &self.event_tx,
                                                            ParallelEvent::ProcessingStarted(change_id.clone()),
                                                        ).await;

                                                        // Clone all necessary data for the spawned task
                                                        let change_id = workspace.change_id.clone();
                                                        let workspace_path = workspace.path.clone();
                                                        let workspace_name = workspace.name.clone();
                                                        let apply_cmd = self.apply_command.clone();
                                                        let archive_cmd = self.archive_command.clone();
                                                        let config = self.config.clone();
                                                        let event_tx = self.event_tx.clone();
                                                        let vcs_backend = self.workspace_manager.backend_type();
                                                        let hooks = self.hooks.clone();
                                                        let cancel_token = self.cancel_token.clone();
                                                        let ai_runner = self.ai_runner.clone();
                                                        let repo_root = self.repo_root.clone();
                                                        let apply_history = self.apply_history.clone();
                                                        let archive_history = self.archive_history.clone();
                                                        let status_tx_clone = status_tx.clone();
                                                        let shared_stagger_state = self.shared_stagger_state.clone();

                                                        // Build parallel hook context
                                                        let parallel_ctx = ParallelHookContext {
                                                            workspace_path: workspace_path.to_string_lossy().to_string(),
                                                            group_index,
                                                            total_changes_in_group: total_changes_in_group + spawned_changes.len() - total_changes_in_group,
                                                            total_changes,
                                                            changes_processed,
                                                        };

                                                        // Update status
                                                        self.workspace_manager.update_workspace_status(&workspace_name, WorkspaceStatus::Applying);

                                                        // Spawn task (same logic as initial spawn loop)
                                                        join_set.spawn(async move {
                                                            let _permit = permit;

                                                            let mut agent = AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());

                                                            const MAX_APPLY_ACCEPTANCE_CYCLES: u32 = 10;
                                                            let mut cycle_count = 0u32;
                                                            let mut cumulative_iteration = 0u32;

                                                            let _apply_revision = loop {
                                                                cycle_count += 1;
                                                                if cycle_count > MAX_APPLY_ACCEPTANCE_CYCLES {
                                                                    error!(
                                                                        "Max apply+acceptance cycles ({}) reached for {}",
                                                                        MAX_APPLY_ACCEPTANCE_CYCLES, change_id
                                                                    );
                                                                    return WorkspaceResult {
                                                                        change_id,
                                                                        workspace_name,
                                                                        final_revision: None,
                                                                        error: Some(format!(
                                                                            "Max apply+acceptance cycles ({}) reached",
                                                                            MAX_APPLY_ACCEPTANCE_CYCLES
                                                                        )),
                                                                    };
                                                                }

                                                                let apply_result = execute_apply_in_workspace(
                                                                    &change_id,
                                                                    &workspace_path,
                                                                    &apply_cmd,
                                                                    &config,
                                                                    event_tx.clone(),
                                                                    vcs_backend,
                                                                    hooks.as_ref().map(|h| h.as_ref()),
                                                                    Some(&parallel_ctx),
                                                                    cancel_token.as_ref(),
                                                                    &ai_runner,
                                                                    &repo_root,
                                                                    &apply_history,
                                                                    cumulative_iteration,
                                                                )
                                                                .await;

                                                                let (revision, final_iteration) = match apply_result {
                                                                    Ok((rev, iter)) => (rev, iter),
                                                                    Err(e) => {
                                                                        return WorkspaceResult {
                                                                            change_id,
                                                                            workspace_name,
                                                                            final_revision: None,
                                                                            error: Some(format!("Apply failed: {}", e)),
                                                                        };
                                                                    }
                                                                };

                                                                cumulative_iteration = final_iteration;

                                                                if let Some(ref tx) = event_tx {
                                                                    let _ = tx
                                                                        .send(ParallelEvent::ApplyCompleted {
                                                                            change_id: change_id.clone(),
                                                                            revision: revision.clone(),
                                                                        })
                                                                        .await;
                                                                }

                                                                let _ = status_tx_clone.send((workspace_name.clone(), WorkspaceStatus::Accepting));
                                                                if let Some(ref tx) = event_tx {
                                                                    let _ = tx
                                                                        .send(ParallelEvent::WorkspaceStatusUpdated {
                                                                            workspace_name: workspace_name.clone(),
                                                                            status: WorkspaceStatus::Accepting,
                                                                        })
                                                                        .await;
                                                                }

                                                                info!(
                                                                    "Running acceptance test for {} after apply completion (cycle {})",
                                                                    change_id, cycle_count
                                                                );
                                                                let acceptance_result = execute_acceptance_in_workspace(
                                                                    &change_id,
                                                                    &workspace_path,
                                                                    &mut agent,
                                                                    event_tx.clone(),
                                                                    cancel_token.as_ref(),
                                                                    &ai_runner,
                                                                    &config,
                                                                )
                                                                .await;

                                                                let acceptance_iteration = agent.next_acceptance_attempt_number(&change_id);

                                                                match acceptance_result {
                                                                    Ok(crate::orchestration::AcceptanceResult::Pass) => {
                                                                        info!("Acceptance passed for {}, proceeding to archive", change_id);
                                                                        break revision;
                                                                    }
                                                                    Ok(crate::orchestration::AcceptanceResult::Continue) => {
                                                                        let continue_count = agent.count_consecutive_acceptance_continues(&change_id);
                                                                        let max_continues = config.get_acceptance_max_continues();

                                                                        if continue_count >= max_continues {
                                                                            warn!(
                                                                                "Acceptance CONTINUE limit ({}) exceeded for {} (cycle {}), treating as FAIL",
                                                                                max_continues, change_id, cycle_count
                                                                            );
                                                                            if let Some(ref tx) = event_tx {
                                                                                let _ = tx
                                                                                    .send(ParallelEvent::Log(
                                                                                        LogEntry::warn(format!(
                                                                                            "Acceptance CONTINUE limit exceeded (cycle {}), change will not be archived",
                                                                                            cycle_count
                                                                                        ))
                                                                                        .with_change_id(&change_id)
                                                                                        .with_operation("acceptance")
                                                                                        .with_iteration(acceptance_iteration),
                                                                                    ))
                                                                                    .await;
                                                                            }
                                                                            return WorkspaceResult {
                                                                                change_id,
                                                                                workspace_name,
                                                                                final_revision: None,
                                                                                error: Some(format!(
                                                                                    "Acceptance CONTINUE limit ({}) exceeded",
                                                                                    max_continues
                                                                                )),
                                                                            };
                                                                        } else {
                                                                            info!(
                                                                                "Acceptance requires continuation for {} (attempt {}/{}, cycle {}), retrying acceptance",
                                                                                change_id,
                                                                                continue_count,
                                                                                max_continues,
                                                                                cycle_count
                                                                            );
                                                                            if let Some(ref tx) = event_tx {
                                                                                let _ = tx
                                                                                    .send(ParallelEvent::Log(
                                                                                        LogEntry::info(format!(
                                                                                            "Acceptance requires continuation (attempt {}/{}, cycle {}), retrying",
                                                                                            continue_count,
                                                                                            max_continues,
                                                                                            cycle_count
                                                                                        ))
                                                                                        .with_change_id(&change_id)
                                                                                        .with_operation("acceptance")
                                                                                        .with_iteration(acceptance_iteration),
                                                                                    ))
                                                                                    .await;
                                                                            }
                                                                            continue;
                                                                        }
                                                                    }
                                                                    Ok(crate::orchestration::AcceptanceResult::Fail { findings }) => {
                                                                        warn!(
                                                                            "Acceptance failed for {} with {} findings (cycle {}), returning to apply loop",
                                                                            change_id,
                                                                            findings.len(),
                                                                            cycle_count
                                                                        );
                                                                        if let Err(e) =
                                                                            crate::orchestration::update_tasks_on_acceptance_failure(
                                                                                &change_id,
                                                                                &findings,
                                                                                Some(&workspace_path),
                                                                            )
                                                                            .await
                                                                        {
                                                                            warn!(
                                                                                "Failed to update tasks.md for {}: {}",
                                                                                change_id, e
                                                                            );
                                                                        }
                                                                        if let Some(ref tx) = event_tx {
                                                                            let _ = tx
                                                                                .send(ParallelEvent::Log(
                                                                                    LogEntry::warn(format!(
                                                                                        "Acceptance failed with {} findings, returning to apply loop (cycle {})",
                                                                                        findings.len(),
                                                                                        cycle_count
                                                                                    ))
                                                                                    .with_change_id(&change_id)
                                                                                    .with_operation("acceptance")
                                                                                    .with_iteration(acceptance_iteration),
                                                                                ))
                                                                                .await;
                                                                        }
                                                                        continue;
                                                                    }
                                                                    Ok(crate::orchestration::AcceptanceResult::CommandFailed {
                                                                        error,
                                                                        findings,
                                                                    }) => {
                                                                        error!(
                                                                            "Acceptance command failed for {} (cycle {}): {}",
                                                                            change_id, cycle_count, error
                                                                        );
                                                                        if let Err(e) =
                                                                            crate::orchestration::update_tasks_on_acceptance_failure(
                                                                                &change_id,
                                                                                &findings,
                                                                                Some(&workspace_path),
                                                                            )
                                                                            .await
                                                                        {
                                                                            warn!(
                                                                                "Failed to update tasks.md for {}: {}",
                                                                                change_id, e
                                                                            );
                                                                        }
                                                                        if let Some(ref tx) = event_tx {
                                                                            let _ = tx
                                                                                .send(ParallelEvent::Log(
                                                                                    LogEntry::error(format!(
                                                                                        "Acceptance command failed (cycle {}): {}",
                                                                                        cycle_count, error
                                                                                    ))
                                                                                    .with_change_id(&change_id)
                                                                                    .with_operation("acceptance")
                                                                                    .with_iteration(acceptance_iteration),
                                                                                ))
                                                                                .await;
                                                                        }
                                                                        // Command failed - this is a critical error, don't retry
                                                                        return WorkspaceResult {
                                                                            change_id,
                                                                            workspace_name: workspace.name,
                                                                            final_revision: None,
                                                                            error: Some(format!("Acceptance command failed: {}", error)),
                                                                        };
                                                                    }
                                                                    Ok(crate::orchestration::AcceptanceResult::Cancelled) => {
                                                                        info!("Acceptance cancelled for {}", change_id);
                                                                        return WorkspaceResult {
                                                                            change_id,
                                                                            workspace_name,
                                                                            final_revision: None,
                                                                            error: Some("Acceptance cancelled".to_string()),
                                                                        };
                                                                    }
                                                                    Err(e) => {
                                                                        error!("Acceptance error for {}: {}", change_id, e);
                                                                        return WorkspaceResult {
                                                                            change_id,
                                                                            workspace_name,
                                                                            final_revision: None,
                                                                            error: Some(format!("Acceptance error: {}", e)),
                                                                        };
                                                                    }
                                                                }
                                                            };

                                                            let _ = status_tx_clone.send((workspace_name.clone(), WorkspaceStatus::Archiving));
                                                            if let Some(ref tx) = event_tx {
                                                                let _ = tx
                                                                    .send(ParallelEvent::WorkspaceStatusUpdated {
                                                                        workspace_name: workspace_name.clone(),
                                                                        status: WorkspaceStatus::Archiving,
                                                                    })
                                                                    .await;
                                                                let _ = tx
                                                                    .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                                                                    .await;
                                                            }

                                                            let archive_result = execute_archive_in_workspace(
                                                                &change_id,
                                                                &workspace_path,
                                                                &archive_cmd,
                                                                &config,
                                                                event_tx.clone(),
                                                                vcs_backend,
                                                                hooks.as_ref().map(|h| h.as_ref()),
                                                                Some(&parallel_ctx),
                                                                cancel_token.as_ref(),
                                                                &ai_runner,
                                                                &archive_history,
                                                                &apply_history,
                                                                &shared_stagger_state,
                                                            )
                                                            .await;

                                                            match archive_result {
                                                                Ok(archive_revision) => {
                                                                    agent.clear_acceptance_history(&change_id);

                                                                    if let Some(ref tx) = event_tx {
                                                                        let _ = tx
                                                                            .send(ParallelEvent::ChangeArchived(change_id.clone()))
                                                                            .await;
                                                                    }
                                                                    WorkspaceResult {
                                                                        change_id,
                                                                        workspace_name,
                                                                        final_revision: Some(archive_revision),
                                                                        error: None,
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    warn!("Archive failed for {}: {}", change_id, e);
                                                                    if let Some(ref tx) = event_tx {
                                                                        let _ = tx
                                                                            .send(ParallelEvent::ArchiveFailed {
                                                                                change_id: change_id.clone(),
                                                                                error: e.to_string(),
                                                                            })
                                                                            .await;
                                                                    }
                                                                    WorkspaceResult {
                                                                        change_id,
                                                                        workspace_name,
                                                                        final_revision: None,
                                                                        error: Some(format!("Archive failed: {}", e)),
                                                                    }
                                                                }
                                                            }
                                                        });
                                                    } else {
                                                        warn!("Dynamically added change '{}' not found in openspec", dynamic_id);
                                                        drop(permit);
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("Failed to load dynamically added change '{}': {}", dynamic_id, e);
                                                    drop(permit);
                                                }
                                            }
                                        } else {
                                            drop(permit);
                                        }
                                    } else {
                                        drop(permit);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Task join error: {}", e);
                        }
                    }
                }
                // All tasks completed and channel closed
                else => break,
            }
        }

        Ok(results)
    }

    async fn attempt_merge(
        &self,
        revisions: &[String],
        change_ids: &[String],
        archive_paths: &[PathBuf],
    ) -> Result<MergeAttempt> {
        use crate::execution::archive::verify_archive_completion;

        let _merge_guard = global_merge_lock().lock().await;
        if let Some(reason) = merge::base_dirty_reason(&self.repo_root).await? {
            return Ok(MergeAttempt::Deferred(reason));
        }

        if change_ids.len() != archive_paths.len() {
            return Err(OrchestratorError::GitCommand(format!(
                "Expected {} archive paths for {} changes",
                change_ids.len(),
                archive_paths.len()
            )));
        }

        // Verify that all changes are actually archived before attempting merge
        for (change_id, archive_path) in change_ids.iter().zip(archive_paths.iter()) {
            let verification = verify_archive_completion(change_id, Some(archive_path));
            if !verification.is_success() {
                let reason = format!(
                    "Archive verification failed for '{}': change directory still exists in openspec/changes/. \
                     The change was not properly archived and cannot be merged.",
                    change_id
                );
                warn!("{}", reason);
                return Ok(MergeAttempt::Deferred(reason));
            }
        }

        self.merge_and_resolve(revisions, change_ids).await?;
        Ok(MergeAttempt::Merged)
    }

    pub async fn resolve_merge_for_change(&mut self, change_id: &str) -> Result<()> {
        let workspace_info = self
            .workspace_manager
            .find_existing_workspace(change_id)
            .await
            .map_err(OrchestratorError::from_vcs_error)?
            .ok_or_else(|| OrchestratorError::ChangeNotFound(change_id.to_string()))?;
        let workspace = self
            .workspace_manager
            .reuse_workspace(&workspace_info)
            .await
            .map_err(OrchestratorError::from_vcs_error)?;

        let revisions = vec![workspace.name.clone()];
        let change_ids = vec![change_id.to_string()];

        // Send ResolveStarted to update TUI status
        send_event(
            &self.event_tx,
            ParallelEvent::ResolveStarted {
                change_id: change_id.to_string(),
            },
        )
        .await;

        let archive_paths = vec![workspace.path.clone()];
        match self
            .attempt_merge(&revisions, &change_ids, &archive_paths)
            .await?
        {
            MergeAttempt::Merged => {
                send_event(
                    &self.event_tx,
                    ParallelEvent::CleanupStarted {
                        workspace: workspace.name.clone(),
                    },
                )
                .await;
                if let Err(err) = self
                    .workspace_manager
                    .cleanup_workspace(&workspace.name)
                    .await
                {
                    warn!(
                        "Failed to cleanup worktree '{}' after merge: {}",
                        workspace.name, err
                    );
                } else {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupCompleted {
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;
                }

                // Send ResolveCompleted to update TUI status
                send_event(
                    &self.event_tx,
                    ParallelEvent::ResolveCompleted {
                        change_id: change_id.to_string(),
                        worktree_change_ids: None,
                    },
                )
                .await;

                Ok(())
            }
            MergeAttempt::Deferred(reason) => {
                // Send ResolveFailed to update TUI status
                send_event(
                    &self.event_tx,
                    ParallelEvent::ResolveFailed {
                        change_id: change_id.to_string(),
                        error: reason.clone(),
                    },
                )
                .await;
                Err(OrchestratorError::GitCommand(reason))
            }
        }
    }

    /// Merge revisions and resolve any conflicts
    async fn merge_and_resolve(&self, revisions: &[String], change_ids: &[String]) -> Result<()> {
        let change_ids_vec = change_ids.to_vec();
        let shared_stagger_state = self.shared_stagger_state.clone();
        self.merge_and_resolve_with(revisions, change_ids, |revisions, details| {
            let change_ids_clone = change_ids_vec.clone();
            let shared_stagger_state_clone = shared_stagger_state.clone();
            async move {
                conflict::resolve_conflicts_with_retry(
                    self.workspace_manager.as_ref(),
                    &self.config,
                    &self.event_tx,
                    &revisions,
                    &change_ids_clone,
                    &details,
                    self.max_conflict_retries,
                    shared_stagger_state_clone,
                )
                .await
            }
        })
        .await
    }

    async fn merge_and_resolve_with<'a, F, Fut>(
        &'a self,
        revisions: &'a [String],
        change_ids: &'a [String],
        mut resolve_conflicts: F,
    ) -> Result<()>
    where
        F: FnMut(Vec<String>, String) -> Fut,
        Fut: std::future::Future<Output = Result<()>> + Send + 'a,
    {
        let max_attempts = self.max_conflict_retries.max(1);

        send_event(
            &self.event_tx,
            ParallelEvent::MergeStarted {
                revisions: revisions.to_vec(),
            },
        )
        .await;

        if matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            let base_revision = self.workspace_manager.get_current_revision().await?;
            let target_branch = self.workspace_manager.original_branch().ok_or_else(|| {
                OrchestratorError::GitCommand("Original branch not initialized".to_string())
            })?;

            if change_ids.len() != revisions.len() {
                return Err(OrchestratorError::GitCommand(format!(
                    "Expected {} change_ids for {} revisions",
                    revisions.len(),
                    change_ids.len()
                )));
            }

            conflict::resolve_merges_with_retry(conflict::ResolveMergesWithRetryArgs {
                workspace_manager: self.workspace_manager.as_ref(),
                config: &self.config,
                event_tx: &self.event_tx,
                revisions,
                change_ids,
                target_branch: target_branch.as_str(),
                base_revision: base_revision.as_str(),
                max_retries: max_attempts,
                shared_stagger_state: self.shared_stagger_state.clone(),
            })
            .await?;

            self.verify_merge_commits(&base_revision, &target_branch, change_ids)
                .await?;

            let merge_revision = self.workspace_manager.get_current_revision().await?;
            send_event(
                &self.event_tx,
                ParallelEvent::MergeCompleted {
                    change_id: change_ids[0].clone(),
                    revision: merge_revision,
                },
            )
            .await;
            return Ok(());
        }

        for attempt in 1..=max_attempts {
            info!(
                "Merge attempt {}/{} for revisions: {}",
                attempt,
                max_attempts,
                revisions.join(", ")
            );

            let merge_result = self.workspace_manager.merge_workspaces(revisions).await;

            match merge_result {
                Ok(merge_revision) => {
                    if attempt > 1 {
                        info!("Merge succeeded after {} attempts", attempt);
                    }
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeCompleted {
                            change_id: change_ids[0].clone(),
                            revision: merge_revision,
                        },
                    )
                    .await;

                    // Send ResolveCompleted for each change_id if there were conflicts resolved
                    if attempt > 1 {
                        for change_id in change_ids {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveCompleted {
                                    change_id: change_id.to_string(),
                                    worktree_change_ids: None,
                                },
                            )
                            .await;
                        }
                    }

                    return Ok(());
                }
                Err(VcsError::Conflict { details, .. }) => {
                    let conflict_files =
                        conflict::detect_conflicts(self.workspace_manager.as_ref()).await?;
                    warn!(
                        "Merge conflict detected on attempt {}/{}",
                        attempt, max_attempts
                    );
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeConflict {
                            files: conflict_files,
                        },
                    )
                    .await;

                    if attempt >= max_attempts {
                        let error_msg = format!(
                            "Merge conflict unresolved after {} attempts: {}",
                            max_attempts, details
                        );
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ConflictResolutionFailed {
                                error: error_msg.clone(),
                            },
                        )
                        .await;

                        // Send ResolveFailed for each change_id to update TUI status
                        for change_id in change_ids {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: change_id.to_string(),
                                    error: error_msg.clone(),
                                },
                            )
                            .await;
                        }

                        return Err(OrchestratorError::from_vcs_error(VcsError::Conflict {
                            backend: self.workspace_manager.backend_type(),
                            details: error_msg,
                        }));
                    }

                    info!(
                        "Resolving merge conflicts (attempt {}/{}).",
                        attempt, max_attempts
                    );

                    // Send ResolveStarted for each change_id to update TUI status
                    for change_id in change_ids {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ResolveStarted {
                                change_id: change_id.to_string(),
                            },
                        )
                        .await;
                    }

                    if let Err(err) = resolve_conflicts(revisions.to_vec(), details.clone()).await {
                        warn!(
                            "Conflict resolution failed on attempt {}/{}: {}",
                            attempt, max_attempts, err
                        );

                        // Send ResolveFailed for each change_id to update TUI status
                        for change_id in change_ids {
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: change_id.to_string(),
                                    error: err.to_string(),
                                },
                            )
                            .await;
                        }

                        return Err(err);
                    }
                    info!("Conflict resolution completed, retrying merge");

                    // Note: ResolveCompleted will be sent when the merge succeeds
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    async fn verify_merge_commits(
        &self,
        base_revision: &str,
        _target_branch: &str,
        change_ids: &[String],
    ) -> Result<()> {
        if matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            let repo_root = self.workspace_manager.repo_root();
            let missing =
                git_commands::missing_merge_commits_since(repo_root, base_revision, change_ids)
                    .await
                    .map_err(OrchestratorError::from_vcs_error)?;
            if !missing.is_empty() {
                return Err(OrchestratorError::GitCommand(format!(
                    "Missing merge commit message containing change_id(s): {}",
                    missing.join(", ")
                )));
            }
        }

        Ok(())
    }
}

pub async fn resolve_deferred_merge(
    repo_root: PathBuf,
    config: OrchestratorConfig,
    change_id: &str,
) -> Result<()> {
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    executor.resolve_merge_for_change(change_id).await
}

#[cfg(test)]
mod tests;
