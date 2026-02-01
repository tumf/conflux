//! Key event handlers for TUI
//!
//! This module contains helper functions to handle keyboard input in the TUI.

use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, StopMode};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::runner::{
    execute_worktree_command, load_worktrees_with_conflict_check, suspend_terminal_and_execute_sync,
};

/// Context for key event handling containing necessary state and channels
pub struct KeyEventContext<'a> {
    pub app: &'a mut AppState,
    pub terminal: &'a mut DefaultTerminal,
    pub repo_root: &'a Path,
    pub config: &'a OrchestratorConfig,
    pub worktree_base_dir: &'a Path,
    pub tx: &'a mpsc::Sender<OrchestratorEvent>,
    pub cmd_tx: &'a mpsc::Sender<TuiCommand>,
    pub ai_runner: &'a AiCommandRunner,
    pub graceful_stop_flag: &'a Arc<AtomicBool>,
    pub orchestrator_cancel: &'a Option<CancellationToken>,
    pub orchestrator_handle: &'a Option<tokio::task::JoinHandle<Result<()>>>,
}

/// Handle Tab key: Switch between Changes and Worktrees views
pub async fn handle_tab_key(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;
    let new_view = match ctx.app.view_mode {
        ViewMode::Changes => ViewMode::Worktrees,
        ViewMode::Worktrees => ViewMode::Changes,
    };

    // Load worktrees with conflict check when switching to Worktrees view
    if new_view == ViewMode::Worktrees {
        let load_tx = ctx.tx.clone();
        let load_repo_root = ctx.repo_root.to_path_buf();
        tokio::spawn(async move {
            match load_worktrees_with_conflict_check(&load_repo_root).await {
                Ok(worktrees) => {
                    let _ = load_tx
                        .send(OrchestratorEvent::WorktreesRefreshed { worktrees })
                        .await;
                }
                Err(e) => {
                    let _ = load_tx
                        .send(OrchestratorEvent::Log(LogEntry::error(format!(
                            "Failed to load worktrees: {}",
                            e
                        ))))
                        .await;
                }
            }
        });
    }

    ctx.app.view_mode = new_view;
    Ok(())
}

/// Handle cursor movement keys (Up/Down/k/j)
pub fn handle_cursor_movement(app: &mut AppState, is_up: bool) {
    use crate::tui::types::ViewMode;
    match app.view_mode {
        ViewMode::Changes => {
            if is_up {
                app.cursor_up()
            } else {
                app.cursor_down()
            }
        }
        ViewMode::Worktrees => {
            if is_up {
                app.worktree_cursor_up()
            } else {
                app.worktree_cursor_down()
            }
        }
    }
}

/// Handle 'e' key: Launch editor for change or worktree
pub async fn handle_editor_launch(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;

    let view_mode = ctx.app.view_mode;
    let change_id = if !ctx.app.changes.is_empty() && ctx.app.cursor_index < ctx.app.changes.len() {
        Some(ctx.app.changes[ctx.app.cursor_index].id.clone())
    } else {
        None
    };
    let worktree_path = ctx.app.get_selected_worktree_path();

    suspend_terminal_and_execute_sync(ctx.terminal, || {
        // Launch editor based on view mode
        match view_mode {
            ViewMode::Changes => {
                if let Some(id) = change_id {
                    if let Err(e) = crate::tui::utils::launch_editor_for_change(&id) {
                        eprintln!("Failed to launch editor: {}", e);
                    }
                }
            }
            ViewMode::Worktrees => {
                if let Some(path) = worktree_path {
                    if let Err(e) = crate::tui::utils::launch_editor_in_dir(&path) {
                        eprintln!("Failed to launch editor: {}", e);
                    }
                }
            }
        }
        Ok(())
    })
}

