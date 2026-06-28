//! TUI runner and main event loop
//!
//! Contains run_tui and run_tui_loop functions.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
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
use super::log_deduplicator;
// orchestrator functions now called from command_handlers
use super::queue::DynamicQueue;
use super::render::{render, SPINNER_CHARS};
use super::state::{AppState, AUTO_REFRESH_INTERVAL_SECS};
// AppMode/StopMode are used in handlers
use super::terminal::restore_terminal;
use super::worktrees::load_worktrees_with_conflict_check;

/// Run the TUI application (local mode only, no remote client).
///
/// This is a convenience wrapper around [`run_tui_with_remote`] for callers that
/// do not need remote server connectivity.
#[allow(dead_code)]
pub async fn run_tui(
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
) -> Result<()> {
    run_tui_with_remote(
        initial_changes,
        config,
        web_url,
        #[cfg(feature = "web-monitoring")]
        web_state,
        None,
    )
    .await
}

/// Run the TUI application with an optional remote client.
///
/// When `remote_client` is `Some`, a background task subscribes to the WebSocket
/// endpoint of the remote server and forwards state updates into the TUI event channel.
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

fn should_bypass_local_refresh(is_remote_mode: bool) -> bool {
    is_remote_mode
}

fn refresh_local_changes(repo_root: &Path) -> Result<(Vec<Change>, Vec<Change>)> {
    let active_changes = crate::openspec::list_changes_native_from(repo_root)?;
    let rejected_changes = crate::openspec::list_rejected_changes_native_from(repo_root)?;
    Ok((active_changes, rejected_changes))
}

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
        | ExecutionEvent::ChangeStopFailed { .. }
        | ExecutionEvent::RemoteChangeUpdate { .. } => false,
    }
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

pub async fn run_tui_with_remote(
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
    remote_client: Option<crate::remote::RemoteClient>,
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
        remote_client,
    )
    .await;

    // Restore terminal state
    restore_terminal();

    result
}

