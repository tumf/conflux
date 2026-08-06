use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::orchestration::state::OrchestratorState;
use crate::orchestration::target_resolution::{
    resolve_explicit_targets, ExplicitTargetPlan, RepositoryTargetEvidence, TargetResolution,
    TargetResolutionOptions,
};
use crate::orchestration::LogOutputHandler;
use crate::parallel::PostArchiveAction;
use crate::parallel_run_service::ParallelRunService;
use crate::tui::log_deduplicator;
use crate::vcs::git::commands as git_commands;
use crate::vcs::VcsBackend;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

#[cfg(feature = "web-monitoring")]
use crate::web::WebState;
#[cfg(feature = "web-monitoring")]
use tokio::sync::mpsc;

pub struct Orchestrator {
    config: OrchestratorConfig,
    /// Target changes specified by --change option (comma-separated)
    target_changes: Option<Vec<String>>,
    /// Snapshot of change IDs captured at run start.
    /// Only changes present in this snapshot will be processed during the run.
    /// This prevents mid-run proposals from being processed before they are ready.
    initial_change_ids: Option<HashSet<String>>,
    /// Hook runner for executing hooks at various stages
    hooks: HookRunner,
    /// Maximum iterations limit (0 = no limit)
    max_iterations: u32,
    /// Maximum concurrent workspaces
    max_concurrent: Option<usize>,
    /// Dry run mode (preview without execution)
    dry_run: bool,
    /// VCS backend for worktree execution
    #[allow(dead_code)] // Will be passed to ParallelRunService in future
    vcs_backend: VcsBackend,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
    /// Terminal action after a successful archive.
    post_archive_action: PostArchiveAction,
    /// Shared orchestration state (single source of truth for state tracking)
    /// Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
    shared_state: std::sync::Arc<tokio::sync::RwLock<OrchestratorState>>,
    /// Web monitoring state (for broadcasting updates to WebSocket clients)
    #[cfg(feature = "web-monitoring")]
    web_state: Option<Arc<WebState>>,
    /// Optional observability-only sink projecting execution events onto an
    /// external lifecycle adapter. It never participates in workflow routing.
    lifecycle_sink: Option<Arc<dyn crate::events::EventSink>>,
    /// Invocation-scoped upstream integration runtime.
    ///
    /// `None` is the default-off path: no checkpoint object is installed and no
    /// upstream fetch, merge, verification, event, or push is added. This is
    /// deliberately not part of persistent orchestration config.
    upstream_integration: Option<crate::upstream::UpstreamRuntime>,
    /// Base identity captured at run start.
    ///
    /// Explicit-target classification reads this base, so a mid-run branch
    /// change cannot silently move the completion evidence it was resolved
    /// against.
    captured_base_branch: Option<String>,
}

/// How explicit targets were resolved for a cumulative parallel run.
enum ParallelTargets {
    /// No explicit targets: `--all` semantics, unchanged.
    All(Vec<Change>),
    /// Classified against the captured base before dispatch.
    Resolved(TargetResolution),
    /// Classification deferred to the post-checkpoint boundary of a real `-u` run.
    Deferred {
        plan: ExplicitTargetPlan,
        seed: Vec<Change>,
    },
}

impl ParallelTargets {
    /// Changes handed to the scheduler at start.
    fn dispatch_changes(&self) -> Vec<Change> {
        match self {
            Self::All(changes) => changes.clone(),
            Self::Resolved(resolution) => resolution.dispatch_changes(),
            Self::Deferred { seed, .. } => seed.clone(),
        }
    }

    fn plan(&self) -> Option<ExplicitTargetPlan> {
        match self {
            Self::Deferred { plan, .. } => Some(plan.clone()),
            _ => None,
        }
    }

    fn resolution(&self) -> Option<&TargetResolution> {
        match self {
            Self::Resolved(resolution) => Some(resolution),
            _ => None,
        }
    }
}

