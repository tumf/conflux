use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::error_history::{CircuitBreakerConfig, ErrorHistory};
use crate::events::ExecutionEvent;
use crate::execution::apply::{check_task_progress, create_progress_commit, is_progress_complete};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::orchestration::state::OrchestratorState;
use crate::orchestration::{
    acceptance_test_streaming, apply_change, archive_change, update_tasks_on_acceptance_failure,
    AcceptanceResult, ApplyContext, ApplyResult, ArchiveContext, ArchiveResult, LogOutputHandler,
};
use crate::parallel_run_service::ParallelRunService;
use crate::progress::ProgressDisplay;
use crate::serial_run_service::SerialRunService;
use crate::stall::{StallDetector, StallPhase};
use crate::task_parser::TaskProgress;
use crate::tui::log_deduplicator;
use crate::vcs::git::commands as git_commands;
use crate::vcs::{GitWorkspaceManager, VcsBackend, WorkspaceManager};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{error, info, warn};

#[cfg(feature = "web-monitoring")]
use crate::web::WebState;
#[cfg(feature = "web-monitoring")]
use tokio::sync::mpsc;

struct SerialSnapshot {
    progress: crate::task_parser::TaskProgress,
    empty_commit: Option<bool>,
}
#[cfg(feature = "web-monitoring")]
use std::sync::Arc;

pub struct Orchestrator {
    agent: AgentRunner,
    config: OrchestratorConfig,
    progress: Option<ProgressDisplay>,
    /// Target changes specified by --change option (comma-separated)
    target_changes: Option<Vec<String>>,
    /// Snapshot of change IDs captured at run start.
    /// Only changes present in this snapshot will be processed during the run.
    /// This prevents mid-run proposals from being processed before they are ready.
    initial_change_ids: Option<HashSet<String>>,
    /// Hook runner for executing hooks at various stages
    hooks: HookRunner,
    /// Current change ID being processed (for on_change_start/on_change_end detection)
    current_change_id: Option<String>,
    /// Completed change IDs (after archive, on_change_end called)
    completed_change_ids: HashSet<String>,
    /// Apply counts per change (how many times each change has been applied)
    apply_counts: HashMap<String, u32>,
    /// Stall detector for empty WIP commit tracking
    stall_detector: StallDetector,
    /// Changes marked as stalled (failed due to no progress)
    stalled_change_ids: HashSet<String>,
    /// Changes skipped due to stalled dependencies
    skipped_change_ids: HashSet<String>,
    /// Error history per change (for circuit breaker pattern)
    error_histories: HashMap<String, ErrorHistory>,
    /// Number of changes processed (archived)
    changes_processed: usize,
    /// Maximum iterations limit (0 = no limit)
    max_iterations: u32,
    /// Current iteration number (for max_iterations check)
    iteration: u32,
    /// Enable parallel execution mode
    parallel: bool,
    /// Maximum concurrent workspaces for parallel execution
    max_concurrent: Option<usize>,
    /// Dry run mode (preview without execution)
    dry_run: bool,
    /// VCS backend for parallel execution
    #[allow(dead_code)] // Will be passed to ParallelRunService in future
    vcs_backend: VcsBackend,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
    /// Shared orchestration state (single source of truth for state tracking)
    /// Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
    shared_state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
    /// Web monitoring state (for broadcasting updates to WebSocket clients)
    #[cfg(feature = "web-monitoring")]
    web_state: Option<Arc<WebState>>,
}

