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
use tracing::{info, warn};

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

/// Run the TUI application
pub async fn run_tui(
    initial_changes: Vec<Change>,
    openspec_cmd: String,
    _opencode_path: String, // Deprecated - use config instead
    config: OrchestratorConfig,
    web_url: Option<String>,
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
        openspec_cmd,
        config,
        web_url,
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
    openspec_cmd: String,
    config: OrchestratorConfig,
    web_url: Option<String>,
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
        .unwrap_or_else(|| std::env::temp_dir().join("openspec-tui-worktrees"));
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

    let mut app = AppState::new(initial_changes);
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
                        Ok(changes) => {
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

                            if refresh_tx
                                .send(OrchestratorEvent::ChangesRefreshed {
                                    changes,
                                    committed_change_ids,
                                    worktree_change_ids,
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
                                if let Some(cmd) = app.confirm_worktree_delete() {
                                    let _ = cmd_tx.send(cmd).await;
                                }
                            }
                            (KeyCode::Char('n'), _)
                            | (KeyCode::Char('N'), _)
                            | (KeyCode::Esc, _) => {
                                app.cancel_worktree_delete();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                            break;
                        }
                        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                            app.cursor_up();
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                            app.cursor_down();
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
                            // Open editor in change directory
                            if !app.changes.is_empty() && app.cursor_index < app.changes.len() {
                                let change_id = app.changes[app.cursor_index].id.clone();

                                // Suspend TUI and launch editor
                                disable_raw_mode()?;
                                execute!(
                                    std::io::stdout(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                // Launch editor
                                if let Err(e) = super::utils::launch_editor_for_change(&change_id) {
                                    eprintln!("Failed to launch editor: {}", e);
                                }

                                // Restore TUI
                                enable_raw_mode()?;
                                execute!(
                                    std::io::stdout(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                            }
                        }
                        (KeyCode::Char('m'), _) | (KeyCode::Char('M'), _) => {
                            if let Some(cmd) = app.resolve_merge() {
                                let _ = cmd_tx.send(cmd).await;
                            }
                        }
                        (KeyCode::Char('d'), _) | (KeyCode::Char('D'), _) => {
                            if app.mode == AppMode::Select {
                                app.request_worktree_delete();
                            }
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
                                    // Reset any in-flight change back to Queued
                                    for change in &mut app.changes {
                                        if matches!(
                                            change.queue_status,
                                            QueueStatus::Processing | QueueStatus::Archiving
                                        ) {
                                            change.queue_status = QueueStatus::Queued;
                                        }
                                    }
                                    app.current_change = None;
                                    app.mode = AppMode::Stopped;
                                    app.add_log(LogEntry::warn("Force stopped"));
                                }
                                _ => {}
                            }
                        }
                        (KeyCode::F(5), _) => {
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
                                    let orch_openspec_cmd = openspec_cmd.clone();
                                    let orch_config = config.clone();
                                    let orch_cancel = CancellationToken::new();
                                    let orch_dynamic_queue = dynamic_queue.clone();
                                    let orch_graceful_stop = graceful_stop_flag.clone();
                                    orchestrator_cancel = Some(orch_cancel.clone());
                                    let use_parallel = app.parallel_mode;

                                    orchestrator_handle = Some(tokio::spawn(async move {
                                        let result = if use_parallel {
                                            run_orchestrator_parallel(
                                                selected_ids,
                                                orch_openspec_cmd,
                                                orch_config,
                                                orch_tx.clone(),
                                                orch_cancel,
                                                orch_dynamic_queue,
                                                orch_graceful_stop,
                                            )
                                            .await
                                        } else {
                                            run_orchestrator(
                                                selected_ids,
                                                orch_openspec_cmd,
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
                        (KeyCode::Char('+'), _) => {
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

                            // Generate unique branch name with format: oso-session-<rand>
                            let branch_name =
                                match crate::vcs::git::commands::generate_unique_branch_name(
                                    &repo_root,
                                    "oso-session",
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
