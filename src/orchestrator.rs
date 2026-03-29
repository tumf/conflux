use crate::agent::AgentRunner;
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::error_history::{CircuitBreakerConfig, ErrorHistory};
use crate::events::ExecutionEvent;
use crate::execution::apply::{check_task_progress, create_progress_commit};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::orchestration::state::OrchestratorState;
use crate::orchestration::LogOutputHandler;
use crate::parallel_run_service::ParallelRunService;
use crate::progress::ProgressDisplay;
use crate::serial_run_service::SerialRunService;
use crate::stall::StallDetector;
use crate::task_parser::TaskProgress;
use crate::tui::log_deduplicator;
use crate::vcs::git::commands as git_commands;
use crate::vcs::{GitWorkspaceManager, VcsBackend, WorkspaceManager};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[cfg(feature = "web-monitoring")]
use crate::web::WebState;
#[cfg(feature = "web-monitoring")]
use tokio::sync::mpsc;

struct SerialSnapshot {
    progress: crate::task_parser::TaskProgress,
    empty_commit: Option<bool>,
}

pub struct Orchestrator {
    agent: AgentRunner,
    ai_runner: AiCommandRunner,
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
    /// Current execution mode for web monitoring app_mode
    /// "select" | "running" | "stopped" | "stopping" | "error"
    #[cfg(feature = "web-monitoring")]
    execution_mode: String,
}

