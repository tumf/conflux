//! TUI runner and main event loop
//!
//! Contains run_tui and run_tui_loop functions.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use crate::vcs::{GitWorkspaceManager, WorkspaceManager};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::DefaultTerminal;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
use super::utils::clear_screen;

/// Restore terminal state (called on panic or normal exit)
fn restore_terminal() {
    // Always try to disable mouse capture, even if it wasn't enabled
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    let _ = clear_screen();
    ratatui::restore();
}

pub(super) fn should_trigger_worktree_command(
    config: &OrchestratorConfig,
    is_git_repo: bool,
) -> bool {
    config.get_worktree_command().is_some() && is_git_repo
}

pub(super) fn build_worktree_path(base_dir: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    base_dir.join(format!("proposal-{}", timestamp))
}

/// Load worktrees and check for merge conflicts in parallel
pub(super) async fn load_worktrees_with_conflict_check(
    repo_root: &Path,
) -> Result<Vec<super::types::WorktreeInfo>> {
    use super::types::{MergeConflictInfo, WorktreeInfo};

    // First, get the list of worktrees
    let worktrees_data = crate::vcs::git::commands::list_worktrees(repo_root).await?;

    // Convert to WorktreeInfo structs
    let mut worktrees: Vec<WorktreeInfo> = worktrees_data
        .into_iter()
        .map(|(path, head, branch, is_detached, is_main)| WorktreeInfo {
            path: PathBuf::from(path),
            head,
            branch: branch.clone(),
            is_detached,
            is_main,
            merge_conflict: None,
            has_commits_ahead: false, // Will be populated later in parallel check
            is_merging: false,
        })
        .collect();

    // Get the base branch name from the main worktree
    let _base_branch = if let Some(main_wt) = worktrees.iter().find(|wt| wt.is_main) {
        main_wt.branch.clone()
    } else {
        // Fallback: get current branch from repo root
        match crate::vcs::git::commands::get_current_branch(repo_root).await {
            Ok(Some(branch)) => branch,
            Ok(None) | Err(_) => {
                // If we can't get the base branch (detached HEAD or error), skip conflict checking
                return Ok(worktrees);
            }
        }
    };

    // Check conflicts and commits ahead in parallel for non-main, non-detached worktrees
    let mut tasks = tokio::task::JoinSet::new();

    for (idx, worktree) in worktrees.iter().enumerate() {
        // Skip main worktree and detached HEADs
        if worktree.is_main || worktree.is_detached || worktree.branch.is_empty() {
            continue;
        }

        let wt_path = worktree.path.clone();
        let branch_name = worktree.branch.clone();
        let base_branch = _base_branch.clone();

        tasks.spawn(async move {
            // Check merge conflicts
            let conflict_result =
                crate::vcs::git::commands::check_merge_conflicts(&wt_path, &base_branch).await;

            // Check commits ahead
            let ahead_result = crate::vcs::git::commands::count_commits_ahead(
                &wt_path,
                &base_branch,
                &branch_name,
            )
            .await;

            (idx, conflict_result, ahead_result)
        });
    }

    // Collect results
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((idx, conflict_result, ahead_result)) => {
                // Process conflict check result
                match conflict_result {
                    Ok(conflict_files_opt) => {
                        if let Some(conflict_files) = conflict_files_opt {
                            // Conflicts detected
                            worktrees[idx].merge_conflict =
                                Some(MergeConflictInfo { conflict_files });
                        } else {
                            // No conflicts
                            worktrees[idx].merge_conflict = None;
                        }
                    }
                    Err(e) => {
                        // Check failed - treat as unknown (no conflict info)
                        debug!(
                            "Conflict check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                        worktrees[idx].merge_conflict = None;
                    }
                }

                // Process commits ahead check result
                match ahead_result {
                    Ok(count) => {
                        worktrees[idx].has_commits_ahead = count > 0;
                        debug!(
                            "Worktree {} has {} commits ahead of base",
                            worktrees[idx].path.display(),
                            count
                        );
                    }
                    Err(e) => {
                        // Check failed - treat as no commits ahead (safe default)
                        debug!(
                            "Commits ahead check failed for worktree {}: {}",
                            worktrees[idx].path.display(),
                            e
                        );
                        worktrees[idx].has_commits_ahead = false;
                    }
                }
            }
            Err(e) => {
                // Join error
                warn!("Worktree check task panicked: {}", e);
            }
        }
    }

    Ok(worktrees)
}