/// Handle 'M' key: Merge operations (resolve in Changes view, merge in Worktrees view)
pub async fn handle_merge_key(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;

    debug!("M key pressed: view_mode={:?}", ctx.app.view_mode);

    match ctx.app.view_mode {
        ViewMode::Changes => {
            // Changes view: resolve deferred merge
            debug!("M key (Changes view): attempting resolve_merge");
            if let Some(cmd) = ctx.app.resolve_merge() {
                debug!("M key (Changes view): sending command {:?}", cmd);
                let _ = ctx.cmd_tx.send(cmd).await;
            } else {
                debug!("M key (Changes view): resolve_merge returned None");
            }
        }
        ViewMode::Worktrees => {
            // Worktrees view: merge branch to base
            debug!("M key (Worktrees view): attempting request_merge_worktree_branch");
            if let Some(cmd) = ctx.app.request_merge_worktree_branch() {
                debug!("M key (Worktrees view): sending command {:?}", cmd);
                let _ = ctx.cmd_tx.send(cmd).await;
            } else {
                debug!("M key (Worktrees view): request_merge_worktree_branch returned None");
            }
        }
    }
    Ok(())
}

/// Handle Esc key: Graceful stop or force stop
pub fn handle_esc_key(ctx: &mut KeyEventContext<'_>) {
    // Handle stop in Running or Stopping mode
    match ctx.app.mode {
        AppMode::Running => {
            // First Esc: Graceful stop
            ctx.app.stop_mode = StopMode::GracefulPending;
            ctx.graceful_stop_flag.store(true, Ordering::SeqCst);
            ctx.app.mode = AppMode::Stopping;
            ctx.app
                .add_log(LogEntry::warn("Stopping after current change completes..."));
        }
        AppMode::Stopping => {
            // Second Esc: Force stop
            ctx.app.stop_mode = StopMode::ForceStopped;
            if let Some(cancel) = ctx.orchestrator_cancel {
                cancel.cancel();
            }
            // Use OrchestratorEvent::Stopped to properly reset queue status
            // and preserve execution marks (same as graceful stop)
            ctx.app
                .handle_orchestrator_event(OrchestratorEvent::Stopped);
            ctx.app.current_change = None;
            ctx.app.add_log(LogEntry::warn("Force stopped"));
        }
        _ => {}
    }
}

/// Handle F5 key: Start, resume, or retry processing; or cancel stop
/// Prioritizes resolve for MergeWait changes over starting/resuming processing
pub fn handle_f5_key(ctx: &mut KeyEventContext<'_>) -> Option<TuiCommand> {
    use super::types::QueueStatus;

    // Handle F5 in Stopping mode to cancel graceful stop
    if ctx.app.mode == AppMode::Stopping {
        // Check if orchestrator is still running
        if ctx
            .orchestrator_handle
            .as_ref()
            .is_some_and(|h| !h.is_finished())
        {
            // Cancel graceful stop and return to Running mode
            ctx.graceful_stop_flag.store(false, Ordering::SeqCst);
            ctx.app.stop_mode = StopMode::None;
            ctx.app.mode = AppMode::Running;
            ctx.app
                .add_log(LogEntry::info("Stop canceled, continuing..."));
        } else {
            // Already stopped, cannot cancel
            ctx.app.add_log(LogEntry::warn(
                "Cannot cancel stop: processing already completed",
            ));
        }
        return None;
    }

    // Prioritize resolve for MergeWait changes
    // Check if cursor is on a MergeWait change
    if !ctx.app.changes.is_empty() && ctx.app.cursor_index < ctx.app.changes.len() {
        let change = &ctx.app.changes[ctx.app.cursor_index];
        if matches!(change.queue_status, QueueStatus::MergeWait) {
            // F5 on MergeWait change triggers resolve, not start processing
            return ctx.app.resolve_merge();
        }
    }

    // Determine which command to use based on mode
    if ctx.app.mode == AppMode::Error {
        ctx.app.retry_error_changes()
    } else if ctx.app.mode == AppMode::Stopped {
        ctx.app.resume_processing()
    } else {
        ctx.app.start_processing()
    }
}