impl Orchestrator {
    /// Create a new orchestrator with optional custom config path and max iterations override
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target_changes: Option<Vec<String>>,
        config_path: Option<PathBuf>,
        max_iterations_override: Option<u32>,
        max_concurrent: Option<usize>,
        dry_run: bool,
        vcs_override: Option<VcsBackend>,
        no_resume: bool,
        post_archive_action: PostArchiveAction,
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
        // VCS backend: CLI override takes precedence, then config, then auto
        let vcs_backend = vcs_override.unwrap_or_else(|| config.get_vcs_backend());

        // Initialize shared state (will be populated when run() is called with actual changes)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            config,
            target_changes,
            initial_change_ids: None,
            hooks,
            max_iterations,
            max_concurrent,
            dry_run,
            vcs_backend,
            no_resume,
            post_archive_action,
            shared_state,
            #[cfg(feature = "web-monitoring")]
            web_state: None,
            lifecycle_sink: None,
            upstream_integration: None,
            captured_base_branch: None,
        })
    }

    /// Install invocation-scoped upstream integration.
    ///
    /// Constructors are unchanged when the option is absent, so the default-off
    /// path cannot accidentally acquire upstream behavior.
    pub fn set_upstream_integration(&mut self, runtime: crate::upstream::UpstreamRuntime) {
        self.upstream_integration = Some(runtime);
    }

    /// Currently installed upstream runtime, if any.
    #[cfg(test)]
    pub fn upstream_integration(&self) -> Option<&crate::upstream::UpstreamRuntime> {
        self.upstream_integration.as_ref()
    }

    /// Attach an external lifecycle handle.
    ///
    /// Observability-only: the resulting sink receives a read-only projection of
    /// execution events and cannot influence scheduling, acceptance, archive,
    /// merge, or resume decisions.
    pub fn set_lifecycle_handle(
        &mut self,
        handle: crate::lifecycle_integration::LifecycleHandle,
        workspace: Option<String>,
    ) {
        if !handle.is_enabled() {
            return;
        }
        self.lifecycle_sink = Some(Arc::new(crate::events::LifecycleEventSink::new(
            handle, workspace,
        )));
    }

    /// Set web monitoring state for broadcasting updates to WebSocket clients.
    /// Also injects the shared orchestration state reference into WebState for unified tracking.
    #[cfg(feature = "web-monitoring")]
    pub async fn set_web_state(&mut self, web_state: Arc<WebState>) {
        // Inject shared state reference into WebState
        web_state.set_shared_state(self.shared_state.clone()).await;
        self.web_state = Some(web_state);
    }

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

        // Initialize shared state (for testing, will use empty change list)
        // Wrapped in Arc<RwLock<>> to allow sharing with TUI/Web monitoring
        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            Vec::new(),
            max_iterations,
        )));

        Ok(Self {
            config,
            target_changes,
            initial_change_ids: None,
            hooks,
            max_iterations,
            max_concurrent: None,
            dry_run: false,
            vcs_backend: VcsBackend::Auto,
            no_resume: false,
            post_archive_action: PostArchiveAction::MergeToBase,
            shared_state,
            #[cfg(feature = "web-monitoring")]
            web_state: None,
            lifecycle_sink: None,
            upstream_integration: None,
            captured_base_branch: None,
        })
    }

    /// Capture the base identity attached at run start.
    ///
    /// An enabled upstream run names its cumulative base explicitly; every other
    /// run reads the attached local branch once and reuses it, so classification
    /// evidence cannot move underneath the run. A missing or detached base
    /// identity fails here, before any classification could claim completion.
    async fn capture_base_branch(&mut self) -> Result<String> {
        if let Some(base) = &self.captured_base_branch {
            return Ok(base.clone());
        }

        let base =
            match &self.upstream_integration {
                Some(runtime) => runtime.branch.clone(),
                None => {
                    let repo_root = std::env::current_dir()?;
                    match git_commands::get_current_branch(&repo_root).await {
                        Ok(Some(branch)) => branch,
                        Ok(None) => return Err(OrchestratorError::Parse(
                            "cannot resolve explicit run targets: HEAD is detached, so there is \
                             no base branch to prove completion against"
                                .to_string(),
                        )),
                        Err(err) => {
                            return Err(OrchestratorError::GitCommand(format!(
                                "cannot resolve explicit run targets: failed to read the attached \
                             base branch: {}",
                                err
                            )))
                        }
                    }
                }
            };

        self.captured_base_branch = Some(base.clone());
        Ok(base)
    }

    /// Classify explicit parallel targets against the captured base.
    ///
    /// Returns `None` when the invocation has no explicit targets, which keeps
    /// `--all` behavior unchanged.
    async fn classify_explicit_parallel_targets(
        &mut self,
        active_changes: &[Change],
    ) -> Result<Option<TargetResolution>> {
        let Some(targets) = self.target_changes.clone() else {
            return Ok(None);
        };

        let base_branch = self.capture_base_branch().await?;
        let repo_root = std::env::current_dir()?;
        let evidence = RepositoryTargetEvidence::new(repo_root, base_branch);
        let resolution = resolve_explicit_targets(
            &targets,
            active_changes,
            &evidence,
            TargetResolutionOptions {
                no_resume: self.no_resume,
            },
        )
        .await;

        Ok(Some(resolution))
    }

    /// Build the deferred classification plan for an enabled real `-u` run.
    async fn explicit_parallel_target_plan(&mut self) -> Result<Option<ExplicitTargetPlan>> {
        let Some(targets) = self.target_changes.clone() else {
            return Ok(None);
        };

        let base_branch = self.capture_base_branch().await?;
        Ok(Some(ExplicitTargetPlan::new(
            targets,
            base_branch,
            TargetResolutionOptions {
                no_resume: self.no_resume,
            },
        )))
    }

    /// Report an ordered classification to the operator.
    fn report_target_resolution(resolution: &TargetResolution) {
        for line in resolution.report_lines() {
            info!("{}", line);
            println!("{}", line);
        }
    }

    /// Resolve explicit parallel targets into dispatchable changes.
    ///
    /// Ordinary runs classify immediately against the captured local base. An
    /// enabled real `-u` run defers classification to the post-checkpoint
    /// boundary inside the executor, because the mandatory initial upstream
    /// checkpoint can integrate the archive that proves a target is complete.
    async fn resolve_parallel_targets(
        &mut self,
        initial_changes: &[Change],
    ) -> Result<ParallelTargets> {
        if self.target_changes.is_none() {
            return Ok(ParallelTargets::All(initial_changes.to_vec()));
        }

        if self.upstream_integration.is_some() && !self.dry_run {
            let plan = self
                .explicit_parallel_target_plan()
                .await?
                .expect("explicit targets present");
            // Seed the executor with the requested targets that are active right
            // now. Unknown IDs are not rejected here: the post-checkpoint
            // classification owns that decision.
            let requested: HashSet<&str> = plan.requested().iter().map(|id| id.trim()).collect();
            let seed = initial_changes
                .iter()
                .filter(|change| requested.contains(change.id.as_str()))
                .cloned()
                .collect();
            return Ok(ParallelTargets::Deferred { plan, seed });
        }

        let resolution = self
            .classify_explicit_parallel_targets(initial_changes)
            .await?
            .expect("explicit targets present");
        Self::report_target_resolution(&resolution);
        if let Some(err) = resolution.failure_error() {
            return Err(err);
        }
        Ok(ParallelTargets::Resolved(resolution))
    }

    /// Run cumulative worktree orchestration for the resolved targets.
    ///
    /// There is one execution path: every selected change — including a single
    /// explicit target — is dispatched to the cumulative worktree scheduler.
    pub async fn run(
        &mut self,
        cancel_token: tokio_util::sync::CancellationToken,
        graceful_stop_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<()> {
        info!("Starting orchestration loop");

        // Capture initial snapshot of change IDs at run start.
        // Only changes present at this point will be processed during the run.
        // This prevents mid-run proposals from being processed before they are ready.
        let initial_changes = openspec::list_changes_native()?;

        // Explicit targets are resolved from repository evidence instead of the
        // active list alone, so a repeated target set that already completed is
        // skipped instead of rejected as unknown.
        let targets = self.resolve_parallel_targets(&initial_changes).await?;

        if self.dry_run {
            // Read-only classification against the current local base: no
            // network fetch and no workspace mutation or cleanup.
            return self
                .run_parallel_dry_run(&targets.dispatch_changes(), targets.resolution())
                .await;
        }

        self.run_parallel(targets, cancel_token, graceful_stop_flag)
            .await
    }

    /// Preview the execution plan without running anything (dry run)
    ///
    /// The optional resolution is the read-only explicit-target classification
    /// performed against the current local base. Dry run reports it without any
    /// network fetch, workspace mutation, or workspace cleanup.
    async fn run_parallel_dry_run(
        &self,
        changes: &[Change],
        resolution: Option<&TargetResolution>,
    ) -> Result<()> {
        info!("Running dry run (preview only)");

        if let Some(resolution) = resolution {
            println!("\n=== Explicit Target Classification (Dry Run) ===\n");
            for line in resolution.report_lines() {
                println!("{}", line);
            }
            println!();
        }

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

    /// Run `on_finish` exactly once for a parallel run.
    ///
    /// A change stopped by the shared Apply-dispatch ceiling was recorded by its
    /// workspace task as a typed observation, so the hook receives
    /// `iteration_limit` together with that change's exact cumulative dispatch
    /// count instead of a status the hook would have to infer from log text.
    async fn run_parallel_finish_hook(&self) -> Result<()> {
        let state = self.shared_state.read().await;
        let iteration_limit = state.apply_iteration_limits().first().cloned();
        let (finish_status, finish_apply_count) = state.parallel_finish_report();
        let processed = state.changes_processed();
        let total = state.total_changes();
        drop(state);

        if let Some(record) = &iteration_limit {
            info!(
                change_id = %record.change_id,
                attempts = record.attempts,
                max = record.max,
                "Parallel run stopped on the Apply-dispatch ceiling"
            );
        }

        let finish_context = HookContext::new(processed, total, 0, false)
            .with_status(finish_status)
            .with_apply_count(finish_apply_count);
        self.hooks
            .run_hook(HookType::OnFinish, &finish_context)
            .await
    }

    /// Run parallel execution mode
    async fn run_parallel(
        &mut self,
        targets: ParallelTargets,
        cancel_token: tokio_util::sync::CancellationToken,
        graceful_stop_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<()> {
        info!("Running parallel execution mode");

        let changes = targets.dispatch_changes();
        let changes = changes.as_slice();
        let explicit_plan = targets.plan();

        if changes.is_empty() {
            // An enabled upstream run still enters the executor with an empty
            // queue: deferred classification, zero-change upstream recovery, and
            // finalization all live behind that boundary.
            if self.upstream_integration.is_none() {
                info!("No changes found for parallel execution");
                if let Some(resolution) = targets.resolution() {
                    if !resolution.already_completed_ids().is_empty() {
                        info!(
                            "All requested targets are already integrated into base: {}",
                            resolution.already_completed_ids().join(", ")
                        );
                    }
                }
                return Ok(());
            }
            info!(
                "No dispatchable changes at start; continuing so upstream integration can \
                 classify targets and finalize"
            );
        }

        // Store snapshot of change IDs
        let snapshot_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();
        self.initial_change_ids = Some(snapshot_ids);

        // Initialize shared orchestration state with parallel execution mode
        {
            let change_ids: Vec<String> = changes.iter().map(|c| c.id.clone()).collect();
            *self.shared_state.write().await =
                OrchestratorState::new(change_ids, self.max_iterations);
        }

        // Use ParallelRunService for the common parallel execution flow
        let repo_root = std::env::current_dir()?;
        let mut service = ParallelRunService::new(repo_root.clone(), self.config.clone());
        service.set_no_resume(self.no_resume);
        service.set_post_archive_action(self.post_archive_action.clone());
        service.set_shared_orchestrator_state(self.shared_state.clone());
        if let Some(runtime) = self.upstream_integration.clone() {
            service.set_upstream_integration(runtime);
        }
        if let Some(plan) = explicit_plan.clone() {
            service.set_explicit_target_plan(plan);
        }

        // Check if Git is available for true parallel execution
        service.check_vcs_available().await?;

        info!("Git available, executing changes in parallel using worktrees");

        #[cfg(feature = "web-monitoring")]
        let (web_event_tx, web_event_handle) = if let Some(web_state) = self.web_state.clone() {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let handle = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    crate::web::WebState::apply_execution_event(&web_state, &event).await;
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
        // This allows Web control Stop to work during orchestration
        if let Some(ref stop_flag) = graceful_stop_flag {
            let monitor_token = cancel_token.clone();
            let monitor_flag = stop_flag.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if monitor_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        info!("Graceful stop requested, cancelling execution");
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
                    ParallelEvent::ApplyCommitPhase {
                        change_id,
                        phase,
                        attempt,
                    } => {
                        // Headless runs have no `[commit]` row to update, so the
                        // subphase is reported as a line like any other Apply
                        // progress. It stays presentation only.
                        info!(
                            "Apply commit phase for {} (attempt {}): {}",
                            change_id,
                            attempt,
                            phase.as_str()
                        );
                        println!("[{} commit #{}] {}", change_id, attempt, phase.as_str());
                    }
                    ParallelEvent::ApplyCommitOutput {
                        change_id,
                        attempt,
                        stream,
                        line,
                    } => {
                        // Mirrors the `ApplyOutput` arm: streamed repository-hook
                        // output must be visible while the commit runs, not only
                        // after it is classified.
                        println!(
                            "[{} commit #{} {}] {}",
                            change_id,
                            attempt,
                            stream.as_str(),
                            line
                        );
                    }
                    ParallelEvent::ProgressUpdated {
                        change_id,
                        completed,
                        total,
                    } if total > 0 => {
                        info!("Progress {}: {}/{}", change_id, completed, total);
                    }
                    ParallelEvent::ApplyCompleted { change_id, .. } => {
                        info!("Apply completed for {}", change_id);
                    }
                    ParallelEvent::ApplyFailed { change_id, error } => {
                        error!("Apply failed for {}: {}", change_id, error);
                    }
                    ParallelEvent::AcceptanceStarted { change_id, command } => {
                        info!("Acceptance started for {}", change_id);
                        println!(
                            "[{} acceptance] {}",
                            change_id,
                            crate::events::command_log_summary(&command)
                        );
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
                    ParallelEvent::ArchiveResumed {
                        change_id,
                        reason,
                        summary,
                    } => {
                        info!(
                            "Archive resumed for {} (reason={:?}, summary={:?})",
                            change_id, reason, summary
                        );
                    }
                    ParallelEvent::ArchiveRetryScheduled {
                        change_id,
                        attempt,
                        max_attempts,
                        reason,
                        summary,
                    } => {
                        warn!(
                            "Archive retry scheduled for {} ({}/{}): reason={:?}, summary={:?}",
                            change_id, attempt, max_attempts, reason, summary
                        );
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
                    ParallelEvent::ArchiveFailed {
                        change_id,
                        error,
                        reason,
                        summary,
                    } => {
                        error!(
                            "Archive failed for {}: {} (reason={:?}, summary={:?})",
                            change_id, error, reason, summary
                        );
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

        // A deferred `-u` classification is resolved inside the executor, after
        // its initial checkpoint. Report it before propagating any error so the
        // operator sees which targets were processed, skipped, or unresolved.
        if let Some(plan) = &explicit_plan {
            if let Some(resolution) = plan.resolved().await {
                Self::report_target_resolution(&resolution);
            }
        }

        result?;

        self.run_parallel_finish_hook().await?;

        // Report clearly when all requested changes were rejected before any work started.
        let n_rejected = rejected_count.load(std::sync::atomic::Ordering::SeqCst);
        if n_rejected >= total_requested && total_requested > 0 {
            eprintln!(
                "ERROR: No changes started: all {} requested change(s) were rejected by \
                 start-time eligibility filter (uncommitted or not in HEAD). \
                 Commit your changes before running.",
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
    use crate::orchestration::target_resolution::{
        BaseCompletionEvidence, BaseEvidenceErrorKind, TargetEvidence, WorkspaceResumeEvidence,
    };
    use std::collections::HashMap;

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

    /// The run has no per-change return path for a typed Apply outcome, so the
    /// finish hook is driven from the typed observation the workspace task
    /// recorded on the reducer.
    mod parallel_finish_hook {
        use super::*;
        use crate::hooks::{HookConfig, HookConfigValue, HooksConfig};
        use tempfile::TempDir;

        fn orchestrator_with_on_finish(log: &std::path::Path) -> Orchestrator {
            let config = OrchestratorConfig {
                hooks: Some(HooksConfig {
                    on_finish: Some(HookConfigValue::Full(HookConfig {
                        command: format!(
                            "sh -c 'echo \"$OPENSPEC_STATUS $OPENSPEC_APPLY_COUNT\" >> {}'",
                            log.display()
                        ),
                        continue_on_failure: false,
                        timeout: 30,
                        git_commit_no_verify: false,
                        max_retries: 0,
                        retry_delay_secs: 0,
                    })),
                    ..Default::default()
                }),
                ..Default::default()
            };
            Orchestrator::with_config(None, config).expect("test orchestrator")
        }

        fn lines(log: &std::path::Path) -> Vec<String> {
            std::fs::read_to_string(log)
                .unwrap_or_default()
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_recorded_iteration_limit_reports_status_and_exact_count_once() {
            let temp_dir = TempDir::new().unwrap();
            let log = temp_dir.path().join("on-finish.log");
            let orchestrator = orchestrator_with_on_finish(&log);

            {
                let mut state = orchestrator.shared_state.write().await;
                *state = OrchestratorState::new(vec!["change-a".to_string()], 7);
                // The workspace task's typed observation: the ceiling refused
                // dispatch 8 after 7 cumulative dispatches.
                state.record_apply_iteration_limit("change-a", 7, 7);
                // A repeated observation for the same change must not duplicate.
                state.record_apply_iteration_limit("change-a", 7, 7);
            }

            orchestrator
                .run_parallel_finish_hook()
                .await
                .expect("the finish hook must run");

            assert_eq!(
                lines(&log),
                vec!["iteration_limit 7".to_string()],
                "on_finish runs exactly once with the typed status and the exact count"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_run_without_an_iteration_limit_reports_completed() {
            let temp_dir = TempDir::new().unwrap();
            let log = temp_dir.path().join("on-finish.log");
            let orchestrator = orchestrator_with_on_finish(&log);

            {
                let mut state = orchestrator.shared_state.write().await;
                *state = OrchestratorState::new(vec!["change-a".to_string()], 7);
            }

            orchestrator
                .run_parallel_finish_hook()
                .await
                .expect("the finish hook must run");

            assert_eq!(lines(&log), vec!["completed 0".to_string()]);
        }
    }

    /// In-memory evidence double.
    ///
    /// Explicit-target classification is decision logic, so it is verified here
    /// without a repository, worktree, Git process, or clock. Real-boundary
    /// coverage lives in the heavy `explicit_target_resume_*` e2e suite.
    struct FakeEvidence {
        base: HashMap<String, BaseCompletionEvidence>,
        workspace: HashMap<String, WorkspaceResumeEvidence>,
    }

    impl FakeEvidence {
        fn new() -> Self {
            Self {
                base: HashMap::new(),
                workspace: HashMap::new(),
            }
        }

        fn with_base(mut self, id: &str, evidence: BaseCompletionEvidence) -> Self {
            self.base.insert(id.to_string(), evidence);
            self
        }

        fn with_workspace(mut self, id: &str, evidence: WorkspaceResumeEvidence) -> Self {
            self.workspace.insert(id.to_string(), evidence);
            self
        }
    }

    #[async_trait::async_trait]
    impl TargetEvidence for FakeEvidence {
        async fn base_completion(&self, change_id: &str) -> BaseCompletionEvidence {
            self.base
                .get(change_id)
                .cloned()
                .unwrap_or(BaseCompletionEvidence::NotCompleted)
        }

        async fn workspace_resume(&self, change_id: &str) -> WorkspaceResumeEvidence {
            self.workspace.get(change_id).cloned().unwrap_or(
                WorkspaceResumeEvidence::NotResumable {
                    detail: "no managed workspace".to_string(),
                },
            )
        }
    }

    fn resumable(id: &str) -> WorkspaceResumeEvidence {
        WorkspaceResumeEvidence::Resumable {
            path: PathBuf::from(format!("/tmp/ws-{id}")),
            change: Box::new(create_test_change(id, 1, 3)),
        }
    }

    async fn resolve(
        requested: &[&str],
        active: &[Change],
        evidence: &FakeEvidence,
        no_resume: bool,
    ) -> TargetResolution {
        let requested: Vec<String> = requested.iter().map(|id| id.to_string()).collect();
        resolve_explicit_targets(
            &requested,
            active,
            evidence,
            TargetResolutionOptions { no_resume },
        )
        .await
    }

    #[tokio::test]
    async fn explicit_target_resume_classification_table_covers_every_class() {
        let active = vec![create_test_change("active-change", 0, 2)];
        let evidence = FakeEvidence::new()
            .with_base("completed-change", BaseCompletionEvidence::Completed)
            .with_base(
                "contradictory-change",
                BaseCompletionEvidence::Contradictory {
                    detail: "archive entry and active change directory both exist".to_string(),
                },
            )
            .with_base(
                "unreadable-change",
                BaseCompletionEvidence::EvidenceError {
                    kind: BaseEvidenceErrorKind::MissingBranch,
                    detail: "base branch 'main' does not exist".to_string(),
                },
            )
            .with_workspace("resumable-change", resumable("resumable-change"));

        let resolution = resolve(
            &[
                "active-change",
                "completed-change",
                "resumable-change",
                "unknown-change",
                "contradictory-change",
                "unreadable-change",
                "active-change",
            ],
            &active,
            &evidence,
            false,
        )
        .await;

        let classified: Vec<(&str, &str)> = resolution
            .targets
            .iter()
            .map(|t| (t.requested_id.as_str(), t.classification.as_str()))
            .collect();
        assert_eq!(
            classified,
            vec![
                ("active-change", "active"),
                ("completed-change", "already_completed"),
                ("resumable-change", "resumable_workspace"),
                ("unknown-change", "unknown"),
                ("contradictory-change", "evidence_error"),
                ("unreadable-change", "evidence_error"),
            ],
            "each class is classified from its own evidence, in deduplicated request order"
        );
        assert_eq!(resolution.duplicates, vec!["active-change".to_string()]);
    }

    #[tokio::test]
    async fn explicit_target_resume_aggregates_all_diagnostics_in_one_error() {
        let evidence = FakeEvidence::new().with_base(
            "broken-change",
            BaseCompletionEvidence::EvidenceError {
                kind: BaseEvidenceErrorKind::CommandFailure,
                detail: "Failed to list archive tree: boom".to_string(),
            },
        );

        let resolution = resolve(
            &["missing-a", "broken-change", "missing-b", "missing-a"],
            &[],
            &evidence,
            false,
        )
        .await;

        let message = resolution
            .failure_error()
            .expect("unresolvable targets reject the invocation")
            .to_string();
        assert!(
            message.contains("duplicate change IDs: missing-a"),
            "{message}"
        );
        assert!(
            message.contains("unknown change IDs: missing-a, missing-b"),
            "{message}"
        );
        assert!(
            message.contains(
                "unusable change evidence: broken-change (Failed to list archive tree: boom)"
            ),
            "{message}"
        );
    }

    #[tokio::test]
    async fn explicit_target_resume_keeps_requested_order_and_separates_completed_ids() {
        let active = vec![
            create_test_change("change-a", 0, 1),
            create_test_change("change-c", 0, 1),
        ];
        let evidence = FakeEvidence::new()
            .with_base("change-b", BaseCompletionEvidence::Completed)
            .with_workspace("change-d", resumable("change-d"));

        let resolution = resolve(
            &["change-c", "change-b", "change-d", "change-a"],
            &active,
            &evidence,
            false,
        )
        .await;

        assert!(resolution.failure_error().is_none());
        assert_eq!(
            resolution.requested_ids(),
            vec!["change-c", "change-b", "change-d", "change-a"]
        );
        assert_eq!(
            resolution.processed_ids(),
            vec!["change-c", "change-d", "change-a"],
            "already-completed IDs are held separately rather than reordering the rest"
        );
        assert_eq!(resolution.already_completed_ids(), vec!["change-b"]);
        assert_eq!(
            resolution
                .dispatch_changes()
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>(),
            vec!["change-c", "change-d", "change-a"]
        );
    }

    #[tokio::test]
    async fn explicit_target_resume_prefers_active_over_candidate_workspace() {
        let active = vec![create_test_change("change-a", 4, 4)];
        let evidence = FakeEvidence::new().with_workspace("change-a", resumable("change-a"));

        let resolution = resolve(&["change-a"], &active, &evidence, false).await;

        assert_eq!(resolution.active_ids(), vec!["change-a"]);
        assert!(resolution.resumable_ids().is_empty());
        assert_eq!(
            resolution.dispatch_changes()[0].completed_tasks,
            4,
            "active metadata wins over workspace-reconstructed metadata"
        );
    }

    #[tokio::test]
    async fn explicit_target_resume_no_resume_keeps_completed_but_refuses_workspace_only() {
        let evidence = FakeEvidence::new()
            .with_base("done-change", BaseCompletionEvidence::Completed)
            .with_workspace("workspace-only", resumable("workspace-only"));

        let resolution = resolve(&["done-change", "workspace-only"], &[], &evidence, true).await;

        assert_eq!(
            resolution.already_completed_ids(),
            vec!["done-change"],
            "--no-resume never erases base-integrated completion"
        );
        assert_eq!(resolution.resume_refused_ids(), vec!["workspace-only"]);
        assert!(
            resolution.dispatch_changes().is_empty(),
            "a refused target is not dispatched and its workspace is not replaced"
        );
        let message = resolution.failure_error().unwrap().to_string();
        assert!(
            message.contains("workspace-only change IDs refused by --no-resume"),
            "{message}"
        );
        assert!(message.contains("rerun without --no-resume"), "{message}");
    }

    #[tokio::test]
    async fn explicit_target_resume_unreadable_workspace_fails_safely() {
        let evidence = FakeEvidence::new().with_workspace(
            "broken-workspace",
            WorkspaceResumeEvidence::EvidenceError {
                detail: "managed workspace '/tmp/ws' state is unreadable: git failed".to_string(),
            },
        );

        let resolution = resolve(&["broken-workspace"], &[], &evidence, false).await;

        assert_eq!(resolution.evidence_error_ids(), vec!["broken-workspace"]);
        assert!(resolution.already_completed_ids().is_empty());
        assert!(resolution.dispatch_changes().is_empty());
    }

    #[tokio::test]
    async fn explicit_target_resume_report_lines_expose_ordered_classification() {
        let active = vec![create_test_change("change-a", 0, 1)];
        let evidence = FakeEvidence::new()
            .with_base("change-b", BaseCompletionEvidence::Completed)
            .with_workspace("change-c", resumable("change-c"));

        let resolution = resolve(
            &["change-a", "change-b", "change-c", "change-x"],
            &active,
            &evidence,
            false,
        )
        .await;

        let lines = resolution.report_lines();
        assert_eq!(
            lines[0],
            "Explicit targets requested: change-a, change-b, change-c, change-x"
        );
        assert!(lines.contains(&"  to process: change-a, change-c".to_string()));
        assert!(lines.contains(&"  resumable workspaces: change-c".to_string()));
        assert!(lines.contains(&"  already completed (skipped): change-b".to_string()));
        assert_eq!(resolution.pending_ids(), vec!["change-x"]);
    }

    #[tokio::test]
    async fn explicit_target_resume_empty_and_whitespace_targets_are_ignored() {
        let active = vec![create_test_change("change-a", 0, 1)];
        let evidence = FakeEvidence::new();

        let resolution = resolve(&["", "  ", " change-a "], &active, &evidence, false).await;

        assert_eq!(resolution.requested_ids(), vec!["change-a"]);
        assert!(resolution.failure_error().is_none());
    }

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        assert!(orchestrator.target_changes.is_none());
        assert!(orchestrator.initial_change_ids.is_none());

        let state = orchestrator.shared_state.read().await;
        assert!(state.current_change_id().is_none());
        assert_eq!(state.changes_processed(), 0);
        assert_eq!(state.iteration(), 0);
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