/// Run the TUI application
pub async fn run_tui(
    initial_changes: Vec<Change>,
    config: OrchestratorConfig,
    web_url: Option<String>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
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
    app.apply_parallel_eligibility(&committed_change_ids);
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

/// Execute a worktree command with terminal suspension and result logging
///
/// This helper executes a worktree command using AiCommandRunner, forwards output
/// to stdout/stderr, and logs the result to the app state.
pub(super) async fn execute_worktree_command(
    terminal: &mut DefaultTerminal,
    command: &str,
    worktree_path: &Path,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    app: &mut AppState,
) -> Result<()> {
    let command_clone = command.to_string();
    let worktree_path_clone = worktree_path.to_path_buf();
    let ai_runner_clone = ai_runner.clone();

    let status_result = suspend_terminal_and_execute(terminal, || async move {
        info!(
            module = module_path!(),
            "Running worktree command via AiCommandRunner: sh -c {}", command_clone
        );

        // Execute via AiCommandRunner (with stagger and retry)
        let exec_result = ai_runner_clone
            .execute_streaming_with_retry(&command_clone, Some(&worktree_path_clone))
            .await;

        match exec_result {
            Ok((mut child, mut rx)) => {
                // Forward output to stdout/stderr in real-time
                use crate::ai_command_runner::OutputLine;
                while let Some(line) = rx.recv().await {
                    match line {
                        OutputLine::Stdout(s) => {
                            println!("{}", s);
                        }
                        OutputLine::Stderr(s) => {
                            eprintln!("{}", s);
                        }
                    }
                }
                // Wait for child to complete
                child
                    .wait()
                    .await
                    .map_err(crate::error::OrchestratorError::Io)
            }
            Err(e) => {
                eprintln!("Failed to execute worktree command: {}", e);
                Err(e)
            }
        }
    })
    .await?;

    match status_result {
        exit_status if exit_status.success() => {
            app.add_log(LogEntry::success("Worktree command completed successfully"));
        }
        exit_status => {
            app.add_log(LogEntry::error(format!(
                "Worktree command failed with exit code: {:?}",
                exit_status.code()
            )));
        }
    }

    Ok(())
}

/// Suspend terminal, execute a function, then restore terminal
///
/// This helper encapsulates the pattern of:
/// 1. Disable raw mode and leave alternate screen
/// 2. Execute a function (which may interact with the terminal)
/// 3. Restore raw mode and alternate screen
async fn suspend_terminal_and_execute<F, Fut, T>(terminal: &mut DefaultTerminal, f: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    // Suspend TUI
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    // Execute the provided function
    let result = f().await;

    // Restore TUI
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;

    result
}

/// Suspend terminal, execute a synchronous function, then restore terminal
pub(super) fn suspend_terminal_and_execute_sync<F, T>(
    terminal: &mut DefaultTerminal,
    f: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    // Suspend TUI
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    // Execute the provided function
    let result = f();

    // Restore TUI
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    #[test]
    fn test_should_trigger_worktree_command_missing_config() {
        let config = OrchestratorConfig::default();
        assert!(!should_trigger_worktree_command(&config, true));
    }

    #[test]
    fn test_should_trigger_worktree_command_not_git_repo() {
        let config = OrchestratorConfig {
            worktree_command: Some("cmd {workspace_dir}".to_string()),
            ..Default::default()
        };
        assert!(!should_trigger_worktree_command(&config, false));
    }

    #[test]
    fn test_should_trigger_worktree_command_enabled() {
        let config = OrchestratorConfig {
            worktree_command: Some("cmd {repo_root}".to_string()),
            ..Default::default()
        };
        assert!(should_trigger_worktree_command(&config, true));
    }
}