/// Control flow result indicating whether to continue or break the main loop
enum LoopControl {
    Continue,
    Break { finish_status: &'static str },
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
        let repo_root = std::env::current_dir()?;
        let hooks = HookRunner::with_output_handler(
            config.get_hooks(),
            &repo_root,
            Arc::new(LogOutputHandler::new()),
        );
        // CLI override takes precedence over config file value
        let max_iterations = max_iterations_override.unwrap_or_else(|| config.get_max_iterations());
        let agent = AgentRunner::new(config.clone());
        // VCS backend: CLI override takes precedence, then config, then auto
        let vcs_backend = vcs_override.unwrap_or_else(|| config.get_vcs_backend());
        let stall_detector = StallDetector::new(config.get_stall_detection());

        // Create AiCommandRunner for serial mode execution
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
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
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
            strict_process_cleanup: config.get_command_strict_process_cleanup(),
        };
        let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
        ai_runner.set_stream_json_textify(config.get_stream_json_textify());
        ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());

        // Initialize shared state (will be populated when run() is called with actual changes)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            agent,
            ai_runner,
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
            #[cfg(feature = "web-monitoring")]
            execution_mode: "select".to_string(),
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
            web_state
                .update_with_mode(changes, &self.execution_mode)
                .await;
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
        let repo_root = std::env::current_dir()?;
        let hooks = HookRunner::with_output_handler(
            config.get_hooks(),
            &repo_root,
            Arc::new(LogOutputHandler::new()),
        );
        let max_iterations = config.get_max_iterations();
        let agent = AgentRunner::new(config.clone());
        let stall_detector = StallDetector::new(config.get_stall_detection());

        // Create AiCommandRunner for serial mode execution
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
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
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
            strict_process_cleanup: config.get_command_strict_process_cleanup(),
        };
        let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
        ai_runner.set_stream_json_textify(config.get_stream_json_textify());
        ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());

        // Initialize shared state (for testing, will use empty change list)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            agent,
            ai_runner,
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
            #[cfg(feature = "web-monitoring")]
            execution_mode: "select".to_string(),
        })
    }

    /// Update execution mode and broadcast state (helper for mode transitions)
    #[cfg(feature = "web-monitoring")]
    async fn update_execution_mode(&mut self, mode: &str) {
        self.execution_mode = mode.to_string();
        let current_changes = openspec::list_changes_native().unwrap_or_default();
        self.broadcast_state_update(&current_changes).await;
    }

    /// Check for graceful stop flag and update state accordingly
    /// Returns LoopControl indicating whether to continue or break
    async fn check_graceful_stop(
        &mut self,
        graceful_stop_flag: &Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        previous_graceful_stop: &mut bool,
    ) -> LoopControl {
        if let Some(ref graceful_flag) = graceful_stop_flag {
            let current_graceful_stop = graceful_flag.load(std::sync::atomic::Ordering::SeqCst);

            // Detect transition from false to true (entering stopping state)
            if current_graceful_stop && !*previous_graceful_stop {
                info!("Graceful stop requested, entering stopping state");
                #[cfg(feature = "web-monitoring")]
                self.update_execution_mode("stopping").await;
            }

            // Detect transition from true to false (cancel stop - resume running)
            if !current_graceful_stop && *previous_graceful_stop {
                info!("Graceful stop cancelled, resuming running state");
                #[cfg(feature = "web-monitoring")]
                self.update_execution_mode("running").await;
            }

            *previous_graceful_stop = current_graceful_stop;

            // If stop is still requested, exit loop
            if current_graceful_stop {
                info!("Graceful stop: stopping after current change");
                #[cfg(feature = "web-monitoring")]
                self.update_execution_mode("stopped").await;
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                return LoopControl::Break {
                    finish_status: "graceful_stop",
                };
            }
        }
        LoopControl::Continue
    }

    /// Check for cancellation token
    /// Returns LoopControl indicating whether to continue or break
    async fn check_cancellation(
        &mut self,
        cancel_token: &tokio_util::sync::CancellationToken,
    ) -> LoopControl {
        if cancel_token.is_cancelled() {
            info!("Cancellation requested, stopping orchestration");
            #[cfg(feature = "web-monitoring")]
            self.update_execution_mode("stopped").await;
            if let Some(progress) = &mut self.progress {
                progress.complete_all();
            }
            return LoopControl::Break {
                finish_status: "cancelled",
            };
        }
        LoopControl::Continue
    }

    /// Check max iterations limit and increment counter
    /// Returns LoopControl indicating whether to continue or break
    fn check_max_iterations(&mut self) -> LoopControl {
        self.iteration += 1;

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
                return LoopControl::Break {
                    finish_status: "iteration_limit",
                };
            }
        }
        LoopControl::Continue
    }

    /// Check all loop control conditions (graceful stop, cancellation, max iterations).
    /// Returns LoopControl indicating whether to continue or break.
    async fn check_loop_controls(
        &mut self,
        graceful_stop_flag: &Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        previous_graceful_stop: &mut bool,
        cancel_token: &tokio_util::sync::CancellationToken,
    ) -> LoopControl {
        // Check for graceful stop
        match self
            .check_graceful_stop(graceful_stop_flag, previous_graceful_stop)
            .await
        {
            LoopControl::Continue => {}
            break_control => return break_control,
        }

        // Check for cancellation
        match self.check_cancellation(cancel_token).await {
            LoopControl::Continue => {}
            break_control => return break_control,
        }

        // Check max iterations
        self.check_max_iterations()
    }

    /// Update shared state with an execution event
    async fn update_shared_state(&self, event: ExecutionEvent) {
        self.shared_state
            .write()
            .await
            .apply_execution_event(&event);
    }

    /// Handle Archived result
    async fn handle_archived(&mut self, next: &Change) {
        self.changes_processed += 1;

        self.update_shared_state(ExecutionEvent::ChangeArchived(next.id.clone()))
            .await;

        self.completed_change_ids.insert(next.id.clone());
        self.current_change_id = None;
        self.apply_counts.remove(&next.id);
        self.stall_detector.clear_change(&next.id);

        if let Some(progress) = &mut self.progress {
            progress.archive_change(&next.id);
        }
    }

    /// Handle Stalled result
    fn handle_stalled(&mut self, next: &Change, error: &str) -> LoopControl {
        warn!("Change stalled: {} - {}", next.id, error);
        self.mark_change_stalled(&next.id, error);
        LoopControl::Continue
    }

    /// Handle Failed result
    async fn handle_failed(&mut self, next: &Change, error: &str) -> Result<()> {
        error!("Change failed: {} - {}", next.id, error);
        if let Some(progress) = &mut self.progress {
            progress.error(&format!("Failed: {}", next.id));
        }
        #[cfg(feature = "web-monitoring")]
        self.update_execution_mode("error").await;
        Err(OrchestratorError::AgentCommand(error.to_string()))
    }

    /// Handle ApplySuccessIncomplete result
    async fn handle_apply_success_incomplete(
        &mut self,
        next: &Change,
        serial_service: &mut SerialRunService,
    ) -> LoopControl {
        self.update_shared_state(ExecutionEvent::ApplyCompleted {
            change_id: next.id.clone(),
            revision: "serial".to_string(),
        })
        .await;

        // CLI-specific: Create WIP snapshot
        let apply_count = serial_service.apply_count(&next.id);
        let snapshot = match self.snapshot_serial_iteration(&next.id, apply_count).await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                warn!("Failed to snapshot WIP commit for {}: {}", next.id, e);
                SerialSnapshot {
                    progress: crate::task_parser::TaskProgress::default(),
                    empty_commit: None,
                }
            }
        };

        // CLI-specific: Check for stall on empty commits
        if let Some(stall_reason) = serial_service.check_stall_after_apply(
            &next.id,
            &snapshot.progress,
            snapshot.empty_commit,
        ) {
            warn!("{}", stall_reason);
            self.mark_change_stalled(&next.id, &stall_reason);
            return LoopControl::Continue;
        }

        if let Some(progress) = &mut self.progress {
            progress.complete_change(&next.id);
        }
        LoopControl::Continue
    }

    /// Handle ApplyFailed result
    async fn handle_apply_failed(
        &mut self,
        next: &Change,
        error: &str,
        serial_service: &mut SerialRunService,
    ) -> Result<()> {
        self.update_shared_state(ExecutionEvent::ApplyStarted {
            change_id: next.id.clone(),
            command: "(placeholder)".to_string(),
        })
        .await;

        // CLI-specific: Create WIP snapshot even on failure
        let apply_count = serial_service.apply_count(&next.id);
        if let Err(e) = self.snapshot_serial_iteration(&next.id, apply_count).await {
            warn!("Failed to snapshot WIP commit for {}: {}", next.id, e);
        }

        // CLI-specific: Check circuit breaker
        if self.record_error_and_check_circuit_breaker(&next.id, error) {
            let message = format!(
                "Circuit breaker opened for '{}' due to repeated errors",
                next.id
            );
            warn!("{}", message);
            self.mark_change_stalled(&next.id, &message);
            serial_service.mark_stalled(&next.id, &message);
            return Ok(());
        }

        error!("Apply failed for {}: {}", next.id, error);
        if let Some(progress) = &mut self.progress {
            progress.error(&format!("Apply failed: {}", next.id));
        }
        #[cfg(feature = "web-monitoring")]
        self.update_execution_mode("error").await;
        Err(OrchestratorError::AgentCommand(error.to_string()))
    }

    /// Handle acceptance-related results (Passed, Continue, ContinueExceeded, Failed, CommandFailed, Blocked)
    async fn handle_acceptance_result(
        &mut self,
        next: &Change,
        serial_service: &mut SerialRunService,
        result: &crate::serial_run_service::ChangeProcessResult,
    ) {
        use crate::serial_run_service::ChangeProcessResult;

        // Common state update for all acceptance results
        self.update_shared_state(ExecutionEvent::ApplyCompleted {
            change_id: next.id.clone(),
            revision: "serial".to_string(),
        })
        .await;

        // Specific handling based on result type
        match result {
            ChangeProcessResult::AcceptancePassed => {
                // CLI-specific: Squash WIP commits after acceptance pass
                let apply_count = serial_service.apply_count(&next.id);
                let _ = self.squash_serial_wip_commits(&next.id, apply_count).await;
                info!("Acceptance passed for {}, ready for archive", next.id);
            }
            ChangeProcessResult::AcceptanceContinue => {
                info!(
                    "Acceptance requires continuation for {}, retrying...",
                    next.id
                );
            }
            ChangeProcessResult::AcceptanceContinueExceeded => {
                warn!(
                    "Acceptance CONTINUE limit exceeded for {}, treating as FAIL",
                    next.id
                );
            }
            ChangeProcessResult::AcceptanceBlocked => {
                warn!(
                    "Acceptance blocked for {} - implementation blocker detected, marking as stalled",
                    next.id
                );
                // Mark change as stalled to prevent re-selection and archive
                let reason = "Implementation blocker detected - requires manual intervention";
                self.mark_change_stalled(&next.id, reason);
                serial_service.mark_stalled(&next.id, reason);
            }
            ChangeProcessResult::AcceptanceFailed { .. } => {
                info!("Acceptance failed for {}, will retry apply", next.id);
            }
            ChangeProcessResult::AcceptanceCommandFailed { .. } => {
                info!(
                    "Acceptance command failed for {}, will retry apply",
                    next.id
                );
            }
            _ => {}
        }

        if let Some(progress) = &mut self.progress {
            progress.complete_change(&next.id);
        }
    }

    /// Initialize run loop state (shared state, progress display, serial service).
    /// Returns (filtered_initial_changes, serial_service, total_changes).
    async fn initialize_run_loop(
        &mut self,
        initial_changes: Vec<Change>,
    ) -> Result<(Vec<Change>, SerialRunService, usize)> {
        // Filter by target_changes if specified (early filtering)
        let filtered_initial = if let Some(targets) = &self.target_changes {
            // Explicit targets specified via --change option
            let mut found = Vec::new();
            for target in targets {
                let trimmed = target.trim();
                if let Some(change) = initial_changes.iter().find(|c| c.id == trimmed) {
                    found.push(change.clone());
                } else {
                    warn!("Specified change '{}' not found, skipping", trimmed);
                }
            }
            found
        } else {
            // No explicit target: return all changes
            initial_changes
        };

        if filtered_initial.is_empty() {
            // Return empty result - caller will handle early exit
            let repo_root = std::env::current_dir()?;
            let serial_service = SerialRunService::new(repo_root, self.config.clone());
            return Ok((filtered_initial, serial_service, 0));
        }

        // Store snapshot of change IDs (only the filtered ones)
        let snapshot_ids: HashSet<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        info!(
            "Captured snapshot of {} changes: {:?}",
            snapshot_ids.len(),
            snapshot_ids
        );
        self.initial_change_ids = Some(snapshot_ids.clone());

        // Initialize shared orchestration state with filtered changes (serial mode)
        let change_ids: Vec<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        *self.shared_state.write().await = OrchestratorState::new(change_ids, self.max_iterations);

        // Initialize progress display
        self.progress = Some(ProgressDisplay::new(filtered_initial.len()));

        let total_changes = filtered_initial.len();

        // Create serial run service for shared state and helpers
        let repo_root = std::env::current_dir()?;
        let serial_service = SerialRunService::new(repo_root, self.config.clone());

        Ok((filtered_initial, serial_service, total_changes))
    }

    /// Handle ChangeProcessResult and return LoopControl
    async fn handle_change_result(
        &mut self,
        result: crate::serial_run_service::ChangeProcessResult,
        next: &Change,
        serial_service: &mut SerialRunService,
    ) -> Result<LoopControl> {
        use crate::serial_run_service::ChangeProcessResult;

        match result {
            ChangeProcessResult::Archived => {
                self.handle_archived(next).await;
                Ok(LoopControl::Continue)
            }
            ChangeProcessResult::Stalled { error } => Ok(self.handle_stalled(next, &error)),
            ChangeProcessResult::Failed { error } => {
                self.handle_failed(next, &error).await?;
                Ok(LoopControl::Continue)
            }
            ChangeProcessResult::Cancelled => {
                info!("Processing cancelled for {}", next.id);
                Ok(LoopControl::Break {
                    finish_status: "cancelled",
                })
            }
            ChangeProcessResult::ChangeStopped => {
                // In CLI mode, single-change stop is not applicable (no TUI queue)
                // Treat it as a global cancel
                info!("Change {} stopped", next.id);
                Ok(LoopControl::Break {
                    finish_status: "stopped",
                })
            }
            ChangeProcessResult::ApplySuccessIncomplete => Ok(self
                .handle_apply_success_incomplete(next, serial_service)
                .await),
            ChangeProcessResult::ApplyFailed { error } => {
                self.handle_apply_failed(next, &error, serial_service)
                    .await?;
                Ok(LoopControl::Continue)
            }
            ChangeProcessResult::AcceptancePassed
            | ChangeProcessResult::AcceptanceContinue
            | ChangeProcessResult::AcceptanceContinueExceeded
            | ChangeProcessResult::AcceptanceFailed { .. }
            | ChangeProcessResult::AcceptanceCommandFailed { .. }
            | ChangeProcessResult::AcceptanceBlocked => {
                self.handle_acceptance_result(next, serial_service, &result)
                    .await;
                Ok(LoopControl::Continue)
            }
        }
    }

    /// Run the orchestration loop with cancellation support
    pub async fn run(
        &mut self,
        cancel_token: tokio_util::sync::CancellationToken,
        graceful_stop_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<()> {
        info!("Starting orchestration loop");

        // Set execution mode to running (for web monitoring)
        #[cfg(feature = "web-monitoring")]
        {
            self.execution_mode = "running".to_string();
        }

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
            return self
                .run_parallel(&initial_changes, cancel_token, graceful_stop_flag)
                .await;
        }

        if initial_changes.is_empty() {
            info!("No changes found");
            return Ok(());
        }

        // Initialize run loop state (shared state, progress display, serial service)
        let (filtered_initial, mut serial_service, total_changes) =
            self.initialize_run_loop(initial_changes).await?;

        if filtered_initial.is_empty() {
            info!("No changes found matching specified targets");
            return Ok(());
        }

        // Run on_start hook
        let start_context = HookContext::new(0, total_changes, total_changes, false);
        self.hooks
            .run_hook(HookType::OnStart, &start_context)
            .await?;

        let finish_status;

        // Track previous graceful stop state to detect transitions (false -> true)
        let mut previous_graceful_stop = false;

        loop {
            // Check all loop control conditions (graceful stop, cancellation, max iterations)
            match self
                .check_loop_controls(
                    &graceful_stop_flag,
                    &mut previous_graceful_stop,
                    &cancel_token,
                )
                .await
            {
                LoopControl::Continue => {}
                LoopControl::Break {
                    finish_status: status,
                } => {
                    finish_status = status;
                    break;
                }
            }

            // Refetch and select next change to process
            let (next, remaining_changes) =
                match self.refetch_and_select_change(&mut serial_service).await? {
                    Some(result) => result,
                    None => {
                        // All changes processed or stalled
                        finish_status = "completed";
                        break;
                    }
                };

            // Check if this is a new change (for state tracking)
            let is_new_change = self.current_change_id.as_ref() != Some(&next.id);
            if is_new_change {
                // Update shared state: processing started
                self.shared_state
                    .write()
                    .await
                    .apply_execution_event(&ExecutionEvent::ProcessingStarted(next.id.clone()));

                // Note: OnChangeStart hook is called by process_change() internally
                self.current_change_id = Some(next.id.clone());
            }

            // Process the change through SerialRunService
            let output = LogOutputHandler::new();
            let cancel_check = || false; // No cancellation in CLI mode
            let is_single_change_stopped = || false; // No single-change stop in CLI mode

            let result = serial_service
                .process_change(
                    &next,
                    &mut self.agent,
                    &self.ai_runner,
                    &self.hooks,
                    &output,
                    total_changes,
                    remaining_changes,
                    cancel_check,
                    is_single_change_stopped,
                    None, // No operation tracker in CLI mode
                )
                .await?;

            // Handle mode-specific concerns based on result
            match self
                .handle_change_result(result, &next, &mut serial_service)
                .await?
            {
                LoopControl::Continue => {}
                LoopControl::Break {
                    finish_status: status,
                } => {
                    finish_status = status;
                    break;
                }
            }
        }

        // Run on_finish hook
        let finish_context = HookContext::new(self.changes_processed, total_changes, 0, false)
            .with_status(finish_status);
        self.hooks
            .run_hook(HookType::OnFinish, &finish_context)
            .await?;

        // Set execution mode to stopped (for web monitoring)
        #[cfg(feature = "web-monitoring")]
        self.update_execution_mode("stopped").await;

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

    /// Refetch and filter changes for the current iteration.
    /// Returns None if loop should break (all changes processed or stalled).
    /// Returns Some((next_change, remaining_count)) if a change was selected.
    async fn refetch_and_select_change(
        &mut self,
        serial_service: &mut SerialRunService,
    ) -> Result<Option<(Change, usize)>> {
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
            return Ok(None);
        }

        let eligible_changes = self.filter_stalled_changes(&snapshot_changes);
        let remaining_changes = eligible_changes.len();

        if eligible_changes.is_empty() {
            info!("All remaining changes are blocked by stalled dependencies");
            if let Some(progress) = &mut self.progress {
                progress.complete_all();
            }
            return Ok(None);
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

        Ok(Some((next.clone(), remaining_changes)))
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

        if changes.is_empty() {
            println!("No changes found for parallel execution.");
            return Ok(());
        }

        // Use ParallelRunService to analyze groups (uses LLM if enabled)
        let repo_root = std::env::current_dir()?;
        let service = ParallelRunService::new(repo_root, self.config.clone());
        let groups = service.analyze_and_group_public(changes).await;

        // Display parallelization groups
        println!("\n=== Parallel Execution Plan (Dry Run) ===\n");
        println!("Total changes: {}", changes.len());
        println!("Parallelization groups: {}\n", groups.len());

        for group in &groups {
            println!("Group {} (can run in parallel):", group.id);
            for change_id in &group.changes {
                let change = changes.iter().find(|c| c.id == *change_id);
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
    async fn run_parallel(
        &mut self,
        changes: &[Change],
        cancel_token: tokio_util::sync::CancellationToken,
        graceful_stop_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<()> {
        info!("Running parallel execution mode");

        if changes.is_empty() {
            info!("No changes found for parallel execution");
            return Ok(());
        }

        // Store snapshot of change IDs
        let snapshot_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();
        self.initial_change_ids = Some(snapshot_ids);

        // Initialize shared orchestration state with parallel execution mode
        {
            let change_ids: Vec<String> = changes.iter().map(|c| c.id.clone()).collect();
            *self.shared_state.write().await = OrchestratorState::with_mode(
                change_ids,
                self.max_iterations,
                crate::orchestration::state::ExecutionMode::Parallel,
            );
        }

        // Use ParallelRunService for the common parallel execution flow
        let repo_root = std::env::current_dir()?;
        let mut service = ParallelRunService::new(repo_root.clone(), self.config.clone());
        service.set_no_resume(self.no_resume);

        // Check if Git is available for true parallel execution
        service.check_vcs_available().await?;

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

        // Monitor graceful_stop_flag and trigger cancellation if set
        // This allows Web control Stop to work in parallel mode
        if let Some(ref stop_flag) = graceful_stop_flag {
            let monitor_token = cancel_token.clone();
            let monitor_flag = stop_flag.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if monitor_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        info!("Graceful stop requested in parallel mode, cancelling execution");
                        monitor_token.cancel();
                        break;
                    }
                }
            });
        }

        // Track start-time rejections so we can report clearly when no work started.
        let total_requested = changes.len();
        let rejected_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let track_rejected = rejected_count.clone();

        // Run with a simple logging event handler for CLI mode
        let result = service
            .run_parallel(changes.to_vec(), Some(cancel_token), move |event| {
                // Log events for CLI mode (no TUI)
                use crate::parallel::ParallelEvent;
                #[cfg(feature = "web-monitoring")]
                if let Some(tx) = &web_event_sender {
                    let _ = tx.send(event.clone());
                }
                match event {
                    ParallelEvent::ParallelStartRejected {
                        ref change_ids,
                        ref reason,
                    } => {
                        // Immediately surface the rejection so the user knows these changes
                        // will not run, even before the overall completion message.
                        eprintln!(
                            "WARNING: {} change(s) rejected at start-time ({}): {}",
                            change_ids.len(),
                            reason,
                            change_ids.join(", ")
                        );
                        track_rejected
                            .fetch_add(change_ids.len(), std::sync::atomic::Ordering::SeqCst);
                    }
                    ParallelEvent::ApplyStarted { change_id, command } => {
                        info!("Apply started for {}", change_id);
                        println!("[{} apply] {}", change_id, command);
                    }
                    ParallelEvent::ApplyOutput {
                        change_id,
                        output,
                        iteration,
                    } => {
                        let iter = iteration
                            .map(|n| format!("#{}", n))
                            .unwrap_or_else(|| "".to_string());
                        if iter.is_empty() {
                            println!("[{} apply] {}", change_id, output);
                        } else {
                            println!("[{} apply {}] {}", change_id, iter, output);
                        }
                    }
                    ParallelEvent::ProgressUpdated {
                        change_id,
                        completed,
                        total,
                    } => {
                        if total > 0 {
                            info!("Progress {}: {}/{}", change_id, completed, total);
                        }
                    }
                    ParallelEvent::ApplyCompleted { change_id, .. } => {
                        info!("Apply completed for {}", change_id);
                    }
                    ParallelEvent::ApplyFailed { change_id, error } => {
                        error!("Apply failed for {}: {}", change_id, error);
                    }
                    ParallelEvent::AcceptanceStarted { change_id, command } => {
                        info!("Acceptance started for {}", change_id);
                        println!("[{} acceptance] {}", change_id, command);
                    }
                    ParallelEvent::AcceptanceOutput {
                        change_id,
                        output,
                        iteration,
                    } => {
                        let iter = iteration
                            .map(|n| format!("#{}", n))
                            .unwrap_or_else(|| "".to_string());
                        if iter.is_empty() {
                            println!("[{} acceptance] {}", change_id, output);
                        } else {
                            println!("[{} acceptance {}] {}", change_id, iter, output);
                        }
                    }
                    ParallelEvent::AcceptanceCompleted { change_id } => {
                        info!("Acceptance completed for {}", change_id);
                    }
                    ParallelEvent::AcceptanceFailed { change_id, error } => {
                        error!("Acceptance failed for {}: {}", change_id, error);
                    }
                    ParallelEvent::ArchiveStarted { change_id, command } => {
                        info!("Archive started for {}", change_id);
                        println!("[{} archive] {}", change_id, command);
                    }
                    ParallelEvent::ArchiveOutput {
                        change_id,
                        output,
                        iteration,
                    } => {
                        println!("[{} archive #{}] {}", change_id, iteration, output);
                    }
                    ParallelEvent::ChangeArchived(change_id) => {
                        info!("Archived {}", change_id);
                    }
                    ParallelEvent::ArchiveFailed { change_id, error } => {
                        error!("Archive failed for {}: {}", change_id, error);
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
                    ParallelEvent::Log(entry) => {
                        // Forward user-facing log entries in CLI mode as well.
                        println!("{}", entry.message);
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

        // Report clearly when all requested changes were rejected before any work started.
        let n_rejected = rejected_count.load(std::sync::atomic::Ordering::SeqCst);
        if n_rejected >= total_requested && total_requested > 0 {
            eprintln!(
                "ERROR: No changes started: all {} requested change(s) were rejected by \
                 start-time eligibility filter (uncommitted or not in HEAD). \
                 Commit your changes before running in parallel mode.",
                total_requested
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::ProposalMetadata;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
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
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "change-b".to_string(),
                completed_tasks: 0,
                total_tasks: 3,
                last_modified: "now".to_string(),
                dependencies: vec!["change-a".to_string()],
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "change-c".to_string(),
                completed_tasks: 0,
                total_tasks: 3,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
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

    #[tokio::test]
    async fn test_acceptance_blocked_prevents_reapply_and_archive() {
        use crate::serial_run_service::ChangeProcessResult;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config.clone()).unwrap();
        let mut serial_service = SerialRunService::new(temp_dir.path().to_path_buf(), config);

        let blocked_change = create_test_change("blocked-change", 3, 5);

        // Simulate AcceptanceBlocked result
        let result = ChangeProcessResult::AcceptanceBlocked;

        // Process the result through handle_change_result
        orchestrator
            .handle_change_result(result, &blocked_change, &mut serial_service)
            .await
            .unwrap();

        // Verify the change is marked as stalled in orchestrator
        assert!(orchestrator.stalled_change_ids.contains(&blocked_change.id));

        // Verify the change is marked as stalled in serial service
        assert!(serial_service.is_stalled(&blocked_change.id));

        // Create a list with the blocked change and another eligible change
        let changes = vec![
            blocked_change.clone(),
            create_test_change("other-change", 2, 5),
        ];

        // Filter should exclude the blocked change
        let eligible = orchestrator.filter_stalled_changes(&changes);
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].id, "other-change");
    }

    /// Regression: when ALL requested changes are rejected by start-time eligibility filtering,
    /// the CLI event callback must count them as rejected so the orchestrator can report that
    /// zero changes started.  This test directly exercises the rejected_count accumulation
    /// logic used in `run_parallel_in_parallel_mode` to trigger the
    /// "ERROR: No changes started" message.
    #[test]
    fn test_cli_all_rejected_start_detection() {
        use crate::parallel::ParallelEvent;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let total_requested: usize = 2;
        let rejected_count = Arc::new(AtomicUsize::new(0));
        let track_rejected = rejected_count.clone();

        // Mirror the event-callback logic from run_parallel_in_parallel_mode.
        let handle_event = move |event: ParallelEvent| {
            if let ParallelEvent::ParallelStartRejected { change_ids, .. } = event {
                track_rejected.fetch_add(change_ids.len(), Ordering::SeqCst);
            }
        };

        // Simulate a single ParallelStartRejected event covering all requested changes.
        handle_event(ParallelEvent::ParallelStartRejected {
            change_ids: vec!["change-a".to_string(), "change-b".to_string()],
            reason: "uncommitted or not in HEAD".to_string(),
        });

        let n_rejected = rejected_count.load(Ordering::SeqCst);
        assert_eq!(
            n_rejected, total_requested,
            "rejected_count must equal total_requested when all changes are filtered out"
        );
        // Verify the guard condition used in the orchestrator to emit the error message.
        assert!(
            n_rejected >= total_requested && total_requested > 0,
            "orchestrator should detect the all-rejected condition and report no changes started"
        );
    }
}