impl Orchestrator {
    /// Create a new orchestrator with optional custom config path and max iterations override
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_changes: Option<Vec<String>>,
        config_path: Option<PathBuf>,
        max_iterations_override: Option<u32>,
        parallel: bool,
        max_concurrent: Option<usize>,
        dry_run: bool,
        vcs_override: Option<VcsBackend>,
        no_resume: bool,
    ) -> Result<Self> {
        let config = OrchestratorConfig::load(config_path.as_deref())?;
        log_deduplicator::configure_logging(config.get_logging());
        let hooks = HookRunner::new(config.get_hooks());
        // CLI override takes precedence over config file value
        let max_iterations = max_iterations_override.unwrap_or_else(|| config.get_max_iterations());
        let agent = AgentRunner::new(config.clone());
        // VCS backend: CLI override takes precedence, then config, then auto
        let vcs_backend = vcs_override.unwrap_or_else(|| config.get_vcs_backend());
        let stall_detector = StallDetector::new(config.get_stall_detection());

        // Initialize shared state (will be populated when run() is called with actual changes)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            agent,
            config,
            progress: None,
            target_changes,
            initial_change_ids: None,
            hooks,
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            apply_counts: HashMap::new(),
            stall_detector,
            stalled_change_ids: HashSet::new(),
            skipped_change_ids: HashSet::new(),
            error_histories: HashMap::new(),
            changes_processed: 0,
            max_iterations,
            iteration: 0,
            parallel,
            max_concurrent,
            dry_run,
            vcs_backend,
            no_resume,
            shared_state,
            #[cfg(feature = "web-monitoring")]
            web_state: None,
        })
    }

    /// Set web monitoring state for broadcasting updates to WebSocket clients.
    /// Also injects the shared orchestration state reference into WebState for unified tracking.
    #[cfg(feature = "web-monitoring")]
    pub async fn set_web_state(&mut self, web_state: Arc<WebState>) {
        // Inject shared state reference into WebState
        web_state.set_shared_state(self.shared_state.clone()).await;
        self.web_state = Some(web_state);
    }

    /// Broadcast state update to web monitoring clients
    #[cfg(feature = "web-monitoring")]
    async fn broadcast_state_update(&self, changes: &[Change]) {
        if let Some(ref web_state) = self.web_state {
            web_state.update(changes).await;
        }
    }

    /// No-op when web monitoring is disabled
    #[cfg(not(feature = "web-monitoring"))]
    async fn broadcast_state_update(&self, _changes: &[Change]) {}

    /// Create a new orchestrator with explicit configuration (for testing)
    #[cfg(test)]
    pub fn with_config(
        target_changes: Option<Vec<String>>,
        config: OrchestratorConfig,
    ) -> Result<Self> {
        log_deduplicator::configure_logging(config.get_logging());
        let hooks = HookRunner::new(config.get_hooks());
        let max_iterations = config.get_max_iterations();
        let agent = AgentRunner::new(config.clone());
        let stall_detector = StallDetector::new(config.get_stall_detection());

        // Initialize shared state (for testing, will use empty change list)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            agent,
            config,
            progress: None,
            target_changes,
            initial_change_ids: None,
            hooks,
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            apply_counts: HashMap::new(),
            stall_detector,
            stalled_change_ids: HashSet::new(),
            skipped_change_ids: HashSet::new(),
            error_histories: HashMap::new(),
            changes_processed: 0,
            max_iterations,
            iteration: 0,
            parallel: false,
            max_concurrent: None,
            dry_run: false,
            vcs_backend: VcsBackend::Auto,
            no_resume: false,
            shared_state,
            #[cfg(feature = "web-monitoring")]
            web_state: None,
        })
    }

    /// Run the orchestration loop with cancellation support
    pub async fn run(&mut self, _cancel_token: tokio_util::sync::CancellationToken) -> Result<()> {
        info!("Starting orchestration loop");
        // TODO: Integrate cancel_token with apply/archive operations for graceful shutdown

        // Capture initial snapshot of change IDs at run start.
        // Only changes present at this point will be processed during the run.
        // This prevents mid-run proposals from being processed before they are ready.
        let initial_changes = openspec::list_changes_native()?;

        // Handle parallel mode with dry_run
        if self.parallel && self.dry_run {
            return self.run_parallel_dry_run(&initial_changes).await;
        }

        // Handle parallel execution mode
        if self.parallel {
            return self.run_parallel(&initial_changes).await;
        }

        if initial_changes.is_empty() {
            info!("No changes found");
            return Ok(());
        }

        // Filter by target_changes if specified (early filtering)
        // Both explicit targets and default mode require approval check
        let filtered_initial = if let Some(targets) = &self.target_changes {
            // Explicit targets specified via --change option
            let mut found = Vec::new();
            for target in targets {
                let trimmed = target.trim();
                if let Some(change) = initial_changes.iter().find(|c| c.id == trimmed) {
                    // Check approval status even for explicitly specified changes
                    if change.is_approved {
                        found.push(change.clone());
                    } else {
                        warn!(
                            "Skipping unapproved change '{}'. Approve it first with: cflx approve set {}",
                            trimmed, trimmed
                        );
                    }
                } else {
                    warn!("Specified change '{}' not found, skipping", trimmed);
                }
            }
            found
        } else {
            // No explicit target: filter to only approved changes
            let (approved, unapproved): (Vec<_>, Vec<_>) =
                initial_changes.into_iter().partition(|c| c.is_approved);

            // Warn about unapproved changes
            for change in &unapproved {
                warn!(
                    "Skipping unapproved change '{}'. Approve it first with: cflx approve set {}",
                    change.id, change.id
                );
            }

            if approved.is_empty() && !unapproved.is_empty() {
                info!(
                    "No approved changes found. {} change(s) are pending approval.",
                    unapproved.len()
                );
                return Ok(());
            }

            approved
        };

        if filtered_initial.is_empty() {
            info!("No changes found matching specified targets");
            return Ok(());
        }

        // Store snapshot of change IDs (only the filtered ones)
        let snapshot_ids: HashSet<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        info!(
            "Captured snapshot of {} changes: {:?}",
            snapshot_ids.len(),
            snapshot_ids
        );
        self.initial_change_ids = Some(snapshot_ids.clone());

        // Initialize shared orchestration state with filtered changes
        let change_ids: Vec<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        *self.shared_state.write().await = OrchestratorState::new(change_ids, self.max_iterations);

        // Initialize progress display
        self.progress = Some(ProgressDisplay::new(filtered_initial.len()));

        let total_changes = filtered_initial.len();

        // Create serial run service for shared state and helpers
        let repo_root = std::env::current_dir()?;
        let mut serial_service = SerialRunService::new(repo_root, self.config.clone());

        // Run on_start hook
        let start_context = HookContext::new(0, total_changes, total_changes, false);
        self.hooks
            .run_hook(HookType::OnStart, &start_context)
            .await?;

        let finish_status;

        loop {
            // Increment iteration counter
            self.iteration += 1;

            // Check max iterations limit (0 = no limit)
            if self.max_iterations > 0 {
                // Log warning when approaching limit (80%)
                let warning_threshold = (self.max_iterations as f32 * 0.8) as u32;
                if self.iteration == warning_threshold {
                    warn!(
                        "Approaching max iterations: {}/{}",
                        self.iteration, self.max_iterations
                    );
                }

                // Stop if max iterations reached
                if self.iteration > self.max_iterations {
                    info!(
                        "Max iterations ({}) reached, stopping orchestration",
                        self.max_iterations
                    );
                    if let Some(progress) = &mut self.progress {
                        progress.complete_all();
                    }
                    finish_status = "iteration_limit";
                    break;
                }
            }

            // List all changes from openspec (to get updated progress)
            let changes = openspec::list_changes_native()?;

            // Broadcast state update to web monitoring clients
            self.broadcast_state_update(&changes).await;

            // Filter to only include changes from initial snapshot
            let snapshot_changes = self.filter_to_snapshot(&changes);

            // Log any new changes that appeared after run started
            self.log_new_changes(&changes);

            if snapshot_changes.is_empty() {
                info!("All changes from initial snapshot processed");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                finish_status = "completed";
                break;
            }

            let eligible_changes = self.filter_stalled_changes(&snapshot_changes);
            let remaining_changes = eligible_changes.len();

            if eligible_changes.is_empty() {
                info!("All remaining changes are blocked by stalled dependencies");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                finish_status = "stalled_dependencies";
                break;
            }

            // Select next change to process using serial service
            let next = serial_service
                .select_next_change(&eligible_changes)
                .ok_or_else(|| {
                    OrchestratorError::AgentCommand("No eligible change found".to_string())
                })?;
            info!("Selected change: {}", next.id);

            if let Some(progress) = &mut self.progress {
                progress.update_change(next);
            }

            // Check if this is a new change (for on_change_start hook)
            let is_new_change = self.current_change_id.as_ref() != Some(&next.id);
            if is_new_change {
                // Update shared state: processing started
                self.shared_state
                    .write()
                    .await
                    .apply_execution_event(&ExecutionEvent::ProcessingStarted(next.id.clone()));

                // Run on_change_start hook
                let change_start_context = HookContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    false,
                )
                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                .with_apply_count(0);
                self.hooks
                    .run_hook(HookType::OnChangeStart, &change_start_context)
                    .await?;
                self.current_change_id = Some(next.id.clone());
            }

            // Get current apply count for this change
            let apply_count = *self.apply_counts.get(&next.id).unwrap_or(&0);

            // Process the change
            if next.is_complete() {
                // Archive completed change using shared function
                info!("Change {} is complete, archiving...", next.id);

                let archive_ctx = ArchiveContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    apply_count,
                );
                let output = LogOutputHandler::new();

                let stall_config = self.config.get_stall_detection();

                match archive_change(
                    next,
                    &mut self.agent,
                    &self.hooks,
                    &archive_ctx,
                    &output,
                    None,
                    &stall_config,
                )
                .await
                {
                    Ok(ArchiveResult::Success) => {
                        // Update changes_processed count
                        self.changes_processed += 1;
                        let new_remaining = remaining_changes - 1;

                        // Update shared state: mark change as archived
                        self.shared_state.write().await.apply_execution_event(
                            &ExecutionEvent::ChangeArchived(next.id.clone()),
                        );

                        // Clear acceptance history after successful archive
                        self.agent.clear_acceptance_history(&next.id);

                        // Run on_change_end hook (not included in shared archive_change)
                        let change_end_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            new_remaining,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(apply_count);
                        self.hooks
                            .run_hook(HookType::OnChangeEnd, &change_end_context)
                            .await?;

                        // Mark change as completed and clear current
                        self.completed_change_ids.insert(next.id.clone());
                        self.current_change_id = None;
                        self.apply_counts.remove(&next.id);
                        self.stall_detector.clear_change(&next.id);

                        if let Some(progress) = &mut self.progress {
                            progress.archive_change(&next.id);
                        }
                    }
                    Ok(ArchiveResult::Stalled { error }) => {
                        warn!("Archive stalled for {}: {}", next.id, error);
                        self.mark_change_stalled(&next.id, &error);
                        serial_service.mark_stalled(&next.id, &error);
                        continue;
                    }
                    Ok(ArchiveResult::Failed { error }) => {
                        error!("Archive failed for {}: {}", next.id, error);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Archive failed: {}", next.id));
                        }
                        return Err(OrchestratorError::AgentCommand(error));
                    }
                    Ok(ArchiveResult::Cancelled) => {
                        info!("Archive cancelled for {}", next.id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Archive error for {}: {}", next.id, e);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Archive failed: {}", next.id));
                        }
                        return Err(e);
                    }
                }
            } else {
                // Apply change using shared function
                info!("Applying change: {}", next.id);

                // Update shared state: apply started
                self.shared_state.write().await.apply_execution_event(
                    &ExecutionEvent::ApplyStarted {
                        change_id: next.id.clone(),
                    },
                );

                // Increment apply count
                let new_apply_count = apply_count + 1;
                self.apply_counts.insert(next.id.clone(), new_apply_count);

                let apply_ctx = ApplyContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    new_apply_count,
                );
                let output = LogOutputHandler::new();

                match apply_change(next, &mut self.agent, &self.hooks, &apply_ctx, &output).await {
                    Ok(ApplyResult::Success) => {
                        // Update shared state: apply completed
                        self.shared_state.write().await.apply_execution_event(
                            &ExecutionEvent::ApplyCompleted {
                                change_id: next.id.clone(),
                                revision: "serial".to_string(), // Serial mode doesn't use git revisions
                            },
                        );

                        let snapshot = match self
                            .snapshot_serial_iteration(&next.id, new_apply_count)
                            .await
                        {
                            Ok(snapshot) => snapshot,
                            Err(e) => {
                                warn!("Failed to snapshot WIP commit for {}: {}", next.id, e);
                                SerialSnapshot {
                                    progress: crate::task_parser::TaskProgress::default(),
                                    empty_commit: None,
                                }
                            }
                        };

                        let progress_snapshot = snapshot.progress;

                        if let Some(is_empty) = snapshot.empty_commit {
                            if !is_progress_complete(&progress_snapshot)
                                && self.stall_detector.register_commit(
                                    &next.id,
                                    StallPhase::Apply,
                                    is_empty,
                                )
                            {
                                let count = self
                                    .stall_detector
                                    .current_count(&next.id, StallPhase::Apply);
                                let threshold = self.stall_detector.config().threshold;
                                let message = format!(
                                    "Stall detected for {} after {} empty WIP commits (apply)",
                                    next.id, count
                                );
                                warn!("{} (threshold {})", message, threshold);
                                self.mark_change_stalled(&next.id, &message);
                                serial_service.mark_stalled(&next.id, &message);
                                continue;
                            }
                        }

                        if is_progress_complete(&progress_snapshot) {
                            let _ = self
                                .squash_serial_wip_commits(&next.id, new_apply_count)
                                .await;

                            // Run acceptance test after apply completion
                            info!("Tasks complete for {}, running acceptance test...", next.id);
                            let output = LogOutputHandler::new();
                            let cancel_check = || false; // No cancellation in CLI mode

                            match acceptance_test_streaming(
                                next,
                                &mut self.agent,
                                &output,
                                cancel_check,
                            )
                            .await
                            {
                                Ok(AcceptanceResult::Pass) => {
                                    info!("Acceptance passed for {}, ready for archive", next.id);
                                    // Change will be archived in next iteration
                                }
                                Ok(AcceptanceResult::Continue) => {
                                    let continue_count =
                                        self.agent.count_consecutive_acceptance_continues(&next.id);
                                    let max_continues = self.config.get_acceptance_max_continues();

                                    if continue_count >= max_continues {
                                        warn!(
                                            "Acceptance CONTINUE limit ({}) exceeded for {}, treating as FAIL",
                                            max_continues, next.id
                                        );
                                        // Exceeded limit - treat as FAIL and return to apply loop
                                    } else {
                                        info!(
                                            "Acceptance requires continuation for {} (attempt {}/{}), retrying...",
                                            next.id,
                                            continue_count,
                                            max_continues
                                        );
                                        // Will retry acceptance in next iteration
                                    }
                                }
                                Ok(AcceptanceResult::Fail { findings }) => {
                                    warn!(
                                        "Acceptance failed for {} with {} findings, will retry apply",
                                        next.id,
                                        findings.len()
                                    );
                                    // Update tasks.md with acceptance findings
                                    if let Err(e) = update_tasks_on_acceptance_failure(
                                        &next.id, &findings, None,
                                    )
                                    .await
                                    {
                                        warn!("Failed to update tasks.md for {}: {}", next.id, e);
                                    }
                                    // Change will be selected again for apply in next iteration
                                }
                                Ok(AcceptanceResult::CommandFailed { error, findings }) => {
                                    error!("Acceptance command failed for {}: {}", next.id, error);
                                    // Update tasks.md with command failure
                                    if let Err(e) = update_tasks_on_acceptance_failure(
                                        &next.id, &findings, None,
                                    )
                                    .await
                                    {
                                        warn!("Failed to update tasks.md for {}: {}", next.id, e);
                                    }
                                    // Change will be selected again for apply in next iteration
                                }
                                Ok(AcceptanceResult::Cancelled) => {
                                    info!("Acceptance cancelled for {}", next.id);
                                    return Ok(());
                                }
                                Err(e) => {
                                    error!("Acceptance error for {}: {}", next.id, e);
                                    return Err(e);
                                }
                            }
                        }

                        if let Some(progress) = &mut self.progress {
                            progress.complete_change(&next.id);
                        }
                    }
                    Ok(ApplyResult::Failed { error }) => {
                        if let Err(e) = self
                            .snapshot_serial_iteration(&next.id, new_apply_count)
                            .await
                        {
                            warn!("Failed to snapshot WIP commit for {}: {}", next.id, e);
                        }

                        // Record error and check circuit breaker
                        if self.record_error_and_check_circuit_breaker(&next.id, &error) {
                            let message = format!(
                                "Circuit breaker opened for '{}' due to repeated errors",
                                next.id
                            );
                            warn!("{}", message);
                            self.mark_change_stalled(&next.id, &message);
                            serial_service.mark_stalled(&next.id, &message);
                            continue;
                        }

                        error!("Apply failed for {}: {}", next.id, error);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Apply failed: {}", next.id));
                        }
                        return Err(OrchestratorError::AgentCommand(error));
                    }
                    Ok(ApplyResult::Cancelled) => {
                        info!("Apply cancelled for {}", next.id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Apply error for {}: {}", next.id, e);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Apply failed: {}", next.id));
                        }
                        return Err(e);
                    }
                }
            }
        }

        // Run on_finish hook
        let finish_context = HookContext::new(self.changes_processed, total_changes, 0, false)
            .with_status(finish_status);
        self.hooks
            .run_hook(HookType::OnFinish, &finish_context)
            .await?;

        info!("Orchestration completed");
        Ok(())
    }

    async fn snapshot_serial_iteration(
        &self,
        change_id: &str,
        iteration: u32,
    ) -> Result<SerialSnapshot> {
        let repo_root = std::env::current_dir()?;
        let progress =
            check_task_progress(&repo_root, change_id).unwrap_or_else(|_| TaskProgress::default());
        let mut empty_commit = None;

        if matches!(self.vcs_backend, VcsBackend::Git | VcsBackend::Auto) {
            let is_git_repo = match git_commands::check_git_repo(&repo_root).await {
                Ok(is_repo) => is_repo,
                Err(e) => {
                    warn!("Failed to check Git repository status: {}", e);
                    false
                }
            };

            if is_git_repo {
                let workspace_manager = GitWorkspaceManager::new(
                    repo_root.join(".openspec-worktrees"),
                    repo_root.clone(),
                    1,
                    self.config.clone(),
                );

                if let Err(e) = create_progress_commit(
                    &workspace_manager,
                    &repo_root,
                    change_id,
                    &progress,
                    iteration,
                )
                .await
                {
                    warn!(
                        "Failed to create WIP commit for {} (apply#{}): {}",
                        change_id, iteration, e
                    );
                } else {
                    match git_commands::is_head_empty_commit(&repo_root).await {
                        Ok(is_empty) => empty_commit = Some(is_empty),
                        Err(e) => {
                            warn!(
                                "Failed to check WIP commit contents for {} (apply#{}): {}",
                                change_id, iteration, e
                            );
                        }
                    }
                }
            }
        }

        Ok(SerialSnapshot {
            progress,
            empty_commit,
        })
    }

    async fn squash_serial_wip_commits(&self, change_id: &str, iteration: u32) -> Result<()> {
        if !matches!(self.vcs_backend, VcsBackend::Git | VcsBackend::Auto) {
            return Ok(());
        }

        let repo_root = std::env::current_dir()?;
        let is_git_repo = match git_commands::check_git_repo(&repo_root).await {
            Ok(is_repo) => is_repo,
            Err(e) => {
                warn!("Failed to check Git repository status: {}", e);
                false
            }
        };

        if !is_git_repo {
            return Ok(());
        }

        let workspace_manager = GitWorkspaceManager::new(
            repo_root.join(".openspec-worktrees"),
            repo_root.clone(),
            1,
            self.config.clone(),
        );

        if let Err(e) = workspace_manager
            .squash_wip_commits(&repo_root, change_id, iteration)
            .await
        {
            warn!(
                "Failed to squash WIP commits for {} (apply#{}): {}",
                change_id, iteration, e
            );
        }

        Ok(())
    }

    /// Select the next change to process.
    ///
    /// Uses the shared selection module which provides:
    /// Filter changes to only include those present in the initial snapshot.
    /// Returns an empty vector if no snapshot was captured.
    fn filter_to_snapshot(&self, changes: &[Change]) -> Vec<Change> {
        match &self.initial_change_ids {
            Some(snapshot) => changes
                .iter()
                .filter(|c| snapshot.contains(&c.id))
                .cloned()
                .collect(),
            None => changes.to_vec(),
        }
    }

    /// Log any changes that were not present in the initial snapshot.
    /// These are new changes added after the run started and will be ignored.
    fn log_new_changes(&self, changes: &[Change]) {
        if let Some(snapshot) = &self.initial_change_ids {
            for change in changes {
                if !snapshot.contains(&change.id) {
                    warn!(
                        "New change '{}' detected after run started - will be ignored",
                        change.id
                    );
                }
            }
        }
    }

    /// Filter out stalled changes and those blocked by stalled dependencies.
    fn filter_stalled_changes(&mut self, changes: &[Change]) -> Vec<Change> {
        let mut eligible = Vec::new();

        for change in changes {
            if self.stalled_change_ids.contains(&change.id) {
                continue;
            }

            if let Some(failed_dep) = change
                .dependencies
                .iter()
                .find(|dep| self.stalled_change_ids.contains(*dep))
            {
                if self.skipped_change_ids.insert(change.id.clone()) {
                    warn!(
                        "Skipping '{}' because dependency '{}' stalled",
                        change.id, failed_dep
                    );
                }
                continue;
            }

            eligible.push(change.clone());
        }

        eligible
    }

    fn mark_change_stalled(&mut self, change_id: &str, reason: &str) {
        self.stalled_change_ids.insert(change_id.to_string());
        self.apply_counts.remove(change_id);
        self.error_histories.remove(change_id);
        self.current_change_id = None;
        self.stall_detector.clear_change(change_id);

        if let Some(progress) = &mut self.progress {
            progress.error(reason);
        }
    }

    /// Record an error and check if circuit breaker should trip
    /// Returns true if the change should be skipped due to repeated errors
    fn record_error_and_check_circuit_breaker(&mut self, change_id: &str, error: &str) -> bool {
        let cb_config = self.config.get_error_circuit_breaker();
        let circuit_breaker_config = CircuitBreakerConfig {
            enabled: cb_config.enabled,
            threshold: cb_config.threshold,
        };

        let history = self
            .error_histories
            .entry(change_id.to_string())
            .or_insert_with(|| ErrorHistory::new(circuit_breaker_config.clone()));

        history.record_error(error);

        if history.detect_same_error() {
            error!(
                "Circuit breaker triggered for '{}': same error occurred {} times consecutively",
                change_id, circuit_breaker_config.threshold
            );
            if let Some(last_err) = history.last_error() {
                error!("Last error pattern: {}", last_err);
            }
            true
        } else {
            false
        }
    }

    /// Set initial change IDs snapshot directly (for testing purposes)
    #[cfg(test)]
    pub fn set_initial_change_ids(&mut self, ids: HashSet<String>) {
        self.initial_change_ids = Some(ids);
    }

    /// Run parallel mode with dry run (preview parallelization groups)
    async fn run_parallel_dry_run(&self, changes: &[Change]) -> Result<()> {
        info!("Running parallel mode dry run (preview only)");

        // Filter to approved changes only
        let approved: Vec<_> = changes.iter().filter(|c| c.is_approved).cloned().collect();

        if approved.is_empty() {
            println!("No approved changes found for parallel execution.");
            for change in changes {
                println!(
                    "  - {} (unapproved) - use: cflx approve set {}",
                    change.id, change.id
                );
            }
            return Ok(());
        }

        // Use ParallelRunService to analyze groups (uses LLM if enabled)
        let repo_root = std::env::current_dir()?;
        let service = ParallelRunService::new(repo_root, self.config.clone());
        let groups = service.analyze_and_group_public(&approved).await;

        // Display parallelization groups
        println!("\n=== Parallel Execution Plan (Dry Run) ===\n");
        println!("Total changes: {}", approved.len());
        println!("Parallelization groups: {}\n", groups.len());

        for group in &groups {
            println!("Group {} (can run in parallel):", group.id);
            for change_id in &group.changes {
                let change = approved.iter().find(|c| c.id == *change_id);
                if let Some(c) = change {
                    println!(
                        "  - {} ({}/{} tasks, {:.1}%)",
                        c.id,
                        c.completed_tasks,
                        c.total_tasks,
                        c.progress_percent()
                    );
                } else {
                    println!("  - {}", change_id);
                }
            }
            if !group.depends_on.is_empty() {
                println!("  (depends on group(s): {:?})", group.depends_on);
            }
            println!();
        }

        println!(
            "Max concurrent workspaces: {}",
            self.max_concurrent.unwrap_or(4)
        );
        println!("\nTo execute, run without --dry-run flag.");

        Ok(())
    }

    /// Run parallel execution mode
    async fn run_parallel(&mut self, changes: &[Change]) -> Result<()> {
        info!("Running parallel execution mode");

        // Filter to approved changes only
        let approved: Vec<_> = changes.iter().filter(|c| c.is_approved).cloned().collect();

        if approved.is_empty() {
            info!("No approved changes found for parallel execution");
            return Ok(());
        }

        // Store snapshot of change IDs
        let snapshot_ids: HashSet<String> = approved.iter().map(|c| c.id.clone()).collect();
        self.initial_change_ids = Some(snapshot_ids);

        // Use ParallelRunService for the common parallel execution flow
        let repo_root = std::env::current_dir()?;
        let mut service = ParallelRunService::new(repo_root.clone(), self.config.clone());
        service.set_no_resume(self.no_resume);

        // Check if Git is available for true parallel execution
        if !service.check_vcs_available().await? {
            return Err(OrchestratorError::GitCommand(
                "Git repository not available for parallel execution".to_string(),
            ));
        }

        info!("Git available, executing changes in parallel using worktrees");

        #[cfg(feature = "web-monitoring")]
        let (web_event_tx, web_event_handle) = if let Some(web_state) = self.web_state.clone() {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let handle = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    web_state.apply_execution_event(&event).await;
                    if matches!(
                        event,
                        crate::events::ExecutionEvent::AllCompleted
                            | crate::events::ExecutionEvent::Stopped
                    ) {
                        break;
                    }
                }
            });
            (Some(tx), Some(handle))
        } else {
            (None, None)
        };

        #[cfg(feature = "web-monitoring")]
        let web_event_sender = web_event_tx.clone();

        // Run with a simple logging event handler for CLI mode
        let result = service
            .run_parallel(approved, move |event| {
                // Log events for CLI mode (no TUI)
                use crate::parallel::ParallelEvent;
                #[cfg(feature = "web-monitoring")]
                if let Some(tx) = &web_event_sender {
                    let _ = tx.send(event.clone());
                }
                match event {
                    ParallelEvent::ApplyCompleted { change_id, .. } => {
                        info!("Apply completed for {}", change_id);
                    }
                    ParallelEvent::ApplyFailed { change_id, error } => {
                        error!("Apply failed for {}: {}", change_id, error);
                    }
                    ParallelEvent::ChangeArchived(change_id) => {
                        info!("Archived {}", change_id);
                    }
                    ParallelEvent::AllCompleted => {
                        info!("All parallel execution completed");
                    }
                    ParallelEvent::Error { message } => {
                        error!("Parallel execution error: {}", message);
                    }
                    ParallelEvent::Warning { message, .. } => {
                        eprintln!("{}", message);
                    }
                    _ => {}
                }
            })
            .await;

        #[cfg(feature = "web-monitoring")]
        if let Some(handle) = web_event_handle {
            drop(web_event_tx);
            let _ = handle.await;
        }

        result?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            is_approved: false,
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_filter_to_snapshot_filters_new_changes() {
        // Create orchestrator with mock config (won't be used in this test)
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

        // Set up snapshot with only change-a and change-b
        let snapshot: HashSet<String> = ["change-a", "change-b"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Create changes list including new change-c
        let all_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
            create_test_change("change-c", 0, 3), // New change, not in snapshot
        ];

        // Filter to snapshot
        let filtered = orchestrator.filter_to_snapshot(&all_changes);

        // Should only include change-a and change-b
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|c| c.id == "change-a"));
        assert!(filtered.iter().any(|c| c.id == "change-b"));
        assert!(!filtered.iter().any(|c| c.id == "change-c"));
    }

    #[test]
    fn test_filter_to_snapshot_returns_all_when_no_snapshot() {
        // Create orchestrator without setting snapshot
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        let all_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
        ];

        // Should return all changes when no snapshot is set
        let filtered = orchestrator.filter_to_snapshot(&all_changes);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_to_snapshot_removes_archived_changes() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

        // Set up snapshot with change-a, change-b, change-c
        let snapshot: HashSet<String> = ["change-a", "change-b", "change-c"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Simulate change-b being archived (no longer in list)
        let current_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-c", 1, 5),
        ];

        // Filter should only return change-a and change-c (both in snapshot and in current list)
        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|c| c.id == "change-a"));
        assert!(filtered.iter().any(|c| c.id == "change-c"));
    }

    #[test]
    fn test_filter_to_snapshot_handles_empty_changes() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

        let snapshot: HashSet<String> = ["change-a"].iter().map(|s| s.to_string()).collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Empty changes list
        let current_changes: Vec<Change> = vec![];

        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_snapshot_preserves_updated_progress() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

        // Set up snapshot with change-a
        let snapshot: HashSet<String> = ["change-a"].iter().map(|s| s.to_string()).collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Create changes with updated progress for change-a
        let current_changes = vec![
            create_test_change("change-a", 4, 5), // Progress updated from 2/5 to 4/5
        ];

        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].completed_tasks, 4); // Progress should be updated
    }

    #[test]
    fn test_filter_stalled_changes_skips_dependencies() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();
        orchestrator
            .stalled_change_ids
            .insert("change-a".to_string());

        let changes = vec![
            Change {
                id: "change-a".to_string(),
                completed_tasks: 0,
                total_tasks: 3,
                last_modified: "now".to_string(),
                is_approved: true,
                dependencies: Vec::new(),
            },
            Change {
                id: "change-b".to_string(),
                completed_tasks: 0,
                total_tasks: 3,
                last_modified: "now".to_string(),
                is_approved: true,
                dependencies: vec!["change-a".to_string()],
            },
            Change {
                id: "change-c".to_string(),
                completed_tasks: 0,
                total_tasks: 3,
                last_modified: "now".to_string(),
                is_approved: true,
                dependencies: Vec::new(),
            },
        ];

        let eligible = orchestrator.filter_stalled_changes(&changes);
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].id, "change-c");
    }

    // Note: build_analysis_prompt tests moved to src/orchestration/selection.rs

    #[test]
    fn test_orchestrator_creation() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        assert!(orchestrator.target_changes.is_none());
        assert!(orchestrator.initial_change_ids.is_none());
        assert!(orchestrator.current_change_id.is_none());
        assert!(orchestrator.completed_change_ids.is_empty());
        assert!(orchestrator.apply_counts.is_empty());
        assert_eq!(orchestrator.changes_processed, 0);
        assert_eq!(orchestrator.iteration, 0);
    }

    #[test]
    fn test_orchestrator_with_single_target_change() {
        let config = OrchestratorConfig::default();
        let orchestrator =
            Orchestrator::with_config(Some(vec!["my-change".to_string()]), config).unwrap();

        assert_eq!(
            orchestrator.target_changes,
            Some(vec!["my-change".to_string()])
        );
    }

    #[test]
    fn test_orchestrator_with_multiple_target_changes() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(
            Some(vec![
                "change-a".to_string(),
                "change-b".to_string(),
                "change-c".to_string(),
            ]),
            config,
        )
        .unwrap();

        assert_eq!(
            orchestrator.target_changes,
            Some(vec![
                "change-a".to_string(),
                "change-b".to_string(),
                "change-c".to_string()
            ])
        );
    }
}