/// Handle Enter key: Execute worktree command in selected worktree
pub async fn handle_enter_key(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;

    if ctx.app.view_mode != ViewMode::Worktrees {
        ctx.app
            .add_log(LogEntry::warn("Enter ignored: not in Worktrees view"));
        return Ok(());
    }

    let Some(worktree_path_str) = ctx.app.get_selected_worktree_path() else {
        ctx.app
            .add_log(LogEntry::warn("Enter ignored: no worktree selected"));
        return Ok(());
    };

    let Some(template) = ctx.config.get_worktree_command().map(str::to_string) else {
        ctx.app.add_log(LogEntry::warn(
            "Enter ignored: worktree_command not configured",
        ));
        return Ok(());
    };

    let Some(repo_root_str) = ctx.repo_root.to_str() else {
        ctx.app.add_log(LogEntry::error(
            "Failed to resolve repo root path".to_string(),
        ));
        return Ok(());
    };

    let command =
        OrchestratorConfig::expand_worktree_command(&template, &worktree_path_str, repo_root_str);

    ctx.app.add_log(LogEntry::info(format!(
        "Running worktree command in {}",
        worktree_path_str
    )));

    let worktree_path = Path::new(&worktree_path_str);
    execute_worktree_command(
        ctx.terminal,
        &command,
        worktree_path,
        ctx.ai_runner,
        ctx.app,
    )
    .await
}

/// Handle '+' key: Create new worktree and execute worktree command
pub async fn handle_plus_key(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;

    // Only work in Worktrees view
    if ctx.app.view_mode != ViewMode::Worktrees {
        return Ok(());
    }

    let Some(template) = ctx.config.get_worktree_command().map(str::to_string) else {
        return Ok(());
    };

    let is_git_repo = match crate::vcs::git::commands::check_git_repo(ctx.repo_root).await {
        Ok(is_repo) => is_repo,
        Err(err) => {
            ctx.app.add_log(LogEntry::error(format!(
                "Failed to check git repo: {}",
                err
            )));
            return Ok(());
        }
    };

    if !super::runner::should_trigger_worktree_command(ctx.config, is_git_repo) {
        return Ok(());
    }

    if let Err(err) = std::fs::create_dir_all(ctx.worktree_base_dir) {
        ctx.app.add_log(LogEntry::error(format!(
            "Failed to prepare worktree base dir: {}",
            err
        )));
        return Ok(());
    }

    let worktree_path = super::runner::build_worktree_path(ctx.worktree_base_dir);
    let Some(worktree_path_str) = worktree_path.to_str() else {
        ctx.app.add_log(LogEntry::error(
            "Failed to resolve worktree path".to_string(),
        ));
        return Ok(());
    };
    let Some(repo_root_str) = ctx.repo_root.to_str() else {
        ctx.app.add_log(LogEntry::error(
            "Failed to resolve repo root path".to_string(),
        ));
        return Ok(());
    };

    // Generate unique branch name with format: ws-session-<timestamp>
    let branch_name = match crate::vcs::git::commands::generate_unique_branch_name(
        ctx.repo_root,
        "ws-session",
        10,
    )
    .await
    {
        Ok(name) => name,
        Err(err) => {
            ctx.app.add_log(LogEntry::error(format!(
                "Failed to generate unique branch name: {}",
                err
            )));
            return Ok(());
        }
    };

    // Create worktree with branch instead of detached HEAD
    if let Err(err) = crate::vcs::git::commands::worktree_add(
        ctx.repo_root,
        worktree_path_str,
        &branch_name,
        "HEAD",
    )
    .await
    {
        ctx.app.add_log(LogEntry::error(format!(
            "Failed to create worktree: {}",
            err
        )));
        return Ok(());
    }

    ctx.app.add_log(LogEntry::info(format!(
        "Created worktree with branch '{}'",
        branch_name
    )));

    // Execute setup script if it exists
    if let Err(err) =
        crate::vcs::git::commands::run_worktree_setup(ctx.repo_root, &worktree_path).await
    {
        ctx.app.add_log(LogEntry::error(format!(
            "Failed to run worktree setup: {}",
            err
        )));
        // Don't continue - setup failure is considered an error
        // but the worktree was already created, so we should clean it up
        if let Err(cleanup_err) =
            crate::vcs::git::commands::worktree_remove(ctx.repo_root, worktree_path_str).await
        {
            ctx.app.add_log(LogEntry::error(format!(
                "Failed to cleanup worktree after setup failure: {}",
                cleanup_err
            )));
        }
        return Ok(());
    }

    let command =
        OrchestratorConfig::expand_worktree_command(&template, worktree_path_str, repo_root_str);
    ctx.app.add_log(LogEntry::info(format!(
        "Running worktree command in {}",
        worktree_path_str
    )));

    execute_worktree_command(
        ctx.terminal,
        &command,
        &worktree_path,
        ctx.ai_runner,
        ctx.app,
    )
    .await
}

