//! TUI runner and main event loop
//!
//! Contains run_tui and run_tui_loop functions.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::lifecycle_integration::LifecycleHandle;
use crate::openspec::Change;
use crate::parallel::PostArchiveAction;
use crate::vcs::{GitWorkspaceManager, WorkspaceManager};
use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
use ratatui::DefaultTerminal;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::command_handlers::{handle_tui_command, TuiCommandContext};
use super::events::{LogEntry, OrchestratorEvent, TuiCommand};
use super::key_handlers::{handle_key_event, KeyEventContext};
use super::lifecycle::TuiLifecycleSnapshot;
use super::log_deduplicator;
// orchestrator functions now called from command_handlers
use super::queue::DynamicQueue;
use super::render::{render, SPINNER_CHARS};
use super::state::{AppState, AUTO_REFRESH_INTERVAL_SECS};
// AppMode/StopMode are used in handlers
use super::terminal::restore_terminal;
use super::worktrees::load_worktrees_with_conflict_check;

fn is_refresh_root_usable(repo_root: &Path) -> bool {
    repo_root.is_dir()
}

fn should_skip_local_refresh(repo_root: &Path, stale_refresh_root_warned: &mut bool) -> bool {
    let refresh_root_usable = is_refresh_root_usable(repo_root);
    if !refresh_root_usable {
        if !*stale_refresh_root_warned {
            *stale_refresh_root_warned = true;
            warn!(
                repo_root = %repo_root.display(),
                "Skipping local TUI auto-refresh: stale or missing refresh root"
            );
        }
        return true;
    }

    *stale_refresh_root_warned = false;
    false
}

fn refresh_local_changes(repo_root: &Path) -> Result<(Vec<Change>, Vec<Change>)> {
    let active_changes = crate::openspec::list_changes_native_from(repo_root)?;
    let rejected_changes = crate::openspec::list_rejected_changes_native_from(repo_root)?;
    Ok((active_changes, rejected_changes))
}

/// Whether this event can change the reducer-derived display caches the TUI
/// renders from.
///
/// It no longer decides whether the reducer is *written* — the dispatch owner
/// does that, exactly once, before the event reaches this frontend. It only
/// decides whether re-reading the reducer afterwards could show anything new,
/// so a chatty output event does not cost a lock and two map rebuilds per chunk.
fn should_apply_event_to_tui_reducer(event: &crate::events::ExecutionEvent) -> bool {
    use crate::events::ExecutionEvent;

    match event {
        // Reducer-visible lifecycle and workspace observations that derive TUI display status,
        // queue intent, active counts, wait states, or terminal state.
        ExecutionEvent::ProcessingStarted(_)
        | ExecutionEvent::ProcessingCompleted(_)
        | ExecutionEvent::ProcessingError { .. }
        | ExecutionEvent::ApplyStarted { .. }
        | ExecutionEvent::ApplyCompleted { .. }
        | ExecutionEvent::ApplyFailed { .. }
        | ExecutionEvent::ArchiveStarted { .. }
        | ExecutionEvent::ArchiveResumed { .. }
        | ExecutionEvent::ArchiveRetryScheduled { .. }
        | ExecutionEvent::ChangeArchived(_)
        | ExecutionEvent::ArchiveFailed { .. }
        | ExecutionEvent::AcceptanceStarted { .. }
        | ExecutionEvent::AcceptanceCompleted { .. }
        | ExecutionEvent::AcceptanceFailed { .. }
        | ExecutionEvent::ChangeRejected { .. }
        | ExecutionEvent::RejectionReviewCompleted { .. }
        | ExecutionEvent::RejectionReviewFailed { .. }
        | ExecutionEvent::WorkspaceStatusUpdated { .. }
        | ExecutionEvent::PushStarted { .. }
        | ExecutionEvent::PushCompleted { .. }
        | ExecutionEvent::PushFailed { .. }
        | ExecutionEvent::MergeCompleted { .. }
        | ExecutionEvent::MergeDeferred { .. }
        | ExecutionEvent::ResolveStarted { .. }
        | ExecutionEvent::ResolveCompleted { .. }
        | ExecutionEvent::ResolveFailed { .. }
        | ExecutionEvent::DependencyBlocked { .. }
        | ExecutionEvent::DependencyResolved { .. }
        | ExecutionEvent::AcceptanceGated { .. }
        | ExecutionEvent::ExecutionBlocked { .. }
        | ExecutionEvent::ChangeDequeued { .. }
        | ExecutionEvent::ChangeStopped { .. }
        | ExecutionEvent::ChangesRefreshed { .. } => true,

        // Presentation-only or unrelated TUI events do not affect reducer display state.
        ExecutionEvent::ApplyOutput { .. }
        | ExecutionEvent::ArchiveOutput { .. }
        | ExecutionEvent::AcceptanceOutput { .. }
        | ExecutionEvent::ProgressUpdated { .. }
        | ExecutionEvent::WorkspaceCreated { .. }
        | ExecutionEvent::WorkspaceResumed { .. }
        | ExecutionEvent::WorkspacePreserved { .. }
        | ExecutionEvent::CleanupStarted { .. }
        | ExecutionEvent::CleanupCompleted { .. }
        | ExecutionEvent::MergeStarted { .. }
        | ExecutionEvent::MergeConflict { .. }
        | ExecutionEvent::ConflictResolutionStarted
        | ExecutionEvent::ConflictResolutionCompleted
        | ExecutionEvent::ConflictResolutionFailed { .. }
        | ExecutionEvent::ChangeSkipped { .. }
        | ExecutionEvent::AnalysisStarted { .. }
        | ExecutionEvent::AnalysisOutput { .. }
        | ExecutionEvent::AnalysisCompleted { .. }
        | ExecutionEvent::ResolveOutput { .. }
        | ExecutionEvent::HookStarted { .. }
        | ExecutionEvent::HookCompleted { .. }
        | ExecutionEvent::HookFailed { .. }
        | ExecutionEvent::Warning { .. }
        | ExecutionEvent::ParallelStartRejected { .. }
        | ExecutionEvent::Log(_)
        | ExecutionEvent::Stopping
        | ExecutionEvent::Stopped
        | ExecutionEvent::AllCompleted
        | ExecutionEvent::Error { .. }
        | ExecutionEvent::WorktreesRefreshed { .. }
        | ExecutionEvent::BranchMergeStarted { .. }
        | ExecutionEvent::BranchMergeCompleted { .. }
        | ExecutionEvent::BranchMergeFailed { .. }
        | ExecutionEvent::ChangeStopFailed { .. } => false,
    }
}

