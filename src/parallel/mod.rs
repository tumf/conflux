//! Parallel execution coordinator for VCS workspace-based parallel change application.
//!
//! This module manages the parallel execution of changes using Git worktrees,
//! including workspace creation, apply command execution, merge, and cleanup.

mod cleanup;
mod conflict;
mod events;
mod executor;
mod types;

// Re-export ExecutionEvent as ParallelEvent for backward compatibility
pub use crate::events::ExecutionEvent as ParallelEvent;
pub use types::{FailedChangeTracker, WorkspaceResult};

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::analyzer::{extract_change_dependencies, ParallelGroup};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::events::LogEntry;
use crate::execution::archive::ensure_archive_commit;
use crate::execution::state::{detect_workspace_state, WorkspaceState};
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
use executor::{execute_apply_in_workspace, execute_archive_in_workspace, ParallelHookContext};

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
    /// Hook runner for executing hooks (optional)
    hooks: Option<Arc<HookRunner>>,
    /// Cancellation token for force stop cleanup
    cancel_token: Option<CancellationToken>,
    /// Last queue change timestamp for debouncing re-analysis
    last_queue_change_at: Arc<Mutex<Option<std::time::Instant>>>,
    /// Dynamic queue for runtime change additions (TUI mode)
    dynamic_queue: Option<Arc<crate::tui::queue::DynamicQueue>>,
    /// Shared AI command runner for stagger coordination
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3)
    ai_runner: AiCommandRunner,
    /// Shared stagger state for resolve operations
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 4.1-4.3)
    shared_stagger_state: SharedStaggerState,
}

