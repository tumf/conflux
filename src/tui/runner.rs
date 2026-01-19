//! TUI runner and main event loop
//!
//! Contains run_tui and run_tui_loop functions.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use crate::vcs::{GitWorkspaceManager, WorkspaceManager};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::DefaultTerminal;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::events::{LogEntry, OrchestratorEvent, TuiCommand};
use super::log_deduplicator;
use super::orchestrator::{run_orchestrator, run_orchestrator_parallel};
use super::queue::DynamicQueue;
use super::render::{render, SPINNER_CHARS};
use super::state::{AppState, AUTO_REFRESH_INTERVAL_SECS};
use super::types::{AppMode, QueueStatus, StopMode};
use super::utils::clear_screen;

/// Restore terminal state (called on panic or normal exit)
fn restore_terminal() {
    // Always try to disable mouse capture, even if it wasn't enabled
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    let _ = clear_screen();
    ratatui::restore();
}

fn should_trigger_worktree_command(config: &OrchestratorConfig, is_git_repo: bool) -> bool {
    config.get_worktree_command().is_some() && is_git_repo
}

fn build_worktree_path(base_dir: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    base_dir.join(format!("proposal-{}", timestamp))
}

/// Load worktrees and check for merge conflicts in parallel
async fn load_worktrees_with_conflict_check(
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

    let mut app = AppState::new(initial_changes);
    app.worktree_paths = initial_worktree_paths;
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
    let (tx, mut rx) = mpsc::channel::<OrchestratorEvent>(100);
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<TuiCommand>(100);

    // Dynamic queue for runtime change additions
    let dynamic_queue = DynamicQueue::new();

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
                                        // Try active change location first
                                        match crate::task_parser::parse_change_with_worktree_fallback(
                                            &change.id,
                                            Some(&wt_path)
                                        ) {
                                            Ok(progress) => {
                                                if progress.total == 0 {
                                                    // If progress is 0/0, try archived location
                                                    match crate::task_parser::parse_archived_change_with_worktree_fallback(
                                                        &change.id,
                                                        Some(&wt_path)
                                                    ) {
                                                        Ok(archived_progress) if archived_progress.total > 0 => {
                                                            change.completed_tasks = archived_progress.completed;
                                                            change.total_tasks = archived_progress.total;
                                                        }
                                                        _ => {
                                                            // Keep existing progress if archived progress is also 0/0 or unavailable
                                                            debug!("Keeping existing progress for {} (active: 0/0, archived: unavailable)", change.id);
                                                        }
                                                    }
                                                } else {
                                                    change.completed_tasks = progress.completed;
                                                    change.total_tasks = progress.total;
                                                }
                                            }
                                            Err(_) => {
                                                // If not found in active location, try archived location
                                                // This handles changes that have been archived but not yet merged
                                                match crate::task_parser::parse_archived_change_with_worktree_fallback(
                                                    &change.id,
                                                    Some(&wt_path)
                                                ) {
                                                    Ok(progress) => {
                                                        change.completed_tasks = progress.completed;
                                                        change.total_tasks = progress.total;
                                                    }
                                                    Err(e) => {
                                                        debug!("Failed to read worktree progress for {}: {}", change.id, e);
                                                        // Keep existing progress (from base tree)
                                                    }
                                                }
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
                                }
                            }

                            if refresh_tx
                                .send(OrchestratorEvent::ChangesRefreshed {
                                    changes,
                                    committed_change_ids,
                                    worktree_change_ids,
                                    worktree_paths,
                                    worktree_not_ahead_ids,
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
                    let had_warning_message = app.warning_message.is_some();
                    let had_warning_popup = app.warning_popup.is_some();

                    // Handle QrPopup mode - any key closes the popup
                    if app.mode == AppMode::QrPopup {
                        app.hide_qr_popup();
                        continue;
                    }

                    // Handle worktree delete confirmation
                    if app.mode == AppMode::ConfirmWorktreeDelete {
                        match (key.code, key.modifiers) {
                            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                                if let Some(cmd) = app.confirm_worktree_action_delete() {
                                    let _ = cmd_tx.send(cmd).await;
                                }
                            }
                            (KeyCode::Char('n'), _)
                            | (KeyCode::Char('N'), _)
                            | (KeyCode::Esc, _) => {
                                app.cancel_worktree_action();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match (key.code, key.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                            break;
                        }
                        (KeyCode::Tab, _) => {
                            // Switch between Changes and Worktrees views
                            use crate::tui::types::ViewMode;
                            let new_view = match app.view_mode {
                                ViewMode::Changes => ViewMode::Worktrees,
                                ViewMode::Worktrees => ViewMode::Changes,
                            };

                            // Load worktrees with conflict check when switching to Worktrees view
                            if new_view == ViewMode::Worktrees {
                                let load_tx = tx.clone();
                                let load_repo_root = repo_root.clone();
                                tokio::spawn(async move {
                                    match load_worktrees_with_conflict_check(&load_repo_root).await
                                    {
                                        Ok(worktrees) => {
                                            let _ = load_tx
                                                .send(OrchestratorEvent::WorktreesRefreshed {
                                                    worktrees,
                                                })
                                                .await;
                                        }
                                        Err(e) => {
                                            let _ = load_tx
                                                .send(OrchestratorEvent::Log(LogEntry::error(
                                                    format!("Failed to load worktrees: {}", e),
                                                )))
                                                .await;
                                        }
                                    }
                                });
                            }

                            app.view_mode = new_view;
                        }
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            use crate::tui::types::ViewMode;
                            match app.view_mode {
                                ViewMode::Changes => app.cursor_up(),
                                ViewMode::Worktrees => app.worktree_cursor_up(),
                            }
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            use crate::tui::types::ViewMode;
                            match app.view_mode {
                                ViewMode::Changes => app.cursor_down(),
                                ViewMode::Worktrees => app.worktree_cursor_down(),
                            }
                        }
                        (KeyCode::Char(' '), _) => {
                            if let Some(cmd) = app.toggle_selection() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        (KeyCode::Char('@'), _) => {
                            // Toggle approval status
                            if let Some(cmd) = app.toggle_approval() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        (KeyCode::Char('e'), _) => {
                            use crate::tui::types::ViewMode;

                            // Suspend TUI
                            disable_raw_mode()?;
                            execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                            // Launch editor based on view mode
                            match app.view_mode {
                                ViewMode::Changes => {
                                    if !app.changes.is_empty()
                                        && app.cursor_index < app.changes.len()
                                    {
                                        let change_id = app.changes[app.cursor_index].id.clone();
                                        if let Err(e) =
                                            super::utils::launch_editor_for_change(&change_id)
                                        {
                                            eprintln!("Failed to launch editor: {}", e);
                                        }
                                    }
                                }
                                ViewMode::Worktrees => {
                                    if let Some(path) = app.get_selected_worktree_path() {
                                        if let Err(e) = super::utils::launch_editor_in_dir(&path) {
                                            eprintln!("Failed to launch editor: {}", e);
                                        }
                                    }
                                }
                            }

                            // Restore TUI
                            enable_raw_mode()?;
                            execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                            terminal.clear()?;
                        }
                        (KeyCode::Char('m'), _) | (KeyCode::Char('M'), _) => {
                            use crate::tui::types::ViewMode;
                            use tracing::debug;

                            debug!("M key pressed: view_mode={:?}", app.view_mode);

                            match app.view_mode {
                                ViewMode::Changes => {
                                    // Changes view: resolve deferred merge
                                    debug!("M key (Changes view): attempting resolve_merge");
                                    if let Some(cmd) = app.resolve_merge() {
                                        debug!("M key (Changes view): sending command {:?}", cmd);
                                        let _ = cmd_tx.send(cmd).await;
                                    } else {
                                        debug!("M key (Changes view): resolve_merge returned None");
                                    }
                                }
                                ViewMode::Worktrees => {
                                    // Worktrees view: merge branch to base
                                    debug!("M key (Worktrees view): attempting request_merge_worktree_branch");
                                    if let Some(cmd) = app.request_merge_worktree_branch() {
                                        debug!("M key (Worktrees view): sending command {:?}", cmd);
                                        let _ = cmd_tx.send(cmd).await;
                                    } else {
                                        debug!("M key (Worktrees view): request_merge_worktree_branch returned None");
                                    }
                                }
                            }
                        }
                        (KeyCode::Char('d'), _) | (KeyCode::Char('D'), _) => {
                            use crate::tui::types::ViewMode;
                            if app.view_mode == ViewMode::Worktrees {
                                // Worktree view: delete selected worktree
                                app.request_worktree_delete_from_list();
                            }
                            // Note: D key removed from Changes view as per spec
                        }
                        (KeyCode::Esc, _) => {
                            // Handle stop in Running or Stopping mode
                            match app.mode {
                                AppMode::Running => {
                                    // First Esc: Graceful stop
                                    app.stop_mode = StopMode::GracefulPending;
                                    graceful_stop_flag.store(true, Ordering::SeqCst);
                                    app.mode = AppMode::Stopping;
                                    app.add_log(LogEntry::warn(
                                        "Stopping after current change completes...",
                                    ));
                                }
                                AppMode::Stopping => {
                                    // Second Esc: Force stop
                                    app.stop_mode = StopMode::ForceStopped;
                                    if let Some(cancel) = &orchestrator_cancel {
                                        cancel.cancel();
                                    }
                                    // Use OrchestratorEvent::Stopped to properly reset queue status
                                    // and preserve execution marks (same as graceful stop)
                                    app.handle_orchestrator_event(OrchestratorEvent::Stopped);
                                    app.current_change = None;
                                    app.add_log(LogEntry::warn("Force stopped"));
                                }
                                _ => {}
                            }
                        }
                        (KeyCode::F(5), _) => {
                            // Handle F5 in Stopping mode to cancel graceful stop
                            if app.mode == AppMode::Stopping {
                                // Check if orchestrator is still running
                                if orchestrator_handle
                                    .as_ref()
                                    .is_some_and(|h| !h.is_finished())
                                {
                                    // Cancel graceful stop and return to Running mode
                                    graceful_stop_flag.store(false, Ordering::SeqCst);
                                    app.stop_mode = StopMode::None;
                                    app.mode = AppMode::Running;
                                    app.add_log(LogEntry::info("Stop canceled, continuing..."));
                                } else {
                                    // Already stopped, cannot cancel
                                    app.add_log(LogEntry::warn(
                                        "Cannot cancel stop: processing already completed",
                                    ));
                                }
                                continue;
                            }

                            // Determine which command to use based on mode
                            let cmd = if app.mode == AppMode::Error {
                                app.retry_error_changes()
                            } else if app.mode == AppMode::Stopped {
                                app.resume_processing()
                            } else {
                                app.start_processing()
                            };

                            if let Some(cmd) = cmd {
                                // Start orchestrator task
                                let selected_ids = match &cmd {
                                    TuiCommand::StartProcessing(ids) => ids.clone(),
                                    _ => vec![],
                                };

                                if !selected_ids.is_empty() {
                                    graceful_stop_flag.store(false, Ordering::SeqCst);
                                    let orch_tx = tx.clone();
                                    let orch_config = config.clone();
                                    let orch_cancel = CancellationToken::new();
                                    let orch_dynamic_queue = dynamic_queue.clone();
                                    let orch_graceful_stop = graceful_stop_flag.clone();
                                    orchestrator_cancel = Some(orch_cancel.clone());
                                    let use_parallel = app.parallel_mode;
                                    #[cfg(feature = "web-monitoring")]
                                    let orch_web_state = web_state.clone();

                                    orchestrator_handle = Some(tokio::spawn(async move {
                                        let result = if use_parallel {
                                            run_orchestrator_parallel(
                                                selected_ids,
                                                orch_config,
                                                orch_tx.clone(),
                                                orch_cancel,
                                                orch_dynamic_queue,
                                                orch_graceful_stop,
                                                #[cfg(feature = "web-monitoring")]
                                                orch_web_state,
                                            )
                                            .await
                                        } else {
                                            run_orchestrator(
                                                selected_ids,
                                                orch_config,
                                                orch_tx.clone(),
                                                orch_cancel,
                                                orch_dynamic_queue,
                                                orch_graceful_stop,
                                            )
                                            .await
                                        };

                                        // Log any errors from the orchestrator
                                        if let Err(ref e) = result {
                                            let _ = orch_tx
                                                .send(OrchestratorEvent::Log(LogEntry::error(
                                                    format!("Orchestrator error: {}", e),
                                                )))
                                                .await;
                                        }

                                        result
                                    }));
                                }
                            }
                        }
                        (KeyCode::PageUp, _) => {
                            // Scroll logs up (show older entries)
                            app.scroll_logs_up(5);
                        }
                        (KeyCode::PageDown, _) => {
                            // Scroll logs down (show newer entries)
                            app.scroll_logs_down(5);
                        }
                        (KeyCode::Home, _) => {
                            // Jump to oldest log entry
                            app.scroll_logs_to_top();
                        }
                        (KeyCode::End, _) => {
                            // Jump to newest log entry and re-enable auto-scroll
                            app.scroll_logs_to_bottom();
                        }
                        (KeyCode::Char('='), _) => {
                            // Toggle parallel mode (only if git is available)
                            app.toggle_parallel_mode();
                        }
                        (KeyCode::Enter, _) => {
                            use crate::tui::types::ViewMode;

                            if app.view_mode != ViewMode::Worktrees {
                                app.add_log(LogEntry::warn("Enter ignored: not in Worktrees view"));
                                continue;
                            }

                            let Some(worktree_path_str) = app.get_selected_worktree_path() else {
                                app.add_log(LogEntry::warn("Enter ignored: no worktree selected"));
                                continue;
                            };

                            let Some(template) = config.get_worktree_command().map(str::to_string)
                            else {
                                app.add_log(LogEntry::warn(
                                    "Enter ignored: worktree_command not configured",
                                ));
                                continue;
                            };

                            let Some(repo_root_str) = repo_root.to_str() else {
                                app.add_log(LogEntry::error(
                                    "Failed to resolve repo root path".to_string(),
                                ));
                                continue;
                            };

                            let command = OrchestratorConfig::expand_worktree_command(
                                &template,
                                &worktree_path_str,
                                repo_root_str,
                            );

                            app.add_log(LogEntry::info(format!(
                                "Running worktree command in {}",
                                worktree_path_str
                            )));

                            disable_raw_mode()?;
                            execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                            info!(
                                module = module_path!(),
                                "Running worktree command: sh -c {}", command
                            );
                            let status = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(&command)
                                .current_dir(&worktree_path_str)
                                .status();

                            enable_raw_mode()?;
                            execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                            terminal.clear()?;

                            match status {
                                Ok(exit_status) if exit_status.success() => {
                                    app.add_log(LogEntry::success(
                                        "Worktree command completed successfully",
                                    ));
                                }
                                Ok(exit_status) => {
                                    app.add_log(LogEntry::error(format!(
                                        "Worktree command failed with exit code: {:?}",
                                        exit_status.code()
                                    )));
                                }
                                Err(err) => {
                                    app.add_log(LogEntry::error(format!(
                                        "Failed to execute worktree command: {}",
                                        err
                                    )));
                                }
                            }
                        }
                        (KeyCode::Char('+'), _) => {
                            use crate::tui::types::ViewMode;

                            // Only work in Worktrees view
                            if app.view_mode != ViewMode::Worktrees {
                                continue;
                            }

                            let Some(template) = config.get_worktree_command().map(str::to_string)
                            else {
                                continue;
                            };

                            let is_git_repo =
                                match crate::vcs::git::commands::check_git_repo(&repo_root).await {
                                    Ok(is_repo) => is_repo,
                                    Err(err) => {
                                        app.add_log(LogEntry::error(format!(
                                            "Failed to check git repo: {}",
                                            err
                                        )));
                                        continue;
                                    }
                                };

                            if !should_trigger_worktree_command(&config, is_git_repo) {
                                continue;
                            }

                            if let Err(err) = std::fs::create_dir_all(&worktree_base_dir) {
                                app.add_log(LogEntry::error(format!(
                                    "Failed to prepare worktree base dir: {}",
                                    err
                                )));
                                continue;
                            }

                            let worktree_path = build_worktree_path(&worktree_base_dir);
                            let Some(worktree_path_str) = worktree_path.to_str() else {
                                app.add_log(LogEntry::error(
                                    "Failed to resolve worktree path".to_string(),
                                ));
                                continue;
                            };
                            let Some(repo_root_str) = repo_root.to_str() else {
                                app.add_log(LogEntry::error(
                                    "Failed to resolve repo root path".to_string(),
                                ));
                                continue;
                            };

                            // Generate unique branch name with format: ws-session-<timestamp>
                            let branch_name =
                                match crate::vcs::git::commands::generate_unique_branch_name(
                                    &repo_root,
                                    "ws-session",
                                    10,
                                )
                                .await
                                {
                                    Ok(name) => name,
                                    Err(err) => {
                                        app.add_log(LogEntry::error(format!(
                                            "Failed to generate unique branch name: {}",
                                            err
                                        )));
                                        continue;
                                    }
                                };

                            // Create worktree with branch instead of detached HEAD
                            if let Err(err) = crate::vcs::git::commands::worktree_add(
                                &repo_root,
                                worktree_path_str,
                                &branch_name,
                                "HEAD",
                            )
                            .await
                            {
                                app.add_log(LogEntry::error(format!(
                                    "Failed to create worktree: {}",
                                    err
                                )));
                                continue;
                            }

                            app.add_log(LogEntry::info(format!(
                                "Created worktree with branch '{}'",
                                branch_name
                            )));

                            // Execute setup script if it exists
                            if let Err(err) = crate::vcs::git::commands::run_worktree_setup(
                                &repo_root,
                                &worktree_path,
                            )
                            .await
                            {
                                app.add_log(LogEntry::error(format!(
                                    "Failed to run worktree setup: {}",
                                    err
                                )));
                                // Don't continue - setup failure is considered an error
                                // but the worktree was already created, so we should clean it up
                                if let Err(cleanup_err) =
                                    crate::vcs::git::commands::worktree_remove(
                                        &repo_root,
                                        worktree_path_str,
                                    )
                                    .await
                                {
                                    app.add_log(LogEntry::error(format!(
                                        "Failed to cleanup worktree after setup failure: {}",
                                        cleanup_err
                                    )));
                                }
                                continue;
                            }

                            let command = OrchestratorConfig::expand_worktree_command(
                                &template,
                                worktree_path_str,
                                repo_root_str,
                            );
                            app.add_log(LogEntry::info(format!(
                                "Running worktree command in {}",
                                worktree_path_str
                            )));

                            disable_raw_mode()?;
                            execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                            info!(
                                module = module_path!(),
                                "Running worktree command: sh -c {}", command
                            );
                            let status = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(&command)
                                .current_dir(&worktree_path)
                                .status();

                            enable_raw_mode()?;
                            execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                            terminal.clear()?;

                            match status {
                                Ok(exit_status) if exit_status.success() => {
                                    app.add_log(LogEntry::success(
                                        "Worktree command completed successfully",
                                    ));
                                }
                                Ok(exit_status) => {
                                    app.add_log(LogEntry::error(format!(
                                        "Worktree command failed with exit code: {:?}",
                                        exit_status.code()
                                    )));
                                }
                                Err(err) => {
                                    app.add_log(LogEntry::error(format!(
                                        "Failed to execute worktree command: {}",
                                        err
                                    )));
                                }
                            }
                        }
                        (KeyCode::Char('w'), _) => {
                            // Show QR code popup (only if web_url is set)
                            if app.web_url.is_some() {
                                app.show_qr_popup();
                            }
                        }
                        _ => {}
                    }
                    // Clear previous warning message on any key press
                    if had_warning_message || had_warning_popup {
                        app.warning_message = None;
                        app.warning_popup = None;
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
            // Update web state when changes are refreshed (web-monitoring feature only)
            #[cfg(feature = "web-monitoring")]
            if let OrchestratorEvent::ChangesRefreshed { ref changes, .. } = event {
                if let Some(ref web_state) = web_state {
                    web_state.update(changes).await;
                }
            }

            app.handle_orchestrator_event(event);
        }

        // Handle dynamic queue additions and removals
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TuiCommand::AddToQueue(id) => {
                    // Push to dynamic queue for orchestrator to pick up
                    if dynamic_queue.push(id.clone()).await {
                        app.add_log(LogEntry::info(format!("Added to dynamic queue: {}", id)));
                    } else {
                        app.add_log(LogEntry::warn(format!("Already in dynamic queue: {}", id)));
                    }
                }
                TuiCommand::RemoveFromQueue(id) => {
                    // Remove from dynamic queue so orchestrator won't process it
                    let removed_from_dynamic = dynamic_queue.remove(&id).await;
                    let removed_from_pending = dynamic_queue.mark_removed(id.clone()).await;
                    let mut details = Vec::new();
                    if removed_from_dynamic {
                        details.push("also removed from dynamic queue");
                    }
                    if removed_from_pending {
                        details.push("removed from pending");
                    }
                    let suffix = if details.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", details.join(", "))
                    };
                    app.add_log(LogEntry::info(format!(
                        "Removed from queue: {}{}",
                        id, suffix
                    )));
                }
                TuiCommand::ApproveOnly(id) => {
                    // Approve without adding to queue (select/running/stopped modes)
                    use crate::approval;

                    match approval::approve_change(&id) {
                        Ok(_) => {
                            app.update_approval_status(&id, true);
                            if app.mode == AppMode::Select {
                                if let Some(change) = app.changes.iter_mut().find(|c| c.id == id) {
                                    change.selected = true;
                                    change.queue_status = QueueStatus::NotQueued;
                                }
                            }
                            app.add_log(LogEntry::info(format!("Approved (not queued): {}", id)));
                        }
                        Err(e) => {
                            app.add_log(LogEntry::error(format!(
                                "Failed to approve '{}': {}",
                                id, e
                            )));
                        }
                    }
                }
                TuiCommand::UnapproveAndDequeue(id) => {
                    // Unapprove and remove from queue (used in running/completed mode)
                    use crate::approval;

                    // First check if queued
                    let was_queued = app
                        .changes
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| matches!(c.queue_status, QueueStatus::Queued))
                        .unwrap_or(false);

                    match approval::unapprove_change(&id) {
                        Ok(_) => {
                            app.update_approval_status(&id, false);
                            // Also remove from queue if queued
                            if let Some(change) = app.changes.iter_mut().find(|c| c.id == id) {
                                if matches!(change.queue_status, QueueStatus::Queued) {
                                    change.queue_status = QueueStatus::NotQueued;
                                }
                                change.selected = false;
                            }
                            // Remove from dynamic queue so orchestrator won't process it
                            let removed_from_dynamic = dynamic_queue.remove(&id).await;
                            let removed_from_pending = dynamic_queue.mark_removed(id.clone()).await;
                            let mut details = Vec::new();
                            if removed_from_dynamic {
                                details.push("also removed from dynamic queue");
                            }
                            if removed_from_pending {
                                details.push("removed from pending");
                            }
                            let suffix = if details.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", details.join(", "))
                            };
                            let msg = if was_queued {
                                format!("Unapproved and removed from queue: {}{}", id, suffix)
                            } else {
                                format!("Unapproved: {}{}", id, suffix)
                            };
                            app.add_log(LogEntry::info(msg));
                        }
                        Err(e) => {
                            app.add_log(LogEntry::error(format!(
                                "Failed to unapprove '{}': {}",
                                id, e
                            )));
                        }
                    }
                }
                TuiCommand::DeleteWorktree(id) => {
                    match crate::vcs::git::remove_worktrees_for_change(&repo_root, &id).await {
                        Ok(removed) => {
                            if removed == 0 {
                                app.warning_popup = Some(super::state::WarningPopup {
                                    title: "Worktree not found".to_string(),
                                    message: format!("No worktree found for change '{}'.", id),
                                });
                                app.add_log(LogEntry::warn(format!(
                                    "No worktree to delete for '{}'",
                                    id
                                )));
                            } else {
                                if let Some(change) =
                                    app.changes.iter_mut().find(|change| change.id == id)
                                {
                                    change.has_worktree = false;
                                }
                                app.add_log(LogEntry::success(format!(
                                    "Deleted {} worktree(s) for '{}'",
                                    removed, id
                                )));
                            }
                        }
                        Err(e) => {
                            app.warning_popup = Some(super::state::WarningPopup {
                                title: "Worktree delete failed".to_string(),
                                message: format!("Failed to delete worktrees for '{}': {}", id, e),
                            });
                            app.add_log(LogEntry::error(format!(
                                "Worktree delete failed for '{}': {}",
                                id, e
                            )));
                        }
                    }
                }
                TuiCommand::DeleteWorktreeByPath(path) => {
                    match crate::vcs::git::commands::worktree_remove(
                        &repo_root,
                        path.to_string_lossy().as_ref(),
                    )
                    .await
                    {
                        Ok(_) => {
                            app.add_log(LogEntry::success(format!(
                                "Deleted worktree: {}",
                                path.display()
                            )));

                            // Refresh worktree list with conflict check
                            match load_worktrees_with_conflict_check(&repo_root).await {
                                Ok(worktrees) => {
                                    let _ = tx
                                        .send(OrchestratorEvent::WorktreesRefreshed { worktrees })
                                        .await;
                                }
                                Err(e) => {
                                    app.add_log(LogEntry::error(format!(
                                        "Failed to refresh worktrees: {}",
                                        e
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            app.warning_popup = Some(super::state::WarningPopup {
                                title: "Worktree delete failed".to_string(),
                                message: format!(
                                    "Failed to delete worktree '{}': {}",
                                    path.display(),
                                    e
                                ),
                            });
                            app.add_log(LogEntry::error(format!(
                                "Worktree delete failed for '{}': {}",
                                path.display(),
                                e
                            )));
                        }
                    }
                }
                TuiCommand::MergeWorktreeBranch {
                    worktree_path,
                    branch_name,
                } => {
                    use tracing::debug;

                    debug!(
                        "Processing TuiCommand::MergeWorktreeBranch: worktree_path={}, branch_name={}",
                        worktree_path.display(),
                        branch_name
                    );

                    let merge_tx = tx.clone();
                    let merge_repo_root = repo_root.clone();
                    let merge_branch = branch_name.clone();

                    tokio::spawn(async move {
                        debug!(
                            "Sending BranchMergeStarted event for branch: {}",
                            merge_branch
                        );
                        let _ = merge_tx
                            .send(OrchestratorEvent::BranchMergeStarted {
                                branch_name: merge_branch.clone(),
                            })
                            .await;

                        // FIX: Merge in base (main worktree), not in worktree directory
                        // This ensures working directory clean check happens on base side
                        debug!(
                            "Executing merge in base repository: repo_root={}, branch={}",
                            merge_repo_root.display(),
                            merge_branch
                        );
                        match crate::vcs::git::commands::merge_branch(
                            &merge_repo_root,
                            &merge_branch,
                        )
                        .await
                        {
                            Ok(_) => {
                                debug!("Merge succeeded for branch: {}", merge_branch);
                                let _ = merge_tx
                                    .send(OrchestratorEvent::BranchMergeCompleted {
                                        branch_name: merge_branch.clone(),
                                    })
                                    .await;

                                // Refresh worktree list to update UI with conflict check
                                debug!("Refreshing worktree list after successful merge");
                                match load_worktrees_with_conflict_check(&merge_repo_root).await {
                                    Ok(worktrees) => {
                                        debug!(
                                            "Worktree list refreshed: {} worktrees",
                                            worktrees.len()
                                        );
                                        let _ = merge_tx
                                            .send(OrchestratorEvent::WorktreesRefreshed {
                                                worktrees,
                                            })
                                            .await;
                                    }
                                    Err(e) => {
                                        debug!("Failed to refresh worktrees: {}", e);
                                        let _ = merge_tx
                                            .send(OrchestratorEvent::Log(LogEntry::error(format!(
                                                "Failed to refresh worktrees: {}",
                                                e
                                            ))))
                                            .await;
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Merge failed for branch {}: {}", merge_branch, e);
                                let _ = merge_tx
                                    .send(OrchestratorEvent::BranchMergeFailed {
                                        branch_name: merge_branch,
                                        error: format!("{}", e),
                                    })
                                    .await;
                            }
                        }
                    });
                }
                TuiCommand::ResolveMerge(id) => {
                    let resolve_tx = tx.clone();
                    let resolve_repo_root = repo_root.clone();
                    let resolve_config = config.clone();
                    let resolve_worktree_base_dir = worktree_base_dir.clone();
                    tokio::spawn(async move {
                        let _ = resolve_tx
                            .send(OrchestratorEvent::ResolveStarted {
                                change_id: id.clone(),
                            })
                            .await;

                        match crate::parallel::base_dirty_reason(&resolve_repo_root).await {
                            Ok(Some(reason)) => {
                                let _ = resolve_tx
                                    .send(OrchestratorEvent::ResolveFailed {
                                        change_id: id.clone(),
                                        error: format!("Base is dirty: {}", reason),
                                    })
                                    .await;
                                return;
                            }
                            Err(e) => {
                                let _ = resolve_tx
                                    .send(OrchestratorEvent::ResolveFailed {
                                        change_id: id.clone(),
                                        error: format!("Failed to check base status: {}", e),
                                    })
                                    .await;
                                return;
                            }
                            Ok(None) => {}
                        }

                        match crate::parallel::resolve_deferred_merge(
                            resolve_repo_root.clone(),
                            resolve_config.clone(),
                            &id,
                        )
                        .await
                        {
                            Ok(_) => {
                                let refresh_manager = GitWorkspaceManager::new(
                                    resolve_worktree_base_dir.clone(),
                                    resolve_repo_root.clone(),
                                    resolve_config.get_max_concurrent_workspaces(),
                                    resolve_config.clone(),
                                );
                                let worktree_change_ids = match refresh_manager
                                    .list_worktree_change_ids()
                                    .await
                                {
                                    Ok(ids) => Some(ids),
                                    Err(err) => {
                                        let _ = resolve_tx
                                            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                                                "Failed to refresh worktree snapshot: {}",
                                                err
                                            ))))
                                            .await;
                                        None
                                    }
                                };
                                let _ = resolve_tx
                                    .send(OrchestratorEvent::ResolveCompleted {
                                        change_id: id.clone(),
                                        worktree_change_ids,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = resolve_tx
                                    .send(OrchestratorEvent::ResolveFailed {
                                        change_id: id.clone(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    });
                }
                _ => {}
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