/// Handle main key events
///
/// Returns Some(TuiCommand) if the key event should trigger an orchestrator start (F5 key)
pub async fn handle_key_event(
    key: KeyEvent,
    ctx: &mut KeyEventContext<'_>,
) -> Result<Option<TuiCommand>> {
    let had_warning_message = ctx.app.warning_message.is_some();
    let had_warning_popup = ctx.app.warning_popup.is_some();

    // Handle QrPopup mode - any key closes the popup
    if ctx.app.mode == AppMode::QrPopup {
        ctx.app.hide_qr_popup();
        return Ok(None);
    }

    // Handle worktree delete confirmation
    if ctx.app.mode == AppMode::ConfirmWorktreeDelete {
        match (key.code, key.modifiers) {
            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                if let Some(cmd) = ctx.app.confirm_worktree_action_delete() {
                    let _ = ctx.cmd_tx.send(cmd).await;
                }
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                ctx.app.cancel_worktree_action();
            }
            _ => {}
        }
        return Ok(None);
    }

    let mut cmd_to_start: Option<TuiCommand> = None;

    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            ctx.app.should_quit = true;
        }
        (KeyCode::Tab, _) => {
            handle_tab_key(ctx).await?;
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            handle_cursor_movement(ctx.app, true);
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            handle_cursor_movement(ctx.app, false);
        }
        (KeyCode::Char(' '), _) => {
            if let Some(cmd) = ctx.app.toggle_selection() {
                let _ = ctx.cmd_tx.send(cmd).await;
            }
        }
        (KeyCode::Char('@'), _) => {
            // Toggle approval status
            debug!(
                "@ key pressed: mode={:?}, view_mode={:?}, cursor_index={}, changes_len={}",
                ctx.app.mode,
                ctx.app.view_mode,
                ctx.app.cursor_index,
                ctx.app.changes.len()
            );
            if let Some(cmd) = ctx.app.toggle_approval() {
                debug!("toggle_approval returned: {:?}", cmd);
                let _ = ctx.cmd_tx.send(cmd).await;
            } else {
                debug!("toggle_approval returned None");
            }
        }
        (KeyCode::Char('e'), _) => {
            handle_editor_launch(ctx).await?;
        }
        (KeyCode::Char('m'), _) | (KeyCode::Char('M'), _) => {
            handle_merge_key(ctx).await?;
        }
        (KeyCode::Char('d'), _) | (KeyCode::Char('D'), _) => {
            use crate::tui::types::ViewMode;
            if ctx.app.view_mode == ViewMode::Worktrees {
                // Worktree view: delete selected worktree
                ctx.app.request_worktree_delete_from_list();
            }
            // Note: D key removed from Changes view as per spec
        }
        (KeyCode::Esc, _) => {
            handle_esc_key(ctx);
        }
        (KeyCode::F(5), _) => {
            cmd_to_start = handle_f5_key(ctx);
        }
        (KeyCode::PageUp, _) => {
            // Scroll logs up (show older entries)
            ctx.app.scroll_logs_up(5);
        }
        (KeyCode::PageDown, _) => {
            // Scroll logs down (show newer entries)
            ctx.app.scroll_logs_down(5);
        }
        (KeyCode::Home, _) => {
            // Jump to oldest log entry
            ctx.app.scroll_logs_to_top();
        }
        (KeyCode::End, _) => {
            // Jump to newest log entry and re-enable auto-scroll
            ctx.app.scroll_logs_to_bottom();
        }
        (KeyCode::Char('='), _) => {
            // Toggle parallel mode (only if git is available)
            ctx.app.toggle_parallel_mode();
        }
        (KeyCode::Enter, _) => {
            handle_enter_key(ctx).await?;
        }
        (KeyCode::Char('+'), _) => {
            handle_plus_key(ctx).await?;
        }
        (KeyCode::Char('w'), _) => {
            // Show QR code popup (only if web_url is set)
            if ctx.app.web_url.is_some() {
                ctx.app.show_qr_popup();
            }
        }
        _ => {}
    }

    // Clear previous warning message on any key press
    if had_warning_message || had_warning_popup {
        ctx.app.warning_message = None;
        ctx.app.warning_popup = None;
    }

    Ok(cmd_to_start)
}
