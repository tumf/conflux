//! TUI runner and main event loop
//!
//! Contains run_tui and run_tui_loop functions.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::DefaultTerminal;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::events::{LogEntry, OrchestratorEvent, TuiCommand};
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

    let mut app = AppState::new(initial_changes);
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
    let refresh_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(AUTO_REFRESH_INTERVAL_SECS));
        loop {
            tokio::select! {
                _ = refresh_cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    match openspec::list_changes_native() {
                        Ok(changes) => {
                            if refresh_tx
                                .send(OrchestratorEvent::ChangesRefreshed(changes))
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
                    // Handle QrPopup mode - any key closes the popup
                    if app.mode == AppMode::QrPopup {
                        app.hide_qr_popup();
                        continue;
                    }

                    // Handle Proposing mode separately (textarea input)
                    if app.mode == AppMode::Proposing {
                        match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) => {
                                app.cancel_proposing();
                            }
                            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                                // Try to submit proposal
                                if let Some(proposal) = app.submit_proposal() {
                                    // Submission successful, send command
                                    let _ = cmd_tx.send(TuiCommand::SubmitProposal(proposal)).await;
                                } else {
                                    // Submission failed (empty input), stay in Proposing mode
                                    // No action needed - submit_proposal() already keeps mode and input
                                }
                            }
                            (KeyCode::Char(_), _)
                            | (KeyCode::Backspace, _)
                            | (KeyCode::Delete, _)
                            | (KeyCode::Enter, _)
                            | (KeyCode::Left, _)
                            | (KeyCode::Right, _)
                            | (KeyCode::Up, _)
                            | (KeyCode::Down, _)
                            | (KeyCode::Home, _)
                            | (KeyCode::End, _) => {
                                if let Some(ref mut textarea) = app.propose_textarea {
                                    // Convert crossterm KeyEvent to tui-textarea Input
                                    use tui_textarea::Input;
                                    let input: Input = key.into();
                                    textarea.input(input);
                                }
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
                                // Resume after stop: collect queued changes
                                let queued: Vec<String> = app
                                    .changes
                                    .iter()
                                    .filter(|c| matches!(c.queue_status, QueueStatus::Queued))
                                    .map(|c| c.id.clone())
                                    .collect();
                                if queued.is_empty() {
                                    app.warning_message =
                                        Some("No queued changes to resume".to_string());
                                    None
                                } else {
                                    app.mode = AppMode::Running;
                                    app.stop_mode = StopMode::None;
                                    graceful_stop_flag.store(false, Ordering::SeqCst);
                                    app.add_log(LogEntry::info(format!(
                                        "Resuming processing {} change(s)",
                                        queued.len()
                                    )));
                                    Some(TuiCommand::StartProcessing(queued))
                                }
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
                            // Enter propose mode if propose_command is configured
                            if config.get_propose_command().is_some() {
                                app.start_proposing();
                            } else {
                                app.warning_message = Some(
                                    "propose_command not configured in .openspec-orchestrator.jsonc"
                                        .to_string(),
                                );
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
                    // Clear warning message on any key press
                    app.warning_message = None;
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
                    if dynamic_queue.remove(&id).await {
                        app.add_log(LogEntry::info(format!(
                            "Removed from queue: {} (also removed from dynamic queue)",
                            id
                        )));
                    } else {
                        app.add_log(LogEntry::info(format!("Removed from queue: {}", id)));
                    }
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
                            let msg = if was_queued {
                                if removed_from_dynamic {
                                    format!(
                                        "Unapproved and removed from queue: {} (also removed from dynamic queue)",
                                        id
                                    )
                                } else {
                                    format!("Unapproved and removed from queue: {}", id)
                                }
                            } else {
                                format!("Unapproved: {}", id)
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
                TuiCommand::SubmitProposal(proposal) => {
                    // Execute propose_command with the proposal text
                    if let Some(template) = config.get_propose_command() {
                        let command = OrchestratorConfig::expand_proposal(template, &proposal);
                        app.add_log(LogEntry::info(format!("Submitting proposal: {}", proposal)));

                        // Suspend TUI and execute command
                        disable_raw_mode()?;
                        execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

                        // Execute the propose command
                        info!("Running propose command: sh -c {}", command);
                        let status = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&command)
                            .status();

                        // Restore TUI
                        enable_raw_mode()?;
                        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
                        terminal.clear()?;

                        match status {
                            Ok(exit_status) if exit_status.success() => {
                                app.add_log(LogEntry::success("Proposal submitted successfully"));
                                // Success: mode was already changed by submit_proposal()
                            }
                            Ok(exit_status) => {
                                app.add_log(LogEntry::error(format!(
                                    "Proposal command failed with exit code: {:?}",
                                    exit_status.code()
                                )));
                                // Failure: return to Proposing mode to allow retry
                                app.mode = AppMode::Proposing;
                                // Recreate textarea with previous input
                                app.propose_textarea = Some(AppState::create_propose_textarea());
                                if let Some(ref mut textarea) = app.propose_textarea {
                                    textarea.insert_str(&proposal);
                                }
                            }
                            Err(e) => {
                                app.add_log(LogEntry::error(format!(
                                    "Failed to execute proposal command: {}",
                                    e
                                )));
                                // Failure: return to Proposing mode to allow retry
                                app.mode = AppMode::Proposing;
                                // Recreate textarea with previous input
                                app.propose_textarea = Some(AppState::create_propose_textarea());
                                if let Some(ref mut textarea) = app.propose_textarea {
                                    textarea.insert_str(&proposal);
                                }
                            }
                        }
                    } else {
                        app.add_log(LogEntry::error("propose_command not configured"));
                    }
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