/// Refresh the TUI's reducer-derived display caches after one event.
///
/// The reducer is **read**, never written. By the time an event reaches this
/// frontend its dispatch owner has already applied it, so a write here would be
/// the second transition for one internal event: an apply count that advances
/// twice, a change that leaves the queue twice, a terminal state reached twice.
///
/// Status and blocker view come from the same snapshot so a row's
/// `blocked`/`stalled` word and its blocker kind can never disagree.
async fn sync_reducer_display_caches(
    app: &mut AppState,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    event: &crate::events::ExecutionEvent,
) {
    if !should_apply_event_to_tui_reducer(event) {
        return;
    }
    let (display_map, blocker_views) = {
        let state = shared_state.read().await;
        (state.all_display_statuses(), state.all_blocker_views())
    };
    app.apply_display_statuses_from_reducer(&display_map);
    app.apply_blocker_views_from_reducer(&blocker_views);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalOrchestratorShutdownOutcome {
    NoTask,
    AlreadyFinished,
    Graceful,
    AbortedAfterTimeout,
}

pub(crate) async fn shutdown_local_orchestrator_task(
    orchestrator_handle: Option<tokio::task::JoinHandle<Result<()>>>,
    orchestrator_cancel: Option<CancellationToken>,
    grace_period: Duration,
) -> LocalOrchestratorShutdownOutcome {
    if let Some(cancel) = orchestrator_cancel {
        info!(
            grace_ms = grace_period.as_millis(),
            "Cancelling local TUI orchestrator during shutdown"
        );
        cancel.cancel();
    }

    let Some(handle) = orchestrator_handle else {
        debug!("No local TUI orchestrator task to shut down");
        return LocalOrchestratorShutdownOutcome::NoTask;
    };

    if handle.is_finished() {
        let _ = handle.await;
        info!("Local TUI orchestrator task was already finished during shutdown");
        return LocalOrchestratorShutdownOutcome::AlreadyFinished;
    }

    tokio::pin!(handle);
    tokio::select! {
        join_result = &mut handle => {
            match join_result {
                Ok(Ok(())) => info!("Local TUI orchestrator task finished gracefully during shutdown"),
                Ok(Err(err)) => warn!(error = %err, "Local TUI orchestrator task exited with error during shutdown"),
                Err(err) => warn!(error = %err, "Local TUI orchestrator task join failed during shutdown"),
            }
            LocalOrchestratorShutdownOutcome::Graceful
        }
        _ = tokio::time::sleep(grace_period) => {
            warn!(
                grace_ms = grace_period.as_millis(),
                "Local TUI orchestrator did not finish before shutdown grace period; aborting task to prevent detached local work"
            );
            handle.as_ref().abort_handle().abort();
            match tokio::time::timeout(Duration::from_secs(1), &mut handle).await {
                Ok(Ok(_)) => info!("Aborted local TUI orchestrator task joined after abort"),
                Ok(Err(err)) if err.is_cancelled() => {
                    info!("Local TUI orchestrator task aborted successfully")
                }
                Ok(Err(err)) => {
                    warn!(error = %err, "Local TUI orchestrator task join failed after abort")
                }
                Err(_) => warn!("Timed out while joining aborted local TUI orchestrator task"),
            }
            LocalOrchestratorShutdownOutcome::AbortedAfterTimeout
        }
    }
}

/// Run the local TUI application.
#[allow(clippy::too_many_arguments)]
pub async fn run_tui(
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    lifecycle: LifecycleHandle,
) -> Result<()> {
    // Set up panic hook to restore terminal on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    let mut terminal = ratatui::init();

    // Mouse capture disabled due to terminal compatibility issues
    // Use PageUp/PageDown or k/j keys for scrolling instead
    // execute!(std::io::stdout(), EnableMouseCapture)?;

    let result = run_tui_loop(
        &mut terminal,
        initial_changes,
        config,
        web_url,
        #[cfg(feature = "web-monitoring")]
        web_state,
        post_archive_action,
        upstream_runtime,
        lifecycle,
    )
    .await;

    // Restore terminal state
    restore_terminal();

    result
}

/// Main TUI event loop
#[allow(clippy::too_many_arguments)]
async fn run_tui_loop(
    terminal: &mut DefaultTerminal,
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    lifecycle: LifecycleHandle,
) -> Result<()> {
    let repo_root = std::env::current_dir()?;

    let (committed_change_ids, uncommitted_file_change_ids): (HashSet<String>, HashSet<String>) = {
        let committed_change_ids: HashSet<String> =
            match crate::vcs::git::commands::list_changes_in_head(&repo_root).await {
                Ok(ids) => ids.into_iter().collect(),
                Err(err) => {
                    warn!("Failed to load committed change snapshot: {}", err);
                    initial_changes
                        .iter()
                        .map(|change| change.id.clone())
                        .collect()
                }
            };

        let uncommitted_file_change_ids: HashSet<String> =
            match crate::vcs::git::commands::list_changes_with_uncommitted_files(&repo_root).await {
                Ok(ids) => ids.into_iter().collect(),
                Err(err) => {
                    warn!("Failed to detect uncommitted files in changes: {}", err);
                    HashSet::new()
                }
            };

        (committed_change_ids, uncommitted_file_change_ids)
    };
    let worktree_base_dir = config
        .get_workspace_base_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::config::defaults::default_workspace_base_dir(Some(&repo_root)));
    let worktree_manager = GitWorkspaceManager::new(
        worktree_base_dir.clone(),
        repo_root.clone(),
        config.get_max_concurrent_workspaces(),
        config.clone(),
    );
    let worktree_change_ids: HashSet<String> =
        match worktree_manager.list_worktree_change_ids().await {
            Ok(ids) => ids,
            Err(err) => {
                warn!("Failed to load worktree snapshot: {}", err);
                HashSet::new()
            }
        };

    // Collect initial worktree paths for all changes
    let mut initial_worktree_paths = std::collections::HashMap::new();
    for change in &initial_changes {
        match crate::vcs::git::get_worktree_path_for_change(&repo_root, &change.id).await {
            Ok(Some(wt_path)) => {
                initial_worktree_paths.insert(change.id.clone(), wt_path);
            }
            Ok(None) => {
                // No worktree for this change
            }
            Err(e) => {
                debug!("Failed to get worktree path for {}: {}", change.id, e);
            }
        }
    }

    // Create shared orchestration state for unified tracking across TUI and Web
    let change_ids: Vec<String> = initial_changes.iter().map(|c| c.id.clone()).collect();
    let max_iterations = config.get_max_iterations();
    let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(
        crate::orchestration::state::OrchestratorState::new(change_ids, max_iterations),
    ));

    let tui_config = crate::tui::config::TuiConfig::load_user_config()?;
    let mut app = AppState::new(initial_changes);
    app.set_tui_config(tui_config);
    app.worktree_paths = initial_worktree_paths;
    // Inject shared state reference into TUI for unified tracking
    app.set_shared_state(shared_state.clone());
    let git_dir_exists = crate::cli::check_git_directory();
    let parallel_available = crate::cli::check_parallel_available();
    let mut parallel_mode = config.resolve_parallel_mode(false, git_dir_exists);

    if parallel_mode && !parallel_available {
        parallel_mode = false;
        app.warning_message =
            Some("Parallel mode disabled because git is not available".to_string());
    }
    app.parallel_available = parallel_available;
    app.parallel_mode = parallel_mode;
    app.apply_parallel_eligibility(&committed_change_ids, &uncommitted_file_change_ids);
    app.apply_worktree_status(&worktree_change_ids);
    app.max_concurrent = config.get_max_concurrent_workspaces();
    app.web_url = web_url;

    // Create shared stagger state for all AI commands (worktree, apply, archive, acceptance)
    use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
    let shared_stagger_state: SharedStaggerState = Arc::new(tokio::sync::Mutex::new(None));
    let ai_runner =
        AiCommandRunner::from_orchestrator_config(&config, shared_stagger_state.clone());

    // Two directions, deliberately not one channel.
    //
    // `tx` is the producer side every TUI-local emitter already holds. `rx` is
    // therefore a *producer* boundary: this loop is the dispatch owner for what
    // arrives there, and applies it to the reducer once before rendering it.
    //
    // `frontend_rx` is the delivery side of the orchestration boundary's own
    // dispatch owner. Those events were already applied to the reducer and
    // already projected to `/api/v2`; re-applying them here is exactly the
    // double transition — a doubled apply count, a doubled event sequence —
    // that this frontend must not cause.
    let (tx, mut rx) = mpsc::channel::<OrchestratorEvent>(100);
    let (frontend_tx, mut frontend_rx) = mpsc::channel::<OrchestratorEvent>(100);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<TuiCommand>(100);

    // Inject shared state into WebState if web monitoring is enabled.
    //
    // The execution-mark store and the repository root go with it: without them
    // the v2 snapshot could report neither operator intent nor a redacted
    // change-to-worktree relation, and a remote frontend would have to infer
    // both.
    #[cfg(feature = "web-monitoring")]
    if let Some(ref ws) = web_state {
        ws.set_shared_state(shared_state.clone()).await;
        ws.set_execution_marks(app.execution_marks()).await;
        ws.set_repo_root(repo_root.clone()).await;
    }

    // The other frontends of the TUI-local producer boundary.
    //
    // This frontend is not in the list: the event is already in hand when this
    // loop dispatches it, so adding a `TuiEventSink` here would only route it
    // back into the same loop.
    #[allow(unused_mut)]
    let mut local_event_sinks: Vec<Arc<dyn crate::events::EventSink>> = Vec::new();
    #[cfg(feature = "web-monitoring")]
    if let Some(ref ws) = web_state {
        local_event_sinks.push(Arc::new(crate::web::state::WebEventSink::new(ws.clone())));
    }

    // Dynamic queue for runtime change additions
    let dynamic_queue = DynamicQueue::new();

    // Manual resolve counter for tracking active manual resolves
    // This allows manual resolves to consume parallel execution slots
    let manual_resolve_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Shared flag for graceful stop (signaling orchestrator to stop after current change)
    let graceful_stop_flag = Arc::new(AtomicBool::new(false));

    // The parallel toggle is read by the run supervisor when it spawns, and by
    // the shared start-eligibility guard, so it lives outside `AppState`.
    let parallel_mode_flag = Arc::new(AtomicBool::new(app.parallel_mode));

    // One run supervisor owns the local orchestrator task for this invocation.
    // The TUI adapter and the `/api/v2` adapter drive it through the same
    // run-control service, so neither can start or cancel a run the other
    // cannot see.
    //
    // It gets `frontend_tx`, not `tx`: a spawned run is an orchestration
    // boundary that dispatches its own events, so it must reach this frontend
    // through the delivery side.
    let supervisor = Arc::new(crate::tui::run_supervisor::TuiRunSupervisor::new(
        repo_root.clone(),
        config.clone(),
        frontend_tx.clone(),
        dynamic_queue.clone(),
        shared_state.clone(),
        manual_resolve_counter.clone(),
        post_archive_action.clone(),
        upstream_runtime.clone(),
        graceful_stop_flag.clone(),
        parallel_mode_flag.clone(),
        #[cfg(feature = "web-monitoring")]
        web_state.clone(),
    ));

    // The single process-local application services every frontend commands
    // through. They are built once here, before the first keypress and before v2
    // is bound, so a remote command and a keypress cannot reach different
    // instances of the lifecycle matrix.
    let operator_service = {
        use crate::orchestration::operator_command::{
            HookRunnerQueueHooks, OperatorCommandService,
        };
        let hook_runner = crate::hooks::HookRunner::with_event_tx(
            config.get_hooks(),
            repo_root.clone(),
            tx.clone(),
        );
        Arc::new(OperatorCommandService::new(
            shared_state.clone(),
            Arc::new(dynamic_queue.clone()),
            Arc::new(HookRunnerQueueHooks::new(hook_runner)),
            app.execution_marks(),
        ))
    };
    let start_eligibility = Arc::new(crate::orchestration::run_control::StartEligibility::new());
    start_eligibility.set_parallel_mode(app.parallel_mode);
    start_eligibility.set_parallel_ineligible(
        app.changes
            .iter()
            .filter(|change| !change.is_parallel_eligible)
            .map(|change| change.id.clone()),
    );
    let run_control = Arc::new(crate::orchestration::run_control::RunControlService::new(
        shared_state.clone(),
        operator_service.clone(),
        supervisor.clone(),
        app.resolve_reservations(),
        start_eligibility.clone(),
    ));

    // Bind `/api/v2` command delegation to the shared application services.
    //
    // The web server started before this point (it owns the URL shown in the
    // TUI), so v2 refuses commands until the same services the TUI uses exist.
    // Binding them here is what makes a remote command and a keypress take
    // identical paths through lifecycle validation and side effects.
    #[cfg(feature = "web-monitoring")]
    if let Some(ref ws) = web_state {
        let service = operator_service.clone();
        // The remote worktree port is built once and bound to both halves of v2:
        // the read routes and the command executor must agree about which
        // worktrees exist and which opaque IDs address them.
        let workspace_base_dir = config
            .get_workspace_base_dir()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                crate::config::defaults::default_workspace_base_dir(Some(&repo_root))
            });
        let worktree_service = Arc::new(crate::worktree_ops::service::WorktreeService::new(
            Arc::new(
                crate::worktree_ops::git_backend::GitWorktreeBackend::new(
                    repo_root.clone(),
                    Arc::new(config.clone()),
                )
                .with_hook_events(tx.clone()),
            ),
            Arc::new(crate::worktree_ops::service::NullEventSink),
            workspace_base_dir,
        ));
        let worktree_port: Arc<dyn crate::web::remote_control_api::worktrees::WorktreeOperations> =
            Arc::new(
                crate::web::remote_control_api::worktrees::RemoteWorktreeOperations::new(
                    worktree_service,
                    Arc::new(crate::web::remote_control_api::worktrees::WorktreeRegistry::new()),
                    repo_root.clone(),
                ),
            );

        let runtime = ws.remote_control();
        runtime
            .bind(Arc::new(
                crate::web::remote_control_api::executor::SharedServiceExecutor::new(
                    service,
                    run_control.clone(),
                    ws.clone(),
                    runtime.projection(),
                )
                .with_worktrees(worktree_port.clone()),
            ))
            .await;
        runtime.bind_worktrees(worktree_port).await;
    }

    // Cancellation token for graceful shutdown
    let cancel_token = CancellationToken::new();

    // Start auto-refresh task
    let refresh_tx = tx.clone();
    let refresh_cancel = cancel_token.clone();
    let refresh_repo_root = repo_root.clone();
    let refresh_worktree_base_dir = worktree_base_dir.clone();
    let refresh_config = config.clone();
    let refresh_handle = tokio::spawn(async move {
        let worktree_manager = GitWorkspaceManager::new(
            refresh_worktree_base_dir,
            refresh_repo_root.clone(),
            refresh_config.get_max_concurrent_workspaces(),
            refresh_config,
        );
        let mut stale_refresh_root_warned = false;
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS));
        loop {
            tokio::select! {
                _ = refresh_cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    if should_skip_local_refresh(
                        &refresh_repo_root,
                        &mut stale_refresh_root_warned,
                    ) {
                        continue;
                    }

                    match refresh_local_changes(&refresh_repo_root) {
                        Ok((mut changes, rejected_changes)) => {
                            let committed_change_ids: HashSet<String> =
                                match crate::vcs::git::commands::list_changes_in_head(&refresh_repo_root).await {
                                    Ok(ids) => ids.into_iter().collect(),
                                    Err(err) => {
                                        warn!("Failed to refresh committed change snapshot: {}", err);
                                        changes.iter().map(|change| change.id.clone()).collect()
                                    }
                                };
                            let uncommitted_file_change_ids: HashSet<String> =
                                match crate::vcs::git::commands::list_changes_with_uncommitted_files(&refresh_repo_root).await {
                                    Ok(ids) => ids.into_iter().collect(),
                                    Err(err) => {
                                        warn!("Failed to refresh uncommitted files snapshot: {}", err);
                                        HashSet::new()
                                    }
                                };
                            let worktree_change_ids: HashSet<String> =
                                match worktree_manager.list_worktree_change_ids().await {
                                    Ok(ids) => ids,
                                    Err(err) => {
                                        warn!("Failed to refresh worktree snapshot: {}", err);
                                        HashSet::new()
                                    }
                                };

                            // Collect worktree paths for all changes
                            let mut worktree_paths = std::collections::HashMap::new();

                            // Enrich progress from worktrees (uncommitted tasks.md)
                            for change in &mut changes {
                                match crate::vcs::git::get_worktree_path_for_change(
                                    &refresh_repo_root,
                                    &change.id
                                ).await {
                                    Ok(Some(wt_path)) => {
                                        // Store the worktree path for this change
                                        worktree_paths.insert(change.id.clone(), wt_path.clone());
                                        // Use unified fallback helper: worktree → archive → base
                                        match crate::task_parser::parse_progress_with_fallback(
                                            &change.id,
                                            Some(&wt_path)
                                        ) {
                                            Ok(progress) => {
                                                if progress.total > 0 {
                                                    change.completed_tasks = progress.completed;
                                                    change.total_tasks = progress.total;
                                                } else {
                                                    // Keep existing progress if 0/0
                                                    debug!("Keeping existing progress for {} (parsed: 0/0)", change.id);
                                                }
                                            }
                                            Err(e) => {
                                                debug!("Failed to read progress for {}: {}", change.id, e);
                                                // Keep existing progress (from base tree)
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        // No worktree exists, use progress from base tree
                                    }
                                    Err(e) => {
                                        warn!("Failed to get worktree path for {}: {}", change.id, e);
                                        // Keep existing progress
                                    }
                                }
                            }

                            // Check which worktrees are not ahead of base (for MergeWait auto-clear)
                            let mut worktree_not_ahead_ids = std::collections::HashSet::new();
                            // Check which worktrees are archived but not merged (for MergeWait restoration)
                            let mut merge_wait_ids = std::collections::HashSet::new();

                            // Get base branch (current branch in main repo)
                            if let Ok(Some(base_branch)) = crate::vcs::git::commands::get_current_branch(&refresh_repo_root).await {
                                // For each change with a worktree, check if worktree branch is ahead of base
                                for (change_id, wt_path) in &worktree_paths {
                                    // Get the branch name for this worktree
                                    if let Ok(Some(worktree_branch)) = crate::vcs::git::commands::get_current_branch(wt_path).await {
                                        // Count commits ahead
                                        match crate::vcs::git::commands::count_commits_ahead(
                                            &refresh_repo_root,
                                            &base_branch,
                                            &worktree_branch
                                        ).await {
                                            Ok(0) => {
                                                // Worktree is not ahead (0 commits), mark for auto-clear
                                                worktree_not_ahead_ids.insert(change_id.clone());
                                            }
                                            Ok(_) => {
                                                // Worktree is ahead, keep MergeWait if present
                                            }
                                            Err(e) => {
                                                debug!("Failed to count commits ahead for {}: {}", change_id, e);
                                                // On error, don't auto-clear (safe default)
                                            }
                                        }
                                    }

                                    // Detect WorkspaceState::Archived for MergeWait restoration
                                    match crate::execution::state::detect_workspace_state(change_id, wt_path, &base_branch).await {
                                        Ok(crate::execution::state::WorkspaceState::Archived) => {
                                            // Worktree is archived but not merged, restore MergeWait
                                            merge_wait_ids.insert(change_id.clone());
                                            debug!("Detected MergeWait for '{}': archive complete, waiting for merge", change_id);
                                        }
                                        Ok(_) => {
                                            // Other states, do nothing
                                        }
                                        Err(e) => {
                                            debug!("Failed to detect workspace state for {}: {}", change_id, e);
                                            // On error, skip detection (safe default)
                                        }
                                    }
                                }
                            }

                            if refresh_tx
                                .send(OrchestratorEvent::ChangesRefreshed {
                                    changes,
                                    rejected_changes,
                                    committed_change_ids,
                                    uncommitted_file_change_ids,
                                    worktree_change_ids,
                                    worktree_paths,
                                    worktree_not_ahead_ids,
                                    merge_wait_ids,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = refresh_tx
                                .send(OrchestratorEvent::Log(LogEntry::error(format!(
                                    "Refresh failed: {}",
                                    e
                                ))))
                                .await;
                        }
                    }

                    // Refresh worktrees with conflict check (if in Worktrees view)
                    // We do this in the background without blocking
                    let wt_refresh_tx = refresh_tx.clone();
                    let wt_refresh_repo_root = refresh_repo_root.clone();
                    tokio::spawn(async move {
                        match load_worktrees_with_conflict_check(&wt_refresh_repo_root).await {
                            Ok(worktrees) => {
                                let _ = wt_refresh_tx
                                    .send(OrchestratorEvent::WorktreesRefreshed { worktrees })
                                    .await;
                            }
                            Err(e) => {
                                debug!("Failed to refresh worktrees: {}", e);
                                // Don't spam logs on refresh failures
                            }
                        }
                    });

                    log_deduplicator::maybe_log_summary();
                }
            }
        }
    });

    // External lifecycle reporting is derived from typed TUI state, never from
    // rendered screen contents. Unchanged states are deduplicated by the
    // dispatcher, so publishing once per frame is cheap and non-blocking.
    let lifecycle_workspace = repo_root.display().to_string();
    let publish_lifecycle_state = |app: &AppState| {
        if !lifecycle.is_enabled() {
            return;
        }
        let snapshot = TuiLifecycleSnapshot::from_app(app);
        lifecycle.publish_state(
            snapshot.lifecycle_state(),
            snapshot.lifecycle_context(&lifecycle_workspace),
        );
    };

    publish_lifecycle_state(&app);

    loop {
        // Increment spinner frame for animation (updates every 100ms)
        app.spinner_frame = (app.spinner_frame + 1) % SPINNER_CHARS.len();

        // Draw the UI
        terminal.draw(|frame| render(frame, &mut app))?;

        // Handle events with timeout
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Create context for key event handling
                    let mut key_ctx = KeyEventContext {
                        app: &mut app,
                        terminal,
                        repo_root: &repo_root,
                        config: &config,
                        worktree_base_dir: &worktree_base_dir,
                        tx: &tx,
                        cmd_tx: &cmd_tx,
                        ai_runner: &ai_runner,
                        supervisor: &supervisor,
                    };

                    // Handle key event using helper
                    match handle_key_event(key, &mut key_ctx).await {
                        Ok(Some(cmd)) => {
                            // Send command to command channel for processing
                            let _ = cmd_tx.send(cmd).await;
                        }
                        Ok(None) => {
                            // No command to execute
                        }
                        Err(e) => {
                            app.add_log(LogEntry::error(format!("Key handling error: {}", e)));
                        }
                    }

                    // Check if app should quit (set by Ctrl+C)
                    if app.should_quit {
                        break;
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            // Scroll logs up (show older entries) - 3 lines at a time
                            app.scroll_logs_up(3);
                        }
                        MouseEventKind::ScrollDown => {
                            // Scroll logs down (show newer entries) - 3 lines at a time
                            app.scroll_logs_down(3);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Events raised by TUI-local producers (key handlers, hooks, worktree
        // operations, the auto-refresh). This loop is their dispatch owner: one
        // reducer transition, one delivery to every other frontend, then this
        // frontend's own state — not a second reducer write per frontend and not
        // a hand-picked subset forwarded to the web projection.
        while let Ok(event) = rx.try_recv() {
            crate::events::dispatch_event(&shared_state, &local_event_sinks, event.clone()).await;
            sync_reducer_display_caches(&mut app, &shared_state, &event).await;

            // Painting only: an orchestrator event never produces a command. The
            // scheduler dispatches every transition it implies from the
            // reducer-owned intent the same event already recorded.
            app.handle_orchestrator_event(event);
        }

        // Authoritative deliveries from the orchestration boundary's dispatch
        // owner. The reducer transition and the `/api/v2` projection already
        // happened; this frontend only reads the result and renders it.
        while let Ok(event) = frontend_rx.try_recv() {
            sync_reducer_display_caches(&mut app, &shared_state, &event).await;

            // Painting only, for the same reason as the producer loop above: the
            // scheduler dispatches every transition the event implies from the
            // reducer-owned intent it already recorded.
            app.handle_orchestrator_event(event);
        }

        // Handle dynamic queue additions and removals
        while let Ok(cmd) = cmd_rx.try_recv() {
            // Create context for TuiCommand handling
            let mut cmd_ctx = TuiCommandContext {
                app: &mut app,
                repo_root: &repo_root,
                config: &config,
                tx: &tx,
                run_control: &run_control,
                #[cfg(feature = "web-monitoring")]
                web_state: &web_state,
            };

            // Handle TuiCommand using helper
            if let Err(e) = handle_tui_command(cmd, &mut cmd_ctx, &shared_state).await {
                app.add_log(LogEntry::error(format!("Command handling error: {}", e)));
            }
        }

        // The parallel toggle and its eligibility set are read by the shared
        // start guard, so republish them once per frame instead of at every
        // place the TUI can change them.
        parallel_mode_flag.store(app.parallel_mode, std::sync::atomic::Ordering::SeqCst);
        start_eligibility.set_parallel_mode(app.parallel_mode);
        start_eligibility.set_parallel_ineligible(
            app.changes
                .iter()
                .filter(|change| !change.is_parallel_eligible)
                .map(|change| change.id.clone()),
        );

        publish_lifecycle_state(&app);

        if app.should_quit {
            break;
        }
    }

    // Cleanup: cancel all TUI-scoped tasks and force-stop local orchestration launched by this TUI.
    cancel_token.cancel();

    // Wait for tasks to finish gracefully. Remote mode has no local orchestrator handle here;
    // remote server-side work is stopped only by explicit Stop/ForceStop commands.
    refresh_handle.abort();
    let (orchestrator_handle, orchestrator_cancel) = supervisor.take_run();
    let _ = shutdown_local_orchestrator_task(
        orchestrator_handle,
        orchestrator_cancel,
        Duration::from_secs(5),
    )
    .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        is_refresh_root_usable, refresh_local_changes, should_apply_event_to_tui_reducer,
        shutdown_local_orchestrator_task, LocalOrchestratorShutdownOutcome,
    };
    use super::{AppState, OrchestratorEvent};
    use crate::events::{ExecutionEvent, RejectionOutcome, StalledBlocker};
    use crate::openspec::{Change, ProposalMetadata};
    use crate::vcs::WorkspaceStatus;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn sample_change() -> Change {
        Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn empty_changes_refreshed_event() -> ExecutionEvent {
        ExecutionEvent::ChangesRefreshed {
            changes: vec![sample_change()],
            rejected_changes: Vec::new(),
            committed_change_ids: HashSet::new(),
            uncommitted_file_change_ids: HashSet::new(),
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::<String, PathBuf>::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
        }
    }

    fn stalled_blocker() -> StalledBlocker {
        StalledBlocker::acceptance_external(
            "pending_verification",
            "managed verification job still running",
        )
    }

    /// A boundary run's events reach this frontend already applied.
    ///
    /// The regression this pins: the loop used to write the reducer for every
    /// event it received, including the ones the orchestration boundary had
    /// already applied. One `ApplyCompleted` then advanced the apply count
    /// twice — once per frontend path — which is exactly what an authoritative
    /// remote snapshot cannot survive.
    #[tokio::test]
    async fn frontend_delivery_does_not_reapply_the_reducer() {
        use crate::events::EventSink;
        use crate::orchestration::state::OrchestratorState;
        use crate::tui::events::TuiEventSink;

        let shared_state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let (frontend_tx, mut frontend_rx) = mpsc::channel::<OrchestratorEvent>(16);
        let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(TuiEventSink::new(frontend_tx))];

        for event in [
            ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
            ExecutionEvent::ApplyCompleted {
                change_id: "change-a".to_string(),
                revision: String::new(),
            },
        ] {
            crate::events::dispatch_event(&shared_state, &sinks, event).await;
        }
        assert_eq!(shared_state.read().await.apply_count("change-a"), 1);

        let mut app = AppState::new(vec![sample_change()]);
        app.set_shared_state(shared_state.clone());
        while let Ok(event) = frontend_rx.try_recv() {
            super::sync_reducer_display_caches(&mut app, &shared_state, &event).await;
        }

        assert_eq!(
            shared_state.read().await.apply_count("change-a"),
            1,
            "the frontend applied a delivered event a second time"
        );
    }

    /// A TUI-local producer's event is applied once, by this loop, and reaches
    /// the other frontends through the same authoritative dispatch.
    #[cfg(feature = "web-monitoring")]
    #[tokio::test]
    async fn tui_local_producer_events_are_dispatched_once_to_every_frontend() {
        use crate::events::EventSink;
        use crate::orchestration::state::OrchestratorState;
        use crate::web::state::{WebEventSink, WebState};

        let shared_state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(shared_state.clone()).await;
        let projection = web_state.remote_control().projection();
        let local_sinks: Vec<Arc<dyn EventSink>> =
            vec![Arc::new(WebEventSink::new(web_state.clone()))];

        let event = ExecutionEvent::ApplyCompleted {
            change_id: "change-a".to_string(),
            revision: String::new(),
        };
        crate::events::dispatch_event(&shared_state, &local_sinks, event.clone()).await;

        let mut app = AppState::new(vec![sample_change()]);
        app.set_shared_state(shared_state.clone());
        super::sync_reducer_display_caches(&mut app, &shared_state, &event).await;

        assert_eq!(
            shared_state.read().await.apply_count("change-a"),
            1,
            "a locally produced event must be applied exactly once"
        );
        let (_, _, sequence) = projection.snapshot();
        assert_eq!(
            sequence, 1,
            "a locally produced event must reach the v2 stream exactly once"
        );
    }

    #[test]
    fn tui_reducer_sync_includes_running_lifecycle_display_events() {
        let reducer_visible_events = vec![
            ExecutionEvent::ProcessingStarted("change-a".to_string()),
            ExecutionEvent::ProcessingCompleted("change-a".to_string()),
            ExecutionEvent::ProcessingError {
                id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
            ExecutionEvent::ApplyCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-a".to_string(),
            },
            ExecutionEvent::ApplyFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::AcceptanceStarted {
                change_id: "change-a".to_string(),
                command: "accept".to_string(),
            },
            ExecutionEvent::AcceptanceCompleted {
                change_id: "change-a".to_string(),
            },
            ExecutionEvent::AcceptanceFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::ArchiveStarted {
                change_id: "change-a".to_string(),
                command: "archive".to_string(),
            },
            ExecutionEvent::ArchiveResumed {
                change_id: "change-a".to_string(),
                reason: Some("resume".to_string()),
                summary: Some("resume archive".to_string()),
            },
            ExecutionEvent::ArchiveRetryScheduled {
                change_id: "change-a".to_string(),
                attempt: 1,
                max_attempts: 2,
                reason: Some("retry".to_string()),
                summary: Some("retry archive".to_string()),
            },
            ExecutionEvent::ChangeArchived("change-a".to_string()),
            ExecutionEvent::ArchiveFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
                reason: Some("failed".to_string()),
                summary: Some("archive failed".to_string()),
            },
            ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "dirty base".to_string(),
                auto_resumable: true,
            },
            ExecutionEvent::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-a".to_string(),
            },
            ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve".to_string(),
            },
            ExecutionEvent::ResolveCompleted {
                change_id: "change-a".to_string(),
                worktree_change_ids: None,
            },
            ExecutionEvent::ResolveFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws-a".to_string(),
                status: WorkspaceStatus::Applying,
            },
            ExecutionEvent::RejectionReviewCompleted {
                change_id: "change-a".to_string(),
                outcome: RejectionOutcome::Resume,
            },
            ExecutionEvent::RejectionReviewFailed {
                change_id: "change-a".to_string(),
                error: "boom".to_string(),
            },
            ExecutionEvent::DependencyBlocked {
                change_id: "change-a".to_string(),
                dependency_ids: vec!["dep".to_string()],
            },
            ExecutionEvent::DependencyResolved {
                change_id: "change-a".to_string(),
            },
            ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: stalled_blocker(),
            },
            ExecutionEvent::ExecutionBlocked {
                change_id: "change-a".to_string(),
                blocker: stalled_blocker(),
            },
            ExecutionEvent::ChangeDequeued {
                change_id: "change-a".to_string(),
            },
            ExecutionEvent::ChangeStopped {
                change_id: "change-a".to_string(),
            },
            empty_changes_refreshed_event(),
        ];

        for event in reducer_visible_events {
            assert!(
                should_apply_event_to_tui_reducer(&event),
                "event should sync to TUI reducer before display snapshot: {event:?}"
            );
        }
    }

    #[test]
    fn tui_reducer_sync_excludes_presentation_only_events() {
        let presentation_events = vec![
            ExecutionEvent::ApplyOutput {
                change_id: "change-a".to_string(),
                output: "chunk".to_string(),
                iteration: Some(1),
            },
            ExecutionEvent::ProgressUpdated {
                change_id: "change-a".to_string(),
                completed: 1,
                total: 2,
            },
            ExecutionEvent::Log(crate::events::LogEntry::info("hello")),
            ExecutionEvent::WorktreesRefreshed { worktrees: vec![] },
        ];

        for event in presentation_events {
            assert!(
                !should_apply_event_to_tui_reducer(&event),
                "presentation-only event should not sync to TUI reducer: {event:?}"
            );
        }
    }

    #[test]
    fn refresh_root_usable_for_existing_directory() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        assert!(is_refresh_root_usable(temp_dir.path()));
    }

    #[test]
    fn refresh_root_not_usable_for_missing_directory() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let missing_path = temp_dir.path().join("missing-root");
        assert!(!is_refresh_root_usable(&missing_path));
    }

    #[test]
    fn stale_refresh_root_sets_warned_once_and_resets_when_root_recovers() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let missing_path = temp_dir.path().join("missing-root");
        let mut warned = false;

        assert!(super::should_skip_local_refresh(&missing_path, &mut warned));
        assert!(warned, "first stale root check should set warned flag");

        assert!(super::should_skip_local_refresh(&missing_path, &mut warned));
        assert!(warned, "second stale root check keeps warned flag set");

        assert!(!super::should_skip_local_refresh(
            temp_dir.path(),
            &mut warned
        ));
        assert!(!warned, "usable root should reset warned flag");
    }

    #[test]
    fn local_refresh_not_skipped_for_existing_root() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let mut warned = false;

        assert!(!super::should_skip_local_refresh(
            temp_dir.path(),
            &mut warned
        ));
        assert!(
            !warned,
            "existing root should not trigger stale warning suppression"
        );
    }

    #[test]
    fn refresh_local_changes_uses_explicit_repo_root_for_active_and_rejected_rows() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let repo_dir = tempfile::tempdir().expect("repo tempdir");
        let other_dir = tempfile::tempdir().expect("cwd tempdir");
        let changes_dir = repo_dir.path().join("openspec").join("changes");

        let active_dir = changes_dir.join("change-active");
        std::fs::create_dir_all(&active_dir).expect("active dir");
        std::fs::write(active_dir.join("proposal.md"), "# proposal").expect("active proposal");
        std::fs::write(active_dir.join("tasks.md"), "- [ ] task").expect("active tasks");

        let rejected_dir = changes_dir.join("change-rejected");
        std::fs::create_dir_all(&rejected_dir).expect("rejected dir");
        std::fs::write(rejected_dir.join("proposal.md"), "# proposal").expect("rejected proposal");
        std::fs::write(rejected_dir.join("tasks.md"), "- [ ] task").expect("rejected tasks");
        std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED").expect("rejected marker");

        let original_dir = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(other_dir.path()).expect("set cwd elsewhere");

        let (active, rejected) = refresh_local_changes(repo_dir.path()).expect("refresh succeeds");

        std::env::set_current_dir(original_dir).expect("restore cwd");

        assert_eq!(
            active.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["change-active"]
        );
        assert_eq!(
            rejected.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["change-rejected"]
        );
    }

    #[tokio::test]
    async fn shutdown_local_orchestrator_cancels_and_aborts_non_finishing_task() {
        let token = CancellationToken::new();
        let task_token = token.clone();
        let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel();
        let (post_abort_tx, mut post_abort_rx) = tokio::sync::mpsc::channel::<()>(1);

        let handle = tokio::spawn(async move {
            task_token.cancelled().await;
            let _ = cancelled_tx.send(());
            tokio::time::sleep(Duration::from_secs(60)).await;
            loop {
                let _ = post_abort_tx.send(()).await;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            #[allow(unreachable_code)]
            Ok(())
        });

        let outcome =
            shutdown_local_orchestrator_task(Some(handle), Some(token), Duration::from_millis(10))
                .await;

        assert_eq!(
            outcome,
            LocalOrchestratorShutdownOutcome::AbortedAfterTimeout
        );
        assert!(
            cancelled_rx.await.is_ok(),
            "shutdown should cancel orchestrator token"
        );
        tokio::task::yield_now().await;
        let post_cleanup_event =
            tokio::time::timeout(Duration::from_millis(80), post_abort_rx.recv()).await;
        assert!(
            !matches!(post_cleanup_event, Ok(Some(()))),
            "aborted local orchestrator must not keep sending events after cleanup"
        );
    }

    #[tokio::test]
    async fn shutdown_local_orchestrator_without_handle_is_noop() {
        let outcome = shutdown_local_orchestrator_task(None, None, Duration::from_millis(1)).await;

        assert_eq!(outcome, LocalOrchestratorShutdownOutcome::NoTask);
    }
}