/// Main TUI event loop
async fn run_tui_loop(
    terminal: &mut DefaultTerminal,
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
    remote_client: Option<crate::remote::RemoteClient>,
) -> Result<()> {
    let repo_root = std::env::current_dir()?;

    // Parallel eligibility is a local-worktree concept.
    // In remote server mode, avoid incorrectly marking all remote changes as
    // "uncommitted" based on the local repository.
    let (committed_change_ids, uncommitted_file_change_ids): (HashSet<String>, HashSet<String>) =
        if remote_client.is_some() {
            (
                initial_changes
                    .iter()
                    .map(|change| change.id.clone())
                    .collect(),
                HashSet::new(),
            )
        } else {
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
                match crate::vcs::git::commands::list_changes_with_uncommitted_files(&repo_root)
                    .await
                {
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
    let mut parallel_available = crate::cli::check_parallel_available();
    let mut parallel_mode = config.resolve_parallel_mode(false, git_dir_exists);

    if remote_client.is_some() {
        parallel_available = false;
        parallel_mode = false;
    }
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
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    let shared_stagger_state: SharedStaggerState = Arc::new(tokio::sync::Mutex::new(None));
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
    let stream_json_textify = config.get_stream_json_textify();
    let mut ai_runner = AiCommandRunner::new(queue_config.clone(), shared_stagger_state.clone());
    ai_runner.set_stream_json_textify(stream_json_textify);
    ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());
    ai_runner.set_command_envs(config.get_command_envs());

    let (tx, mut rx) = mpsc::channel::<OrchestratorEvent>(100);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<TuiCommand>(100);

    // Inject shared state into WebState if web monitoring is enabled
    #[cfg(feature = "web-monitoring")]
    if let Some(ref ws) = web_state {
        ws.set_shared_state(shared_state.clone()).await;
    }

    // Dynamic queue for runtime change additions
    let dynamic_queue = DynamicQueue::new();

    // Manual resolve counter for tracking active manual resolves
    // This allows manual resolves to consume parallel execution slots
    let manual_resolve_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Cancellation token for graceful shutdown
    let cancel_token = CancellationToken::new();

    // Wire web control channel to TUI command channel
    #[cfg(feature = "web-monitoring")]
    if let Some(ref ws) = web_state {
        // Create unbounded channel for web control commands
        let (control_tx, mut control_rx) =
            mpsc::unbounded_channel::<crate::web::state::ControlCommand>();

        // Set the control channel in WebState
        ws.set_control_channel(control_tx).await;

        // Spawn bridge task to translate ControlCommand -> TuiCommand
        let bridge_cmd_tx = cmd_tx.clone();
        let bridge_cancel = cancel_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = bridge_cancel.cancelled() => {
                        break;
                    }
                    Some(control_cmd) = control_rx.recv() => {
                        use crate::web::state::ControlCommand;

                        // For Start command, we need a special marker that will be handled
                        // in the main loop to call app.start_processing()/resume_processing()/retry_error_changes()
                        // For other commands, we can directly translate to TuiCommand
                        let tui_cmd_opt = match control_cmd {
                            ControlCommand::Start => {
                                // Send a special StartProcessing with empty vec as a signal
                                // The main loop will need to handle this by calling the appropriate method
                                Some(TuiCommand::StartProcessing(vec![]))
                            }
                            ControlCommand::Stop => Some(TuiCommand::Stop),
                            ControlCommand::CancelStop => Some(TuiCommand::CancelStop),
                            ControlCommand::ForceStop => Some(TuiCommand::ForceStop),
                            ControlCommand::Retry => Some(TuiCommand::Retry),
                        };

                        if let Some(tui_cmd) = tui_cmd_opt {
                            if bridge_cmd_tx.send(tui_cmd).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    // Keep a clone for user-triggered actions (F5 run/stop/retry).
    let remote_client_actions = remote_client.clone();

    // Start remote WebSocket subscription task (remote mode only)
    let _ws_handle: Option<tokio::task::JoinHandle<()>> = if let Some(client) = remote_client {
        let ws_url = client.ws_url();
        let ws_token = client.token().map(str::to_owned);
        let ws_tx = tx.clone();
        let ws_cancel = cancel_token.clone();

        info!("Starting remote WebSocket subscriber: {}", ws_url);

        // Channel for WS messages
        let (ws_msg_tx, mut ws_msg_rx) =
            tokio::sync::mpsc::channel::<crate::remote::RemoteStateUpdate>(64);

        // Spawn the WS connection task
        let ws_task = tokio::spawn(async move {
            loop {
                // Try to connect; on failure, wait and retry
                match crate::remote::ws::connect_and_subscribe(
                    ws_url.clone(),
                    ws_token.as_deref(),
                    ws_msg_tx.clone(),
                )
                .await
                {
                    Ok(recv_handle) => {
                        // Keep an abort handle so we can cancel while also awaiting
                        let abort_handle = recv_handle.abort_handle();
                        // Wait until the connection task finishes or cancel is requested
                        tokio::select! {
                            _ = ws_cancel.cancelled() => {
                                abort_handle.abort();
                                break;
                            }
                            result = recv_handle => {
                                let _ = result; // ignore JoinError
                                warn!("WS connection dropped, will reconnect in 5s");
                            }
                        }
                    }
                    Err(e) => {
                        warn!("WS connect failed: {}, retrying in 5s", e);
                    }
                }

                // Wait before reconnecting (check cancel every second)
                for _ in 0..5u32 {
                    if ws_cancel.is_cancelled() {
                        return;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        });

        // Spawn a translator task: RemoteStateUpdate -> OrchestratorEvent
        // Maintains a mapping from project.id -> project.name so that ChangeUpdate
        // incremental messages use the same "<project.name>/<change.id>" format as
        // the initial FullState snapshot loaded by group_changes_by_project().
        let translate_tx = ws_tx;
        tokio::spawn(async move {
            // project_id -> project_name mapping, populated from FullState messages
            let mut project_name_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            while let Some(update) = ws_msg_rx.recv().await {
                use crate::remote::types::RemoteStateUpdate;
                match update {
                    RemoteStateUpdate::FullState { projects, .. } => {
                        // Update the project id->name mapping
                        project_name_map.clear();
                        for proj in &projects {
                            project_name_map.insert(proj.id.clone(), proj.name.clone());
                        }

                        let changes = crate::remote::group_changes_by_project(&projects);
                        // Full state snapshot → send as ChangesRefreshed (replaces the full list)
                        let _ = translate_tx
                            .send(super::events::OrchestratorEvent::ChangesRefreshed {
                                changes,
                                rejected_changes: Vec::new(),
                                committed_change_ids: std::collections::HashSet::new(),
                                uncommitted_file_change_ids: std::collections::HashSet::new(),
                                worktree_change_ids: std::collections::HashSet::new(),
                                worktree_paths: std::collections::HashMap::new(),
                                worktree_not_ahead_ids: std::collections::HashSet::new(),
                                merge_wait_ids: std::collections::HashSet::new(),
                            })
                            .await;

                        // Also send per-change status updates so the TUI can render
                        // in-flight states (Applying/Archiving/etc) in remote mode.
                        for proj in &projects {
                            let project_display = proj.name.clone();
                            for ch in &proj.changes {
                                let id = format!("{}::{}/{}", proj.id, project_display, ch.id);
                                let _ = translate_tx
                                    .send(super::events::OrchestratorEvent::RemoteChangeUpdate {
                                        id,
                                        completed_tasks: ch.completed_tasks,
                                        total_tasks: ch.total_tasks,
                                        status: Some(ch.status.clone()),
                                        iteration_number: ch.iteration_number,
                                    })
                                    .await;
                            }
                        }
                    }
                    RemoteStateUpdate::ChangeUpdate { change } => {
                        // Incremental update → send as RemoteChangeUpdate (applies non-regression rule)
                        // Use project.name (from the id->name map) to match the format used by
                        // group_changes_by_project(): "<project_id>::<project.name>/<change.id>"
                        let project_display = project_name_map
                            .get(&change.project)
                            .cloned()
                            .unwrap_or_else(|| change.project.clone());
                        let id = format!("{}::{}/{}", change.project, project_display, change.id);
                        let _ = translate_tx
                            .send(super::events::OrchestratorEvent::RemoteChangeUpdate {
                                id,
                                completed_tasks: change.completed_tasks,
                                total_tasks: change.total_tasks,
                                status: Some(change.status),
                                iteration_number: change.iteration_number,
                            })
                            .await;
                    }
                    RemoteStateUpdate::Log { entry } => {
                        // Convert remote log entry to a TUI log event
                        use crate::tui::events::LogLevel;
                        let level = match entry.level.as_str() {
                            "error" => LogLevel::Error,
                            "warn" | "warning" => LogLevel::Warn,
                            "success" => LogLevel::Success,
                            _ => LogLevel::Info,
                        };
                        // Normalize change_id for remote log association:
                        // When change_id is None but project_id is set, use project_id as
                        // the change_id so that get_latest_log_for_change() can match logs
                        // to changes via project_id prefix matching.
                        let effective_change_id =
                            entry.change_id.or_else(|| entry.project_id.clone());
                        let log_entry = crate::tui::events::LogEntry {
                            timestamp: entry.timestamp.clone(),
                            created_at: chrono::Utc::now(),
                            message: entry.message,
                            color: ratatui::style::Color::Reset,
                            level,
                            change_id: effective_change_id,
                            operation: entry.operation,
                            iteration: entry.iteration,
                            workspace_path: None,
                        };
                        let _ = translate_tx
                            .send(super::events::OrchestratorEvent::Log(log_entry))
                            .await;
                    }
                    RemoteStateUpdate::ChangeRemoved { .. } | RemoteStateUpdate::Ping => {
                        // Ping is a no-op; ChangeRemoved would require a separate event type (future work)
                    }
                }
            }
        });

        Some(ws_task)
    } else {
        None
    };

    // In remote mode, the auto-refresh task must NOT call list_changes_native()
    // because local openspec/ changes are irrelevant when connected to a remote server.
    // State updates arrive exclusively via the WebSocket subscription.
    let is_remote_mode = _ws_handle.is_some();

    // Start auto-refresh task
    let refresh_tx = tx.clone();
    let refresh_cancel = cancel_token.clone();
    let refresh_repo_root = repo_root.clone();
    let refresh_worktree_base_dir = worktree_base_dir.clone();
    let refresh_config = config.clone();
    let refresh_handle = tokio::spawn(async move {
        // Skip local refresh entirely in remote mode; WS task handles updates.
        if should_bypass_local_refresh(is_remote_mode) {
            return;
        }

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

    // Orchestrator task (spawned when processing starts)
    let mut orchestrator_handle: Option<tokio::task::JoinHandle<Result<()>>> = None;
    let mut orchestrator_cancel: Option<CancellationToken> = None;

    // Shared flag for graceful stop (signaling orchestrator to stop after current change)
    let graceful_stop_flag = Arc::new(AtomicBool::new(false));

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
                        graceful_stop_flag: &graceful_stop_flag,
                        orchestrator_cancel: &orchestrator_cancel,
                        orchestrator_handle: &orchestrator_handle,
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

        // Handle orchestrator events
        while let Ok(event) = rx.try_recv() {
            // Apply reducer-visible events to shared state before syncing display caches.
            //
            // Phase 5.1: workspace observations drive the shared reducer (ChangesRefreshed).
            // Phase 5.2: lifecycle events must also update shared reducer so active, wait,
            //   terminal, and queue states cannot be regressed by the next display snapshot.
            // Phase 6.1: TUI derives queue_status from the reducer display snapshot.
            if should_apply_event_to_tui_reducer(&event) {
                let display_map = {
                    let mut state = shared_state.write().await;
                    state.apply_execution_event(&event);
                    state.all_display_statuses()
                };
                app.apply_display_statuses_from_reducer(&display_map);
            }

            // Forward execution events to web state (web-monitoring feature only)
            #[cfg(feature = "web-monitoring")]
            if let Some(ref web_state) = web_state {
                use crate::events::ExecutionEvent;
                match &event {
                    // Changes refreshed - use update method to preserve state
                    ExecutionEvent::ChangesRefreshed { changes, .. } => {
                        web_state.update(changes).await;
                    }
                    // Execution lifecycle events - forward to apply_execution_event
                    ExecutionEvent::ProcessingStarted(_)
                    | ExecutionEvent::ProcessingError { .. }
                    | ExecutionEvent::ChangeArchived(_)
                    | ExecutionEvent::MergeCompleted { .. }
                    | ExecutionEvent::ResolveStarted { .. }
                    | ExecutionEvent::ResolveCompleted { .. }
                    | ExecutionEvent::ResolveFailed { .. }
                    | ExecutionEvent::MergeDeferred { .. }
                    | ExecutionEvent::WorkspaceStatusUpdated { .. }
                    | ExecutionEvent::RejectionReviewCompleted { .. }
                    | ExecutionEvent::RejectionReviewFailed { .. }
                    | ExecutionEvent::Stopping
                    | ExecutionEvent::Stopped
                    | ExecutionEvent::AllCompleted => {
                        web_state.apply_execution_event(&event).await;
                    }
                    _ => {
                        // Other events are not needed for web state updates
                    }
                }
            }

            if let Some(cmd) = app.handle_orchestrator_event(event) {
                // Event triggered a command (e.g., auto-start next resolve)
                let _ = cmd_tx.send(cmd).await;
            }
        }

        // Handle dynamic queue additions and removals
        while let Ok(cmd) = cmd_rx.try_recv() {
            // Create context for TuiCommand handling
            let mut cmd_ctx = TuiCommandContext {
                app: &mut app,
                repo_root: &repo_root,
                config: &config,
                tx: &tx,
                dynamic_queue: &dynamic_queue,
                remote_client: remote_client_actions.clone(),
                orchestrator_running: orchestrator_handle
                    .as_ref()
                    .is_some_and(|handle| !handle.is_finished()),
                #[cfg(feature = "web-monitoring")]
                web_state: &web_state,
            };

            // Handle TuiCommand using helper
            match handle_tui_command(
                cmd,
                &mut cmd_ctx,
                &graceful_stop_flag,
                &shared_state,
                &manual_resolve_counter,
                &mut orchestrator_cancel,
            )
            .await
            {
                Ok(Some(handle)) => {
                    orchestrator_handle = Some(handle);
                }
                Ok(None) => {
                    // Command processed without starting orchestrator
                }
                Err(e) => {
                    app.add_log(LogEntry::error(format!("Command handling error: {}", e)));
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup: cancel all TUI-scoped tasks and force-stop local orchestration launched by this TUI.
    cancel_token.cancel();

    // Wait for tasks to finish gracefully. Remote mode has no local orchestrator handle here;
    // remote server-side work is stopped only by explicit Stop/ForceStop commands.
    refresh_handle.abort();
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
    use crate::events::{ExecutionEvent, RejectionOutcome, StalledBlocker};
    use crate::openspec::{Change, ProposalMetadata};
    use crate::vcs::WorkspaceStatus;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::time::Duration;
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
        StalledBlocker::acceptance_infrastructure("managed verification job still running")
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
            ExecutionEvent::RemoteChangeUpdate {
                id: "change-a".to_string(),
                completed_tasks: 0,
                total_tasks: 1,
                status: Some("applying".to_string()),
                iteration_number: Some(1),
            },
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

    #[test]
    fn remote_mode_bypasses_local_refresh_path() {
        assert!(super::should_bypass_local_refresh(true));
        assert!(!super::should_bypass_local_refresh(false));
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
    async fn shutdown_local_orchestrator_without_handle_is_remote_client_safe_noop() {
        let outcome = shutdown_local_orchestrator_task(None, None, Duration::from_millis(1)).await;

        assert_eq!(outcome, LocalOrchestratorShutdownOutcome::NoTask);
    }
}
