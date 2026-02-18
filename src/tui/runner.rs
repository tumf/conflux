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
use std::path::PathBuf;
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
// AppMode, QueueStatus, StopMode now used in handlers
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
    use crate::openspec;

    let repo_root = std::env::current_dir()?;
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

    let mut app = AppState::new(initial_changes);
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
    };
    let ai_runner = AiCommandRunner::new(queue_config.clone(), shared_stagger_state.clone());

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
                    RemoteStateUpdate::FullState { projects } => {
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
                                committed_change_ids: std::collections::HashSet::new(),
                                uncommitted_file_change_ids: std::collections::HashSet::new(),
                                worktree_change_ids: std::collections::HashSet::new(),
                                worktree_paths: std::collections::HashMap::new(),
                                worktree_not_ahead_ids: std::collections::HashSet::new(),
                                merge_wait_ids: std::collections::HashSet::new(),
                            })
                            .await;
                    }
                    RemoteStateUpdate::ChangeUpdate { change } => {
                        // Incremental update → send as RemoteChangeUpdate (applies non-regression rule)
                        // Use project.name (from the id->name map) to match the format used by
                        // group_changes_by_project(): "<project.name>/<change.id>"
                        let project_display = project_name_map
                            .get(&change.project)
                            .cloned()
                            .unwrap_or_else(|| change.project.clone());
                        let id = format!("{}/{}", project_display, change.id);
                        let _ = translate_tx
                            .send(super::events::OrchestratorEvent::RemoteChangeUpdate {
                                id,
                                completed_tasks: change.completed_tasks,
                                total_tasks: change.total_tasks,
                            })
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
        if is_remote_mode {
            return;
        }

        let worktree_manager = GitWorkspaceManager::new(
            refresh_worktree_base_dir,
            refresh_repo_root.clone(),
            refresh_config.get_max_concurrent_workspaces(),
            refresh_config,
        );
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS));
        loop {
            tokio::select! {
                _ = refresh_cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    match openspec::list_changes_native() {
                        Ok(mut changes) => {
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

    // Cleanup: cancel all tasks and wait for them to finish
    cancel_token.cancel();
    if let Some(cancel) = orchestrator_cancel {
        cancel.cancel();
    }

    // Wait for tasks to finish gracefully
    refresh_handle.abort();
    if let Some(handle) = orchestrator_handle {
        // Give orchestrator time to cleanup child processes
        // Extended from 2s to 5s for more reliable cleanup (especially on Windows)
        match tokio::time::timeout(Duration::from_secs(5), handle).await {
            Ok(_) => tracing::info!("Orchestrator task finished gracefully"),
            Err(_) => tracing::warn!("Orchestrator task timeout after 5 seconds"),
        }
    }

    Ok(())
}