pub async fn base_dirty_reason(repo_root: &Path) -> Result<Option<String>> {
    let is_git_repo = git_commands::check_git_repo(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if !is_git_repo {
        return Ok(None);
    }

    let merge_in_progress = git_commands::is_merge_in_progress(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if merge_in_progress {
        return Ok(Some("Merge in progress (MERGE_HEAD exists)".to_string()));
    }

    let (has_changes, status) = git_commands::has_uncommitted_changes(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if has_changes {
        let trimmed = status.trim();
        let reason = if trimmed.is_empty() {
            "Working tree has uncommitted changes".to_string()
        } else {
            format!("Working tree has uncommitted changes:\n{}", trimmed)
        };
        return Ok(Some(reason));
    }

    Ok(None)
}

enum MergeAttempt {
    Merged,
    Deferred(String),
}

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
        // Create a unique temp directory for this execution
        let base_dir = if let Some(configured_dir) = config.get_workspace_base_dir() {
            // User configured a specific directory
            PathBuf::from(configured_dir)
        } else {
            // Use tempfile to create a unique temp directory
            match tempfile::Builder::new().prefix("openspec-ws-").tempdir() {
                Ok(temp_dir) => {
                    // Keep the path but leak the TempDir so it doesn't get cleaned up immediately
                    let path = temp_dir.path().to_path_buf();
                    std::mem::forget(temp_dir);
                    path
                }
                Err(e) => {
                    error!("Failed to create temp directory: {}", e);
                    // Fallback to a fixed temp directory
                    std::env::temp_dir().join("openspec-workspaces-fallback")
                }
            }
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

        // Create shared stagger state for AI command coordination
        let shared_stagger_state = Arc::new(Mutex::new(None));

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
            hooks: None,
            cancel_token: None,
            last_queue_change_at,
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
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

    /// Check if re-analysis should proceed based on debounce logic.
    ///
    /// Returns `true` if:
    /// - A slot is available (a change just completed), AND
    /// - Either no recent queue changes OR 10 seconds have passed since the last queue change
    ///
    /// This prevents immediate re-analysis when the queue changes, giving time for
    /// multiple changes to be queued before triggering expensive re-analysis.
    pub async fn should_reanalyze(&self, slot_available: bool) -> bool {
        if !slot_available {
            return false;
        }

        let last_change = self.last_queue_change_at.lock().await;
        match *last_change {
            None => {
                // No recent queue changes, proceed with re-analysis
                true
            }
            Some(timestamp) => {
                let elapsed = timestamp.elapsed();
                let debounce_duration = std::time::Duration::from_secs(10);

                if elapsed >= debounce_duration {
                    info!(
                        "Debounce period elapsed ({:.1}s >= 10s), proceeding with re-analysis",
                        elapsed.as_secs_f64()
                    );
                    true
                } else {
                    info!(
                        "Debounce period active ({:.1}s < 10s), deferring re-analysis",
                        elapsed.as_secs_f64()
                    );
                    false
                }
            }
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token
            .as_ref()
            .is_some_and(|token| token.is_cancelled())
    }

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

    /// Execute groups in topological order
    pub async fn execute_groups(&mut self, groups: Vec<ParallelGroup>) -> Result<()> {
        if groups.is_empty() {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!("Executing {} groups in parallel mode", groups.len());

        // Extract change-level dependencies from groups and set them in the tracker
        let change_deps = extract_change_dependencies(&groups);
        self.failed_tracker.set_dependencies(change_deps.clone());
        self.change_dependencies = change_deps;

        // Calculate total changes count
        let total_changes: usize = groups.iter().map(|g| g.changes.len()).sum();
        let mut changes_processed: usize = 0;

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

        for group in groups {
            if self.is_cancelled() {
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn("Parallel execution cancelled")),
                )
                .await;
                return Err(OrchestratorError::AgentCommand("Cancelled".to_string()));
            }
            let group_size = group.changes.len();
            self.execute_group(&group, total_changes, changes_processed)
                .await?;
            changes_processed += group_size;
        }

        if self.has_merge_deferred() {
            send_event(&self.event_tx, ParallelEvent::Stopped).await;
        } else {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        }
        Ok(())
    }

    /// Execute changes with dynamic re-analysis after each group completes.
    ///
    /// This method analyzes the remaining changes after each group completes,
    /// allowing the LLM to reconsider dependencies based on the current state.
    pub async fn execute_with_reanalysis<F>(
        &mut self,
        mut changes: Vec<crate::openspec::Change>,
        analyzer: F,
    ) -> Result<()>
    where
        F: Fn(
                &[crate::openspec::Change],
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Vec<ParallelGroup>> + Send + '_>,
            > + Send
            + Sync,
    {
        if changes.is_empty() {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
            return Ok(());
        }

        info!(
            "Starting execution with re-analysis for {} changes",
            changes.len()
        );

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

        let mut group_counter = 1u32;
        let initial_total_changes = changes.len();
        let mut changes_processed: usize = 0;

        while !changes.is_empty() {
            if self.is_cancelled() {
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::warn("Parallel execution cancelled")),
                )
                .await;
                return Err(OrchestratorError::AgentCommand("Cancelled".to_string()));
            }

            // Check dynamic queue for newly added changes (TUI mode)
            if let Some(queue) = &self.dynamic_queue {
                let mut queue_changed = false;
                while let Some(dynamic_id) = queue.pop().await {
                    // Check if change is already in the list
                    if !changes.iter().any(|c| c.id == dynamic_id) {
                        // Load change details from openspec by listing all changes and filtering
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
                                    changes.push(new_change);
                                    queue_changed = true;
                                } else {
                                    warn!(
                                        "Dynamically added change '{}' not found in openspec",
                                        dynamic_id
                                    );
                                    send_event(
                                        &self.event_tx,
                                        ParallelEvent::Log(LogEntry::warn(format!(
                                            "Dynamically added change '{}' not found in openspec",
                                            dynamic_id
                                        ))),
                                    )
                                    .await;
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to load dynamically added change '{}': {}",
                                    dynamic_id, e
                                );
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::Log(LogEntry::warn(format!(
                                        "Failed to load dynamically added change '{}': {}",
                                        dynamic_id, e
                                    ))),
                                )
                                .await;
                            }
                        }
                    }
                }

                // Update queue change timestamp if items were added
                if queue_changed {
                    let mut last_change = self.last_queue_change_at.lock().await;
                    *last_change = Some(std::time::Instant::now());
                }
            }

            // Filter out changes that depend on failed changes
            let executable_changes: Vec<_> = changes
                .iter()
                .filter(|c| {
                    if let Some(reason) = self.skip_reason_for_change(&c.id) {
                        warn!("Excluding '{}' from analysis: {}", c.id, reason);
                        // Emit skip event
                        // Note: We can't async here, so we'll emit after filtering
                        false
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            // Emit skip events for filtered changes
            for change in &changes {
                if let Some(reason) = self.skip_reason_for_change(&change.id) {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ChangeSkipped {
                            change_id: change.id.clone(),
                            reason,
                        },
                    )
                    .await;
                }
            }

            // Update changes to only include executable ones
            changes = executable_changes;

            if changes.is_empty() {
                info!("All remaining changes are blocked by dependencies, stopping");
                break;
            }

            // Check debounce: a slot is available (after previous group completed)
            // Skip debounce check on first iteration to start immediately
            if group_counter > 1 {
                let slot_available = true; // Slot is available after previous completion
                if !self.should_reanalyze(slot_available).await {
                    // Debounce period still active, wait before re-analyzing
                    let wait_duration = std::time::Duration::from_secs(1);
                    info!("Debounce active, waiting {:?} before retry", wait_duration);
                    tokio::time::sleep(wait_duration).await;
                    continue; // Retry the loop after waiting
                }
            } else {
                info!("First iteration, skipping debounce check");
            }

            // Analyze remaining changes to get the next group
            info!(
                "Analyzing {} remaining changes for next group",
                changes.len()
            );
            send_event(
                &self.event_tx,
                ParallelEvent::AnalysisStarted {
                    remaining_changes: changes.len(),
                },
            )
            .await;

            let groups = analyzer(&changes).await;

            if groups.is_empty() {
                warn!("No groups returned from analysis");
                break;
            }

            // Extract change-level dependencies for this iteration
            let change_deps = extract_change_dependencies(&groups);
            self.failed_tracker.set_dependencies(change_deps.clone());
            self.change_dependencies = change_deps;

            // Execute only the first group (no dependencies)
            let first_group = ParallelGroup {
                id: group_counter,
                changes: groups[0].changes.clone(),
                depends_on: Vec::new(),
            };

            let group_size = first_group.changes.len();
            info!(
                "Executing group {} with {} changes: {:?}",
                first_group.id, group_size, first_group.changes
            );

            self.execute_group(&first_group, initial_total_changes, changes_processed)
                .await?;

            // Remove completed changes from the list
            let completed_set: std::collections::HashSet<_> = first_group.changes.iter().collect();
            changes.retain(|c| !completed_set.contains(&c.id));

            changes_processed += group_size;
            group_counter += 1;
        }

        if self.has_merge_deferred() {
            send_event(&self.event_tx, ParallelEvent::Stopped).await;
        } else {
            send_event(&self.event_tx, ParallelEvent::AllCompleted).await;
        }
        Ok(())
    }

    /// Execute a single group of changes
    async fn execute_group(
        &mut self,
        group: &ParallelGroup,
        total_changes: usize,
        changes_processed: usize,
    ) -> Result<()> {
        if self.is_cancelled() {
            send_event(
                &self.event_tx,
                ParallelEvent::Log(LogEntry::warn("Parallel execution cancelled")),
            )
            .await;
            return Err(OrchestratorError::AgentCommand("Cancelled".to_string()));
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
                "All changes in group {} were skipped due to blocked dependencies",
                group.id
            );
            send_event(
                &self.event_tx,
                ParallelEvent::GroupCompleted { group_id: group.id },
            )
            .await;
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

        send_event(
            &self.event_tx,
            ParallelEvent::GroupStarted {
                group_id: group.id,
                changes: changes_to_execute.clone(),
            },
        )
        .await;

        // Create cleanup guard to ensure workspaces are cleaned up on early errors
        let mut cleanup_guard = WorkspaceCleanupGuard::new(
            self.workspace_manager.backend_type(),
            self.repo_root.clone(),
        );

        // Create or reuse workspaces for all changes in the group
        // If resume is enabled (default), try to find existing workspaces first
        let mut workspaces: Vec<Workspace> = Vec::new();
        let mut archived_results: Vec<WorkspaceResult> = Vec::new();
        let mut archived_workspaces: Vec<Workspace> = Vec::new();
        for change_id in &changes_to_execute {
            // Try to find and reuse existing workspace (unless --no-resume is set)
            let workspace = if self.no_resume {
                None
            } else {
                match self
                    .workspace_manager
                    .find_existing_workspace(change_id)
                    .await
                {
                    Ok(Some(workspace_info)) => {
                        // Found existing workspace, reuse it
                        info!(
                            "Resuming existing workspace for '{}' (last modified: {:?})",
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

            let (workspace, resumed) = match workspace {
                Some(ws) => {
                    // Track workspace in cleanup guard before adding to list
                    cleanup_guard.track(ws.name.clone(), ws.path.clone());

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceResumed {
                            change_id: change_id.clone(),
                            workspace: ws.name.clone(),
                        },
                    )
                    .await;

                    // Send ProcessingStarted event early to show processing status in TUI
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ProcessingStarted(change_id.clone()),
                    )
                    .await;

                    (ws, true)
                }
                None => {
                    // Create new workspace from the base revision
                    match self
                        .workspace_manager
                        .create_workspace(change_id, Some(&base_revision))
                        .await
                    {
                        Ok(ws) => {
                            // Track workspace in cleanup guard before adding to list
                            cleanup_guard.track(ws.name.clone(), ws.path.clone());

                            send_event(
                                &self.event_tx,
                                ParallelEvent::WorkspaceCreated {
                                    change_id: change_id.clone(),
                                    workspace: ws.name.clone(),
                                },
                            )
                            .await;

                            // Send ProcessingStarted event early to show processing status in TUI
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ProcessingStarted(change_id.clone()),
                            )
                            .await;

                            (ws, false)
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
                            // cleanup_guard will clean up previously created workspaces on drop
                            return Err(e.into());
                        }
                    }
                }
            };

            // Detect workspace state for idempotent resume
            let workspace_state = if resumed {
                // Get the original branch for state detection
                let original_branch =
                    self.workspace_manager.original_branch().ok_or_else(|| {
                        OrchestratorError::GitCommand("Original branch not initialized".to_string())
                    })?;

                match detect_workspace_state(change_id, &workspace.path, &original_branch).await {
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
                    info!(
                        "Change '{}' already merged to main in workspace '{}', skipping all operations",
                        change_id, workspace.name
                    );
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
                    // Archive complete, skip apply/archive, only merge
                    info!(
                        "Change '{}' already archived in workspace '{}', skipping apply/archive, will merge",
                        change_id, workspace.name
                    );
                }
                WorkspaceState::Applied
                | WorkspaceState::Applying { .. }
                | WorkspaceState::Created => {
                    // These states require apply (or resume apply)
                    // They will be handled normally by the apply execution
                }
            }

            // For Archived state, ensure archive commit and add to archived_results for merge
            if matches!(workspace_state, WorkspaceState::Archived) {
                info!(
                    "Change '{}' already archived in workspace '{}', skipping apply/archive",
                    change_id, workspace.name
                );

                send_event(
                    &self.event_tx,
                    ParallelEvent::ArchiveStarted(change_id.clone()),
                )
                .await;

                let resolve_agent = AgentRunner::new(self.config.clone());
                let change_id_owned = change_id.clone();
                let event_tx = self.event_tx.clone();
                if let Err(err) = ensure_archive_commit(
                    change_id,
                    &workspace.path,
                    &resolve_agent,
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
                                        iteration: None,
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

            workspaces.push(workspace);
        }

        for result in &archived_results {
            if result.final_revision.is_some() {
                let revisions = vec![result.workspace_name.clone()];
                let change_ids = vec![result.change_id.clone()];

                info!(
                    "Merging archived {} (workspace: {})",
                    result.change_id, result.workspace_name
                );
                let merge_result = self.attempt_merge(&revisions, &change_ids).await;
                match merge_result {
                    Ok(MergeAttempt::Merged) => {}
                    Ok(MergeAttempt::Deferred(reason)) => {
                        self.merge_deferred_changes.insert(result.change_id.clone());
                        send_event(
                            &self.event_tx,
                            ParallelEvent::MergeDeferred {
                                change_id: result.change_id.clone(),
                                reason,
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
        // Each task: apply -> (if success) -> archive
        let mut results = archived_results;
        if !workspaces.is_empty() {
            let apply_results = match self
                .execute_apply_and_archive_parallel(
                    &workspaces,
                    Some(group.id),
                    total_changes,
                    changes_processed,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = format!("Failed to execute applies: {}", e);
                    error!("{}", error_msg);
                    send_event(&self.event_tx, ParallelEvent::Error { message: error_msg }).await;
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

        // Report failures and mark them in the tracker for dependent skipping
        // Also preserve workspaces for failed changes (do not cleanup)
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

        // If all failed, we don't have an error but continue to the next group
        // The dependent changes will be skipped automatically
        if successful.is_empty() && !results.is_empty() {
            warn!(
                "All changes in group {} failed, dependent changes will be skipped",
                group.id
            );
            send_event(
                &self.event_tx,
                ParallelEvent::GroupCompleted { group_id: group.id },
            )
            .await;
            return Ok(());
        }

        // Note: Individual merging is now done in execute_apply_and_archive_parallel
        // immediately after each change is archived. Group-level merge is no longer needed.

        // Cleanup only successful workspaces (preserve failed ones)
        let failed_workspace_names: std::collections::HashSet<_> =
            failed.iter().map(|r| r.workspace_name.clone()).collect();
        let workspace_statuses: std::collections::HashMap<_, _> = self
            .workspace_manager
            .workspaces()
            .into_iter()
            .map(|workspace| (workspace.name, workspace.status))
            .collect();
        let mut cleanup_workspaces = workspaces.clone();
        cleanup_workspaces.extend(archived_workspaces);
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

        // Commit the cleanup guard since normal cleanup succeeded
        // This prevents double-cleanup on drop
        cleanup_guard.commit();

        send_event(
            &self.event_tx,
            ParallelEvent::GroupCompleted { group_id: group.id },
        )
        .await;

        Ok(())
    }

    /// Execute apply + archive in parallel across workspaces
    /// Each task: apply -> (if success) -> archive
    /// Archive starts immediately after apply completes for each change
    async fn execute_apply_and_archive_parallel(
        &mut self,
        workspaces: &[Workspace],
        group_index: Option<u32>,
        total_changes: usize,
        changes_processed: usize,
    ) -> Result<Vec<WorkspaceResult>> {
        let max_concurrent = self.workspace_manager.max_concurrent();
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let mut join_set: JoinSet<WorkspaceResult> = JoinSet::new();
        let total_changes_in_group = workspaces.len();

        for workspace in workspaces {
            let sem = semaphore.clone();
            let change_id = workspace.change_id.clone();
            let workspace_path = workspace.path.clone();
            let workspace_name = workspace.name.clone();
            let repo_root = self.repo_root.clone();
            let apply_cmd = self.apply_command.clone();
            let archive_cmd = self.archive_command.clone();
            let config = self.config.clone();
            let event_tx = self.event_tx.clone();
            let vcs_backend = self.workspace_manager.backend_type();
            let hooks = self.hooks.clone();
            let cancel_token = self.cancel_token.clone();

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
                // Acquire semaphore inside spawn to allow all tasks to be created
                let _permit = sem.acquire_owned().await.unwrap();

                // Step 1: Execute apply
                let apply_result = execute_apply_in_workspace(
                    &change_id,
                    &workspace_path,
                    &repo_root,
                    &apply_cmd,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    hooks.as_ref().map(|h| h.as_ref()),
                    Some(&parallel_ctx),
                    cancel_token.as_ref(),
                )
                .await;

                match apply_result {
                    Ok(apply_revision) => {
                        // Send ApplyCompleted event
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ApplyCompleted {
                                    change_id: change_id.clone(),
                                    revision: apply_revision.clone(),
                                })
                                .await;
                        }

                        // Step 2: Execute archive immediately after apply succeeds
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ArchiveStarted(change_id.clone()))
                                .await;
                        }

                        let archive_result = execute_archive_in_workspace(
                            &change_id,
                            &workspace_path,
                            &repo_root,
                            &archive_cmd,
                            &config,
                            event_tx.clone(),
                            vcs_backend,
                            hooks.as_ref().map(|h| h.as_ref()),
                            Some(&parallel_ctx),
                            cancel_token.as_ref(),
                        )
                        .await;

                        match archive_result {
                            Ok(archive_revision) => {
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
                    }
                    Err(e) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ApplyFailed {
                                    change_id: change_id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                        WorkspaceResult {
                            change_id,
                            workspace_name,
                            final_revision: None,
                            error: Some(e.to_string()),
                        }
                    }
                }
                // _permit is dropped here, releasing semaphore
            });
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
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
                        let merge_result = self.attempt_merge(&revisions, &change_ids).await;
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
                                send_event(
                                    &self.event_tx,
                                    ParallelEvent::MergeDeferred {
                                        change_id: workspace_result.change_id.clone(),
                                        reason,
                                    },
                                )
                                .await;
                            }
                            Err(e) => {
                                error!(
                                    "Failed to merge {} (workspace: {}): {}",
                                    workspace_result.change_id, workspace_result.workspace_name, e
                                );
                                // Merge failure is critical - return error immediately
                                return Err(e);
                            }
                        }
                    }
                    results.push(workspace_result);
                }
                Err(e) => {
                    warn!("Task join error: {}", e);
                }
            }
        }

        Ok(results)
    }

    async fn attempt_merge(
        &self,
        revisions: &[String],
        change_ids: &[String],
    ) -> Result<MergeAttempt> {
        let _merge_guard = global_merge_lock().lock().await;
        if let Some(reason) = base_dirty_reason(&self.repo_root).await? {
            return Ok(MergeAttempt::Deferred(reason));
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

        match self.attempt_merge(&revisions, &change_ids).await? {
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
        self.merge_and_resolve_with(revisions, change_ids, |revisions, details| async move {
            conflict::resolve_conflicts_with_retry(
                self.workspace_manager.as_ref(),
                &self.config,
                &self.event_tx,
                &revisions,
                &details,
                self.max_conflict_retries,
            )
            .await
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
mod tests {

    use super::*;
    use crate::vcs::{VcsResult, VcsWarning, WorkspaceInfo};
    use async_trait::async_trait;
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::process::Command;

    #[test]
    fn test_parallel_executor_creation() {
        let config = OrchestratorConfig::default();
        let repo_root = PathBuf::from("/tmp/test-repo");
        let executor = ParallelExecutor::new(repo_root, config, None);

        assert_eq!(executor.max_conflict_retries, 3);
    }

    #[allow(dead_code)]
    struct TestWorkspaceManager {
        merge_calls: Arc<AtomicUsize>,
        conflict_files: Vec<String>,
        #[allow(dead_code)]
        repo_root: PathBuf,
    }

    impl TestWorkspaceManager {
        #[allow(dead_code)]
        fn new(merge_calls: Arc<AtomicUsize>) -> Self {
            Self {
                merge_calls,
                conflict_files: vec!["conflict.txt".to_string()],
                repo_root: PathBuf::from("/tmp/test-repo"),
            }
        }
    }

    #[async_trait]
    impl WorkspaceManager for TestWorkspaceManager {
        fn backend_type(&self) -> VcsBackend {
            VcsBackend::Git
        }

        async fn check_available(&self) -> VcsResult<bool> {
            Ok(true)
        }

        async fn prepare_for_parallel(&self) -> VcsResult<Option<VcsWarning>> {
            Ok(None)
        }

        async fn get_current_revision(&self) -> VcsResult<String> {
            Ok("rev".to_string())
        }

        async fn create_workspace(
            &mut self,
            change_id: &str,
            _base_revision: Option<&str>,
        ) -> VcsResult<Workspace> {
            Ok(Workspace {
                name: change_id.to_string(),
                path: PathBuf::from("/tmp/test-workspace"),
                change_id: change_id.to_string(),
                base_revision: "base".to_string(),
                status: WorkspaceStatus::Created,
            })
        }

        fn update_workspace_status(&mut self, _workspace_name: &str, _status: WorkspaceStatus) {}

        async fn merge_workspaces(&self, _revisions: &[String]) -> VcsResult<String> {
            let attempt = self.merge_calls.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                Err(VcsError::Conflict {
                    backend: VcsBackend::Git,
                    details: "conflict".to_string(),
                })
            } else {
                Ok("merge-rev".to_string())
            }
        }

        async fn cleanup_workspace(&mut self, _workspace_name: &str) -> VcsResult<()> {
            Ok(())
        }

        async fn cleanup_all(&mut self) -> VcsResult<()> {
            Ok(())
        }

        fn max_concurrent(&self) -> usize {
            1
        }

        fn workspaces(&self) -> Vec<Workspace> {
            Vec::new()
        }

        async fn list_worktree_change_ids(&self) -> VcsResult<HashSet<String>> {
            Ok(HashSet::new())
        }

        fn conflict_resolution_prompt(&self) -> &'static str {
            "test prompt"
        }

        async fn snapshot_working_copy(&self, _workspace_path: &Path) -> VcsResult<()> {
            Ok(())
        }

        async fn set_commit_message(
            &self,
            _workspace_path: &Path,
            _message: &str,
        ) -> VcsResult<()> {
            Ok(())
        }

        async fn create_iteration_snapshot(
            &self,
            _workspace_path: &Path,
            _change_id: &str,
            _iteration: u32,
            _completed: u32,
            _total: u32,
        ) -> VcsResult<()> {
            Ok(())
        }

        async fn squash_wip_commits(
            &self,
            _workspace_path: &Path,
            _change_id: &str,
            _final_iteration: u32,
        ) -> VcsResult<()> {
            Ok(())
        }

        async fn get_revision_in_workspace(&self, _workspace_path: &Path) -> VcsResult<String> {
            Ok("rev".to_string())
        }

        async fn get_status(&self) -> VcsResult<String> {
            Ok(String::new())
        }

        async fn get_log_for_revisions(&self, _revisions: &[String]) -> VcsResult<String> {
            Ok(String::new())
        }

        async fn detect_conflicts(&self) -> VcsResult<Vec<String>> {
            Ok(self.conflict_files.clone())
        }

        fn forget_workspace_sync(&self, _workspace_name: &str) {}

        fn repo_root(&self) -> &Path {
            &self.repo_root
        }

        fn original_branch(&self) -> Option<String> {
            Some("main".to_string())
        }

        async fn find_existing_workspace(
            &mut self,
            _change_id: &str,
        ) -> VcsResult<Option<WorkspaceInfo>> {
            Ok(None)
        }

        async fn reuse_workspace(
            &mut self,
            workspace_info: &WorkspaceInfo,
        ) -> VcsResult<Workspace> {
            Ok(Workspace {
                name: workspace_info.workspace_name.clone(),
                path: workspace_info.path.clone(),
                change_id: workspace_info.change_id.clone(),
                base_revision: "base".to_string(),
                status: WorkspaceStatus::Created,
            })
        }
    }

    async fn init_git_repo(repo_root: &Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
    }

    async fn commit_workspace_change(
        workspace: &Workspace,
        filename: &str,
        contents: &str,
        message: &str,
    ) {
        std::fs::write(workspace.path.join(filename), contents).unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&workspace.path)
            .output()
            .await
            .unwrap();
    }

    #[test]
    fn test_skip_reason_for_merge_deferred_dependency() {
        let merge_calls = Arc::new(AtomicUsize::new(0));
        let manager = TestWorkspaceManager::new(merge_calls);
        let mut change_dependencies = HashMap::new();
        change_dependencies.insert("change-b".to_string(), vec!["change-a".to_string()]);
        let mut merge_deferred_changes = HashSet::new();
        merge_deferred_changes.insert("change-a".to_string());

        // Create test AI runner
        let shared_stagger_state = Arc::new(Mutex::new(None));
        let config = OrchestratorConfig::default();
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
            retry_error_patterns: default_retry_patterns(),
            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 1,
            repo_root: PathBuf::from("/tmp/test-repo"),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies,
            merge_deferred_changes,
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        assert_eq!(
            executor.skip_reason_for_change("change-b"),
            Some("Dependency 'change-a' awaiting merge".to_string())
        );
        assert!(executor.skip_reason_for_change("change-c").is_none());
    }

    #[tokio::test]
    async fn test_resolve_merge_aborts_when_base_dirty() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        init_git_repo(repo_root).await;

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;

        std::fs::write(repo_root.join("dirty.txt"), "dirty").unwrap();

        let result = resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a").await;
        assert!(result.is_err());

        let merge_log = Command::new("git")
            .args(["log", "--merges", "--format=%s"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
        assert!(!merge_messages.contains("Merge change: change-a"));
    }

    #[tokio::test]
    async fn test_resolve_merge_executes_selected_change_only() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let worktree_dir = tempfile::TempDir::new().unwrap();
        let base_dir = worktree_dir.path().join("worktrees");
        let resolver_dir = tempfile::TempDir::new().unwrap();
        let resolver_script = resolver_dir.path().join("merge-resolver.sh");

        init_git_repo(repo_root).await;

        let config = OrchestratorConfig {
            resolve_command: Some(format!("sh {}", resolver_script.display())),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        let workspace_b = manager.create_workspace("change-b", None).await.unwrap();
        commit_workspace_change(&workspace_a, "change-a.txt", "A", "Apply: change-a").await;
        commit_workspace_change(&workspace_b, "change-b.txt", "B", "Apply: change-b").await;

        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n",
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        resolve_deferred_merge(repo_root.to_path_buf(), config, "change-a")
            .await
            .unwrap();

        let merge_log = Command::new("git")
            .args(["log", "--merges", "--format=%s"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
        assert!(merge_messages.contains("Merge change: change-a"));
        assert!(!merge_messages.contains("Merge change: change-b"));
    }

    #[tokio::test]
    async fn test_merge_uses_resolve_command_with_change_ids() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

        std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-b"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-b' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-b' {}\n",
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name,
            workspace_b.path.to_string_lossy(),
            workspace_b.name,
            workspace_b.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name, workspace_b.name];
        let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_merge_allows_non_merge_head_after_merges() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

        std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-b"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-b' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-b' {}\n\
            echo 'post-merge' >> README.md\n\
            git add -A\n\
            git commit -m 'Post-merge commit'\n",
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name,
            workspace_b.path.to_string_lossy(),
            workspace_b.name,
            workspace_b.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name, workspace_b.name];
        let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_merge_retries_when_merge_left_in_progress() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();

        std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\n\
            if [ -f .git/merge-in-progress-marker ]; then\n\
              git commit -m 'Merge change: change-a'\n\
              exit 0\n\
            fi\n\
            git checkout main\n\
            git merge --no-ff --no-commit {}\n\
            touch .git/merge-in-progress-marker\n",
            workspace_a.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name];
        let change_ids = vec!["change-a".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_merge_retries_when_merge_commit_missing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

        std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        std::fs::write(workspace_b.path.join("change-b.txt"), "B").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-b"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            if [ -f .git/merge-missing-marker ]; then\n\
              cd \"{}\"\n\
              git checkout {}\n\
              git merge --no-ff -m 'Pre-sync base into change-b' main\n\
              cd \"$ROOT\"\n\
              git checkout main\n\
              git merge --no-ff -m 'Merge change: change-b' {}\n\
              exit 0\n\
            fi\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            touch .git/merge-missing-marker\n",
            workspace_b.path.to_string_lossy(),
            workspace_b.name,
            workspace_b.name,
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name, workspace_b.name];
        let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();

        let merge_log = Command::new("git")
            .args(["log", "--merges", "--format=%s"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        let merge_messages = String::from_utf8_lossy(&merge_log.stdout);
        assert!(merge_messages.contains("Merge change: change-a"));
        assert!(merge_messages.contains("Merge change: change-b"));
    }

    #[tokio::test]
    async fn test_merge_resolves_conflict_with_resolve_command() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("conflict.txt"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 2, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();
        let workspace_b = manager.create_workspace("change-b", None).await.unwrap();

        std::fs::write(workspace_a.path.join("conflict.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        std::fs::write(workspace_b.path.join("conflict.txt"), "B").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-b"])
            .current_dir(&workspace_b.path)
            .output()
            .await
            .unwrap();

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff -m 'Pre-sync base into change-a' main\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-a' {}\n\
            cd \"{}\"\n\
            git checkout {}\n\
            if ! git merge --no-ff -m 'Pre-sync base into change-b' main; then\n\
              if git rev-parse -q --verify MERGE_HEAD >/dev/null 2>&1; then\n\
                git checkout --ours conflict.txt\n\
                git add -A\n\
                git commit -m 'Pre-sync base into change-b'\n\
              else\n\
                exit 1\n\
              fi\n\
            fi\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff -m 'Merge change: change-b' {}\n",
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name,
            workspace_b.path.to_string_lossy(),
            workspace_b.name,
            workspace_b.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name, workspace_b.name];
        let change_ids = vec!["change-a".to_string(), "change-b".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();

        let merged_contents = std::fs::read_to_string(repo_root.join("conflict.txt")).unwrap();
        assert!(merged_contents.contains('B'));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_merge_retries_after_pre_commit_changes() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        let base_dir = repo_root.join("worktrees");

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("hooked.txt"), "base").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        let config = OrchestratorConfig {
            resolve_command: Some("sh merge-resolver.sh".to_string()),
            ..Default::default()
        };
        let mut manager =
            GitWorkspaceManager::new(base_dir.clone(), repo_root.to_path_buf(), 1, config.clone());

        let workspace_a = manager.create_workspace("change-a", None).await.unwrap();

        std::fs::write(repo_root.join("main.txt"), "main").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Main update"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(workspace_a.path.join("change-a.txt"), "A").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Apply: change-a"])
            .current_dir(&workspace_a.path)
            .output()
            .await
            .unwrap();

        let hooks_dir = repo_root.join(".git/hooks");
        let hook_path = hooks_dir.join("pre-commit");
        let hook_contents = "#!/bin/sh\n\
        set -e\n\
        COMMON_DIR=$(git rev-parse --git-common-dir)\n\
        MARKER=\"$COMMON_DIR/hooks/pre-commit-ran\"\n\
        if [ ! -f \"$MARKER\" ]; then\n\
          echo 'hooked' >> hooked.txt\n\
          git add hooked.txt\n\
          touch \"$MARKER\"\n\
          exit 1\n\
        fi\n\
        exit 0\n";
        std::fs::write(&hook_path, hook_contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&hook_path, perms).unwrap();
        }

        let resolver_script = repo_root.join("merge-resolver.sh");
        let script_contents = format!(
            "#!/bin/sh\nset -e\nROOT=\"$(pwd)\"\n\
            cd \"{}\"\n\
            git checkout {}\n\
            git merge --no-ff --no-commit main\n\
            if ! git commit -m 'Pre-sync base into change-a'; then\n\
              git add -A\n\
              git commit -m 'Pre-sync base into change-a'\n\
            fi\n\
            cd \"$ROOT\"\n\
            git checkout main\n\
            git merge --no-ff --no-commit {}\n\
            if ! git commit -m 'Merge change: change-a'; then\n\
              git add -A\n\
              git commit -m 'Merge change: change-a'\n\
            fi\n",
            workspace_a.path.to_string_lossy(),
            workspace_a.name,
            workspace_a.name
        );
        std::fs::write(&resolver_script, script_contents).unwrap();

        // Create test AI runner

        let shared_stagger_state = Arc::new(Mutex::new(None));

        let queue_config = CommandQueueConfig {
            stagger_delay_ms: DEFAULT_STAGGER_DELAY_MS,

            max_retries: DEFAULT_MAX_RETRIES,

            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,

            retry_error_patterns: default_retry_patterns(),

            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        };

        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        let executor = ParallelExecutor {
            workspace_manager: Box::new(manager),
            config,
            apply_command: String::new(),
            archive_command: String::new(),
            event_tx: None,
            max_conflict_retries: 2,
            repo_root: repo_root.to_path_buf(),
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            merge_deferred_changes: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at: Arc::new(Mutex::new(None)),
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
        };

        let revisions = vec![workspace_a.name];
        let change_ids = vec!["change-a".to_string()];

        executor
            .merge_and_resolve_with(
                &revisions,
                &change_ids,
                |_revs, _details| async move { Ok(()) },
            )
            .await
            .unwrap();

        let hook_contents = std::fs::read_to_string(repo_root.join("hooked.txt")).unwrap();
        assert!(hook_contents.contains("hooked"));
    }

    #[tokio::test]
    async fn test_dynamic_queue_injection() {
        use crate::tui::queue::DynamicQueue;
        use std::sync::Arc;
        use tokio::sync::mpsc;

        // Create a dynamic queue and add a change ID
        let queue = Arc::new(DynamicQueue::new());
        queue.push("test-change-2".to_string()).await;

        // Verify the queue has one item
        assert_eq!(queue.len().await, 1);

        // Create a simple parallel executor with the queue
        let config = OrchestratorConfig::default();
        let repo_root = PathBuf::from("/tmp/test-repo");
        let (tx, _rx) = mpsc::channel(10);
        let mut executor = ParallelExecutor::new(repo_root, config, Some(tx));
        executor.set_dynamic_queue(queue.clone());

        // The queue reference should be set
        assert!(executor.dynamic_queue.is_some());

        // After this point, the execute_with_reanalysis method would poll the queue
        // and inject the change into the execution. This is tested via integration tests.
    }

    #[tokio::test]
    async fn test_debounce_with_queue_changes() {
        use std::time::{Duration, Instant};
        use tokio::sync::mpsc;

        let config = OrchestratorConfig::default();
        let repo_root = PathBuf::from("/tmp/test-repo");
        let (tx, _rx) = mpsc::channel(10);
        let executor = ParallelExecutor::new(repo_root, config, Some(tx));

        // First check: no queue changes, should reanalyze
        assert!(executor.should_reanalyze(true).await);

        // Simulate a queue change
        {
            let mut last_change = executor.last_queue_change_at.lock().await;
            *last_change = Some(Instant::now());
        }

        // Immediate check: should NOT reanalyze (debounce active)
        assert!(!executor.should_reanalyze(true).await);

        // Wait for debounce period to expire (10 seconds + margin)
        tokio::time::sleep(Duration::from_secs(11)).await;

        // After debounce: should reanalyze
        assert!(executor.should_reanalyze(true).await);
    }
}
