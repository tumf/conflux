//! Key event handlers for TUI
//!
//! This module contains helper functions to handle keyboard input in the TUI.

use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, StopMode};
use crate::vcs::VcsResult;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::terminal::{execute_worktree_command, suspend_terminal_and_execute_sync};
use super::worktrees::load_worktrees_with_conflict_check;

#[async_trait]
trait WorktreePlusRuntime {
    async fn check_git_repo(&self, repo_root: &Path) -> VcsResult<bool>;
    async fn generate_unique_branch_name(
        &self,
        repo_root: &Path,
        prefix: &str,
        max_attempts: u32,
    ) -> VcsResult<String>;
    async fn worktree_add(
        &self,
        repo_root: &Path,
        worktree_path: &str,
        branch_name: &str,
        base_commit: &str,
    ) -> VcsResult<()>;
    async fn validate_worktree_command_cwd(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> VcsResult<()>;
    async fn run_worktree_setup(&self, repo_root: &Path, worktree_path: &Path) -> VcsResult<()>;
    async fn worktree_remove_after_setup_failure(
        &self,
        repo_root: &Path,
        worktree_path: &str,
    ) -> VcsResult<()>;
    async fn execute_worktree_command(
        &self,
        terminal: &mut DefaultTerminal,
        command: &str,
        worktree_path: &Path,
        ai_runner: &AiCommandRunner,
        app: &mut AppState,
    ) -> Result<()>;
}

struct ProductionWorktreePlusRuntime;

#[async_trait]
impl WorktreePlusRuntime for ProductionWorktreePlusRuntime {
    async fn check_git_repo(&self, repo_root: &Path) -> VcsResult<bool> {
        crate::vcs::git::commands::check_git_repo(repo_root).await
    }

    async fn generate_unique_branch_name(
        &self,
        repo_root: &Path,
        prefix: &str,
        max_attempts: u32,
    ) -> VcsResult<String> {
        crate::vcs::git::commands::generate_unique_branch_name(repo_root, prefix, max_attempts)
            .await
    }

    async fn worktree_add(
        &self,
        repo_root: &Path,
        worktree_path: &str,
        branch_name: &str,
        base_commit: &str,
    ) -> VcsResult<()> {
        crate::vcs::git::commands::worktree_add(repo_root, worktree_path, branch_name, base_commit)
            .await
    }

    async fn validate_worktree_command_cwd(
        &self,
        repo_root: &Path,
        worktree_path: &Path,
    ) -> VcsResult<()> {
        crate::vcs::git::commands::validate_worktree_command_cwd(repo_root, worktree_path).await
    }

    async fn run_worktree_setup(&self, repo_root: &Path, worktree_path: &Path) -> VcsResult<()> {
        crate::vcs::git::commands::run_worktree_setup(repo_root, worktree_path).await
    }

    async fn worktree_remove_after_setup_failure(
        &self,
        repo_root: &Path,
        worktree_path: &str,
    ) -> VcsResult<()> {
        crate::vcs::git::commands::worktree_remove_with_options(
            repo_root,
            worktree_path,
            crate::vcs::git::commands::WorktreeRemoveOptions {
                skip_teardown: true,
            },
        )
        .await
    }

    async fn execute_worktree_command(
        &self,
        terminal: &mut DefaultTerminal,
        command: &str,
        worktree_path: &Path,
        ai_runner: &AiCommandRunner,
        app: &mut AppState,
    ) -> Result<()> {
        execute_worktree_command(terminal, command, worktree_path, ai_runner, app).await
    }
}

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

fn request_local_tui_quit(app: &mut AppState, orchestrator_cancel: &Option<CancellationToken>) {
    app.should_quit = true;
    if let Some(cancel) = orchestrator_cancel {
        cancel.cancel();
        app.add_log(LogEntry::warn(
            "Quit requested: cancelling local orchestration before TUI shutdown",
        ));
    }
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
    if view_mode == ViewMode::Worktrees && ctx.app.suppress_if_selected_worktree_deleting() {
        ctx.app.add_log(LogEntry::warn(
            "Editor ignored: worktree is already being deleted",
        ));
        return Ok(());
    }

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

fn handle_start_key_inner(
    app: &mut AppState,
    graceful_stop_flag: &AtomicBool,
    orchestrator_handle: &Option<tokio::task::JoinHandle<Result<()>>>,
) -> Option<TuiCommand> {
    // Handle the configured start key in Stopping mode to cancel graceful stop.
    if app.mode == AppMode::Stopping {
        // Check if orchestrator is still running.
        if orchestrator_handle
            .as_ref()
            .is_some_and(|h| !h.is_finished())
        {
            // Cancel graceful stop and return to Running mode.
            graceful_stop_flag.store(false, Ordering::SeqCst);
            app.stop_mode = StopMode::None;
            app.mode = AppMode::Running;
            app.add_log(LogEntry::info("Stop canceled, continuing..."));
        } else {
            // Already stopped, cannot cancel.
            app.add_log(LogEntry::warn(
                "Cannot cancel stop: processing already completed",
            ));
        }
        return None;
    }

    // The configured start key is a cursor-independent orchestration control.
    // It must not inspect the selected row for MergeWait/ResolveWait and must
    // not resolve cursor-local merge waits; Changes-view M is the cursor-local
    // resolve-intent key.
    if app.mode == AppMode::Error {
        app.retry_error_changes()
    } else if app.mode == AppMode::Stopped {
        app.resume_processing()
    } else {
        app.start_processing()
    }
}

/// Handle the configured start key: start, resume, or retry orchestration; or cancel stop.
pub fn handle_start_key(ctx: &mut KeyEventContext<'_>) -> Option<TuiCommand> {
    handle_start_key_inner(ctx.app, ctx.graceful_stop_flag, ctx.orchestrator_handle)
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

    if ctx.app.suppress_if_selected_worktree_deleting() {
        ctx.app.add_log(LogEntry::warn(
            "Enter ignored: worktree is already being deleted",
        ));
        return Ok(());
    }

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
    handle_plus_key_with_runtime(ctx, &ProductionWorktreePlusRuntime).await
}

struct PreparedWorktreeCommand {
    command: String,
    worktree_path: PathBuf,
}

async fn handle_plus_key_with_runtime(
    ctx: &mut KeyEventContext<'_>,
    runtime: &dyn WorktreePlusRuntime,
) -> Result<()> {
    let Some(prepared) = prepare_plus_worktree_command(
        ctx.app,
        ctx.config,
        ctx.repo_root,
        ctx.worktree_base_dir,
        runtime,
    )
    .await
    else {
        return Ok(());
    };

    runtime
        .execute_worktree_command(
            ctx.terminal,
            &prepared.command,
            &prepared.worktree_path,
            ctx.ai_runner,
            ctx.app,
        )
        .await
}

async fn prepare_plus_worktree_command(
    app: &mut AppState,
    config: &OrchestratorConfig,
    repo_root: &Path,
    worktree_base_dir: &Path,
    runtime: &dyn WorktreePlusRuntime,
) -> Option<PreparedWorktreeCommand> {
    use crate::tui::types::ViewMode;

    // Only work in Worktrees view
    if app.view_mode != ViewMode::Worktrees {
        return None;
    }

    let template = config.get_worktree_command().map(str::to_string)?;

    let is_git_repo = match runtime.check_git_repo(repo_root).await {
        Ok(is_repo) => is_repo,
        Err(err) => {
            app.add_log(LogEntry::error(format!(
                "Failed to check git repo: {}",
                err
            )));
            return None;
        }
    };

    if !super::worktrees::should_trigger_worktree_command(config, is_git_repo) {
        return None;
    }

    if let Err(err) = std::fs::create_dir_all(worktree_base_dir) {
        app.add_log(LogEntry::error(format!(
            "Failed to prepare worktree base dir: {}",
            err
        )));
        return None;
    }

    let worktree_path = super::worktrees::build_worktree_path(worktree_base_dir);
    let Some(worktree_path_str) = worktree_path.to_str() else {
        app.add_log(LogEntry::error(
            "Failed to resolve worktree path".to_string(),
        ));
        return None;
    };
    let Some(repo_root_str) = repo_root.to_str() else {
        app.add_log(LogEntry::error(
            "Failed to resolve repo root path".to_string(),
        ));
        return None;
    };

    // Generate unique branch name with format: ws-session-<random>
    let branch_name = match runtime
        .generate_unique_branch_name(repo_root, "ws-session", 10)
        .await
    {
        Ok(name) => name,
        Err(err) => {
            app.add_log(LogEntry::error(format!(
                "Failed to generate unique branch name: {}",
                err
            )));
            return None;
        }
    };

    // Create worktree with branch instead of detached HEAD
    if let Err(err) = runtime
        .worktree_add(repo_root, worktree_path_str, &branch_name, "HEAD")
        .await
    {
        app.add_log(LogEntry::error(format!(
            "Failed to create worktree: {}",
            err
        )));
        return None;
    }

    app.add_log(LogEntry::info(format!(
        "Created worktree with branch '{}' at {}",
        branch_name, worktree_path_str
    )));

    if !validate_plus_worktree_cwd(app, runtime, repo_root, &worktree_path, "after create").await {
        return None;
    }

    // Execute setup script if it exists
    if let Err(err) = runtime.run_worktree_setup(repo_root, &worktree_path).await {
        app.add_log(LogEntry::error(format!(
            "Failed to run worktree setup for {}: {}",
            worktree_path.display(),
            err
        )));
        app.add_log(LogEntry::warn(format!(
            "Cleaning up worktree after setup failure: {}",
            worktree_path.display()
        )));
        match runtime
            .worktree_remove_after_setup_failure(repo_root, worktree_path_str)
            .await
        {
            Ok(()) => app.add_log(LogEntry::info(format!(
                "Cleaned up worktree after setup failure: {}",
                worktree_path.display()
            ))),
            Err(cleanup_err) => app.add_log(LogEntry::error(format!(
                "Failed to cleanup worktree after setup failure at {}: {}",
                worktree_path.display(),
                cleanup_err
            ))),
        }
        return None;
    }

    if !validate_plus_worktree_cwd(app, runtime, repo_root, &worktree_path, "after setup").await {
        return None;
    }

    if !validate_plus_worktree_cwd(
        app,
        runtime,
        repo_root,
        &worktree_path,
        "before command launch",
    )
    .await
    {
        return None;
    }

    let command =
        OrchestratorConfig::expand_worktree_command(&template, worktree_path_str, repo_root_str);
    app.add_log(LogEntry::info(format!(
        "Running worktree command in {}",
        worktree_path_str
    )));

    Some(PreparedWorktreeCommand {
        command,
        worktree_path,
    })
}

async fn validate_plus_worktree_cwd(
    app: &mut AppState,
    runtime: &dyn WorktreePlusRuntime,
    repo_root: &Path,
    worktree_path: &Path,
    phase: &str,
) -> bool {
    match runtime
        .validate_worktree_command_cwd(repo_root, worktree_path)
        .await
    {
        Ok(()) => true,
        Err(err) => {
            app.add_log(LogEntry::error(format!(
                "Suppressing worktree command launch: invalid cwd {} during {}: {}",
                worktree_path.display(),
                phase,
                err
            )));
            false
        }
    }
}

pub(crate) fn handle_warning_popup_key(app: &mut AppState, key: KeyEvent) -> bool {
    if app.warning_popup.is_none() {
        return false;
    }

    match key.code {
        KeyCode::Esc => {
            app.clear_warning_popup();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_warning_popup(-1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_warning_popup(1);
        }
        KeyCode::PageUp => {
            app.scroll_warning_popup(-5);
        }
        KeyCode::PageDown => {
            app.scroll_warning_popup(5);
        }
        _ => {}
    }

    true
}

/// Handle the bulk execution-mark toggle (`x`) in the Changes view.
///
/// Eligibility, mode constraints, and exclusion reporting are enforced in
/// `AppState::toggle_all_marks`. In Running mode, queue commands
/// (AddToQueue/RemoveFromQueue) are emitted for eligible NotQueued/Queued rows,
/// matching single-row Space semantics.
pub(crate) fn handle_bulk_toggle_key(app: &mut AppState) -> Vec<TuiCommand> {
    use crate::tui::types::ViewMode;
    if app.view_mode != ViewMode::Changes {
        return Vec::new();
    }

    app.toggle_all_marks()
}

/// Handle the selected-proposal log filter toggle (`f`) in the Changes view.
///
/// This is presentation-only: it changes which buffered entries the Logs panel
/// renders and never touches execution marks, queue state, or any other
/// workflow-control input. Other views ignore the key.
pub(crate) fn handle_selected_proposal_log_filter_key(app: &mut AppState) {
    use crate::tui::types::ViewMode;
    if app.view_mode != ViewMode::Changes {
        return;
    }

    app.toggle_selected_proposal_log_filter();
}

/// Handle main key events
///
/// Returns Some(TuiCommand) if the key event should trigger the configured start control
pub async fn handle_key_event(
    key: KeyEvent,
    ctx: &mut KeyEventContext<'_>,
) -> Result<Option<TuiCommand>> {
    // Clear the legacy one-line warning message up front so a handler that sets
    // a new warning during this key press keeps it visible.
    ctx.app.warning_message = None;

    if handle_warning_popup_key(ctx.app, key) {
        return Ok(None);
    }

    // Handle QrPopup mode - any key closes the popup
    if ctx.app.mode == AppMode::QrPopup {
        ctx.app.hide_qr_popup();
        return Ok(None);
    }

    // Handle force-kill confirmation mode
    if let AppMode::ConfirmForceKill { ref change_id } = ctx.app.mode {
        let cid = change_id.clone();
        match (key.code, key.modifiers) {
            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                ctx.app.mode = AppMode::Running;
                ctx.app
                    .add_log(LogEntry::info(format!("Force-kill confirmed: {}", cid)));
                let _ = ctx.cmd_tx.send(TuiCommand::DequeueChange(cid)).await;
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                ctx.app.mode = AppMode::Running;
                ctx.app
                    .add_log(LogEntry::info("Force-kill canceled".to_string()));
            }
            _ => {}
        }
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
            (KeyCode::Char('s'), _) | (KeyCode::Char('S'), _) => {
                if let Some(cmd) = ctx.app.confirm_worktree_action_delete_with_options(true) {
                    let _ = ctx.cmd_tx.send(cmd).await;
                }
            }
            _ => {}
        }
        return Ok(None);
    }

    let mut cmd_to_start: Option<TuiCommand> = None;

    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            request_local_tui_quit(ctx.app, ctx.orchestrator_cancel);
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
        (KeyCode::Char('x'), _) => {
            for cmd in handle_bulk_toggle_key(ctx.app) {
                let _ = ctx.cmd_tx.send(cmd).await;
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
        _ if ctx.app.tui_config.matches_start_key(&key) => {
            cmd_to_start = handle_start_key(ctx);
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
        (KeyCode::Char('w'), _) if ctx.app.web_url.is_some() => {
            // Show QR code popup (only if web_url is set)
            ctx.app.show_qr_popup();
        }
        (KeyCode::Char('l'), _) => {
            // Toggle log panel visibility (only in Changes view)
            use crate::tui::types::ViewMode;
            if ctx.app.view_mode == ViewMode::Changes {
                ctx.app.toggle_logs_panel();
            }
        }
        (KeyCode::Char('f'), _) => {
            handle_selected_proposal_log_filter_key(ctx.app);
        }
        (KeyCode::Char('K'), _) => {
            // Enter force-kill confirmation for active changes in Running mode
            use crate::tui::types::ViewMode;
            if ctx.app.view_mode == ViewMode::Changes
                && ctx.app.mode == AppMode::Running
                && !ctx.app.changes.is_empty()
                && ctx.app.cursor_index < ctx.app.changes.len()
            {
                let change = &ctx.app.changes[ctx.app.cursor_index];
                if matches!(
                    change.display_status_cache.as_str(),
                    "applying" | "accepting" | "archiving" | "resolving"
                ) {
                    let cid = change.id.clone();
                    ctx.app.mode = AppMode::ConfirmForceKill {
                        change_id: cid.clone(),
                    };
                    ctx.app.add_log(LogEntry::warn(format!(
                        "Confirm force-kill for '{}': press Y to confirm, N/Esc to cancel",
                        cid
                    )));
                }
            }
        }
        _ => {}
    }

    Ok(cmd_to_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::tui::config::TuiConfig;
    use crate::tui::events::LogLevel;
    use crate::tui::types::ViewMode;
    use crossterm::event::KeyCode;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn inert_stop_flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn log_navigation_state_methods_preserve_existing_key_semantics() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        for index in 0..12 {
            app.add_log(LogEntry::info(format!("log {index}")));
        }
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);

        app.scroll_logs_up(5);
        assert_eq!(app.log_scroll_offset, 5);
        assert!(!app.log_auto_scroll);

        app.scroll_logs_down(5);
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);

        app.scroll_logs_to_top();
        assert_eq!(app.log_scroll_offset, 11);
        assert!(!app.log_auto_scroll);

        app.scroll_logs_to_bottom();
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);

        assert!(app.logs_panel_enabled);
        app.toggle_logs_panel();
        assert!(!app.logs_panel_enabled);
        app.toggle_logs_panel();
        assert!(app.logs_panel_enabled);
    }

    fn log_filter_app() -> AppState {
        let mut app = AppState::new(vec![
            create_test_change("alpha"),
            create_test_change("beta"),
        ]);
        app.logs.clear();
        app.add_log(LogEntry::info("alpha apply").with_change_id("alpha"));
        app.add_log(LogEntry::info("beta apply").with_change_id("beta"));
        app.add_log(LogEntry::info("global orchestration"));
        app
    }

    #[test]
    fn f_key_toggles_selected_proposal_log_filter_in_changes_view() {
        let mut app = log_filter_app();
        assert!(!app.selected_proposal_log_filter);

        handle_selected_proposal_log_filter_key(&mut app);
        assert!(app.selected_proposal_log_filter);
        assert_eq!(app.selected_proposal_log_filter_target(), Some("alpha"));

        handle_selected_proposal_log_filter_key(&mut app);
        assert!(!app.selected_proposal_log_filter);
    }

    #[test]
    fn f_key_does_not_change_execution_marks_cursor_or_log_panel_visibility() {
        let mut app = log_filter_app();
        app.changes[0].selected = true;
        app.changes[1].selected = false;
        let statuses_before: Vec<String> = app
            .changes
            .iter()
            .map(|c| c.display_status_cache.clone())
            .collect();

        handle_selected_proposal_log_filter_key(&mut app);

        assert!(app.changes[0].selected);
        assert!(!app.changes[1].selected);
        assert_eq!(app.cursor_index, 0);
        assert!(app.logs_panel_enabled);
        assert_eq!(app.logs.len(), 3);
        let statuses_after: Vec<String> = app
            .changes
            .iter()
            .map(|c| c.display_status_cache.clone())
            .collect();
        assert_eq!(statuses_before, statuses_after);
    }

    #[test]
    fn f_key_is_ignored_in_worktrees_view() {
        let mut app = log_filter_app();
        app.view_mode = ViewMode::Worktrees;

        handle_selected_proposal_log_filter_key(&mut app);

        assert!(!app.selected_proposal_log_filter);
    }

    #[test]
    fn f_key_leaves_existing_log_panel_and_scroll_keys_intact() {
        let mut app = log_filter_app();

        // `l` semantics are unchanged by the new filter key.
        app.toggle_logs_panel();
        assert!(!app.logs_panel_enabled);
        handle_selected_proposal_log_filter_key(&mut app);
        assert!(!app.logs_panel_enabled);

        // Filtering returns to newest output; PageUp still scrolls back.
        app.scroll_logs_up(5);
        assert!(!app.log_auto_scroll);
        handle_selected_proposal_log_filter_key(&mut app);
        assert_eq!(app.log_scroll_offset, 0);
        assert!(app.log_auto_scroll);
    }

    #[test]
    fn configured_start_key_matches_default_and_custom_bindings() {
        let mut app = AppState::new(vec![create_test_change("run-me")]);
        app.changes[0].selected = true;

        assert!(app.tui_config.matches_start_key(&key(KeyCode::F(5))));
        assert!(app.tui_config.matches_start_key(&key(KeyCode::Char('!'))));

        let custom = TuiConfig::parse_jsonc(
            r#"{"keybindings":{"start":["F5","!"]}}"#,
            std::path::Path::new("/tmp/tui.jsonc"),
        )
        .unwrap();
        app.set_tui_config(custom);

        assert!(app.tui_config.matches_start_key(&key(KeyCode::F(5))));
        assert!(app.tui_config.matches_start_key(&key(KeyCode::Char('!'))));
        assert!(!app.tui_config.matches_start_key(&key(KeyCode::Char('x'))));
    }

    #[test]
    fn configured_start_key_triggers_same_command_as_f5() {
        let graceful_stop = inert_stop_flag();
        let handle = None;
        let custom = TuiConfig::parse_jsonc(
            r#"{"keybindings":{"start":["F5","!"]}}"#,
            std::path::Path::new("/tmp/tui.jsonc"),
        )
        .unwrap();

        let mut f5_app = AppState::new(vec![create_test_change("run-me")]);
        f5_app.set_tui_config(custom.clone());
        f5_app.changes[0].selected = true;
        let f5_command = if f5_app.tui_config.matches_start_key(&key(KeyCode::F(5))) {
            handle_start_key_inner(&mut f5_app, &graceful_stop, &handle)
        } else {
            None
        };

        let mut bang_app = AppState::new(vec![create_test_change("run-me")]);
        bang_app.set_tui_config(custom);
        bang_app.changes[0].selected = true;
        let bang_command = if bang_app
            .tui_config
            .matches_start_key(&key(KeyCode::Char('!')))
        {
            handle_start_key_inner(&mut bang_app, &graceful_stop, &handle)
        } else {
            None
        };

        assert_eq!(format!("{:?}", f5_command), format!("{:?}", bang_command));
        assert!(matches!(
            bang_command,
            Some(TuiCommand::StartProcessing(ids)) if ids == vec!["run-me".to_string()]
        ));
    }

    #[test]
    fn f5_on_merge_wait_row_does_not_emit_resolve_merge() {
        let mut app = AppState::new(vec![
            create_test_change("merge-wait"),
            create_test_change("run-me"),
        ]);
        app.mode = AppMode::Select;
        app.cursor_index = 0;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.changes[1].selected = true;
        let graceful_stop = inert_stop_flag();
        let handle = None;

        let command = handle_start_key_inner(&mut app, &graceful_stop, &handle);

        assert!(
            !matches!(command, Some(TuiCommand::ResolveMerge(_))),
            "F5 must not dispatch cursor-local ResolveMerge for MergeWait rows"
        );
        assert!(matches!(
            command,
            Some(TuiCommand::StartProcessing(ids)) if ids == vec!["run-me".to_string()]
        ));
        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert_eq!(app.changes[1].display_status_cache, "queued");
    }

    #[test]
    fn f5_delegates_start_resume_and_retry_while_resolving() {
        let graceful_stop = inert_stop_flag();
        let handle = None;

        let mut select_app = AppState::new(vec![create_test_change("select-a")]);
        select_app.mode = AppMode::Select;
        select_app.is_resolving = true;
        select_app.changes[0].selected = true;
        let command = handle_start_key_inner(&mut select_app, &graceful_stop, &handle);
        assert!(matches!(
            command,
            Some(TuiCommand::StartProcessing(ids)) if ids == vec!["select-a".to_string()]
        ));
        assert!(select_app.warning_message.is_none());

        let mut stopped_app = AppState::new(vec![create_test_change("stopped-a")]);
        stopped_app.mode = AppMode::Stopped;
        stopped_app.is_resolving = true;
        stopped_app.changes[0].selected = true;
        let command = handle_start_key_inner(&mut stopped_app, &graceful_stop, &handle);
        assert!(matches!(
            command,
            Some(TuiCommand::StartProcessing(ids)) if ids == vec!["stopped-a".to_string()]
        ));
        assert!(stopped_app.warning_message.is_none());

        let mut errobang_app = AppState::new(vec![create_test_change("error-a")]);
        errobang_app.mode = AppMode::Error;
        errobang_app.is_resolving = true;
        errobang_app.changes[0].set_error_message_cache("boom".to_string());
        errobang_app.changes[0].selected = true;
        let shared = std::sync::Arc::new(tokio::sync::RwLock::new(
            crate::orchestration::state::OrchestratorState::new(vec!["error-a".to_string()], 0),
        ));
        shared.blocking_write().apply_execution_event(
            &crate::events::ExecutionEvent::ProcessingError {
                id: "error-a".to_string(),
                error: "boom".to_string(),
            },
        );
        errobang_app.set_shared_state(shared);
        let command = handle_start_key_inner(&mut errobang_app, &graceful_stop, &handle);
        assert!(matches!(
            command,
            Some(TuiCommand::StartProcessing(ids)) if ids == vec!["error-a".to_string()]
        ));
        assert!(errobang_app.warning_message.is_none());
    }

    #[test]
    fn f5_on_merge_wait_with_no_runnable_work_is_noop_not_resolve() {
        let mut app = AppState::new(vec![create_test_change("merge-wait")]);
        app.mode = AppMode::Select;
        app.cursor_index = 0;
        app.changes[0].display_status_cache = "merge wait".to_string();
        let graceful_stop = inert_stop_flag();
        let handle = None;

        let command = handle_start_key_inner(&mut app, &graceful_stop, &handle);

        assert!(command.is_none());
        assert_eq!(app.changes[0].display_status_cache, "merge wait");
        assert_eq!(app.warning_message.as_deref(), Some("No changes selected"));
    }

    #[test]
    fn warning_popup_scroll_keys_do_not_move_underlying_cursor_or_close_popup() {
        let mut app = AppState::new(vec![create_test_change("a"), create_test_change("b")]);
        app.show_warning_popup("warning", "line 1\nline 2\nline 3");
        let cursor_before = app.cursor_index;

        assert!(handle_warning_popup_key(&mut app, key(KeyCode::Down)));

        assert_eq!(app.cursor_index, cursor_before);
        assert_eq!(app.warning_popup_scroll, 1);
        assert!(app.warning_popup.is_some());
    }

    #[test]
    fn warning_popup_close_key_clears_popup_and_resets_scroll() {
        let mut app = AppState::new(vec![create_test_change("a")]);
        app.show_warning_popup("warning", "diagnostic");
        app.warning_popup_scroll = 7;

        assert!(handle_warning_popup_key(&mut app, key(KeyCode::Esc)));

        assert!(app.warning_popup.is_none());
        assert_eq!(app.warning_popup_scroll, 0);
    }

    #[test]
    fn warning_popup_ignores_non_popup_keys_without_underlying_action() {
        let mut app = AppState::new(vec![create_test_change("a"), create_test_change("b")]);
        app.show_warning_popup("warning", "diagnostic");
        let cursor_before = app.cursor_index;

        assert!(handle_warning_popup_key(&mut app, key(KeyCode::Char('x'))));

        assert_eq!(app.cursor_index, cursor_before);
        assert_eq!(app.warning_popup_scroll, 0);
        assert!(app.warning_popup.is_some());
    }
    struct StubPlusRuntime {
        is_git_repo: bool,
        branch_name: String,
        fail_validation_at: Option<usize>,
        setup_error: Option<String>,
        cleanup_error: Option<String>,
        worktree_add_calls: std::sync::atomic::AtomicUsize,
        validation_calls: std::sync::atomic::AtomicUsize,
        setup_calls: std::sync::atomic::AtomicUsize,
        cleanup_calls: std::sync::atomic::AtomicUsize,
        execute_calls: std::sync::atomic::AtomicUsize,
    }

    impl StubPlusRuntime {
        fn new() -> Self {
            Self {
                is_git_repo: true,
                branch_name: "ws-session-test".to_string(),
                fail_validation_at: None,
                setup_error: None,
                cleanup_error: None,
                worktree_add_calls: std::sync::atomic::AtomicUsize::new(0),
                validation_calls: std::sync::atomic::AtomicUsize::new(0),
                setup_calls: std::sync::atomic::AtomicUsize::new(0),
                cleanup_calls: std::sync::atomic::AtomicUsize::new(0),
                execute_calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl WorktreePlusRuntime for StubPlusRuntime {
        async fn check_git_repo(&self, _repo_root: &Path) -> VcsResult<bool> {
            Ok(self.is_git_repo)
        }

        async fn generate_unique_branch_name(
            &self,
            _repo_root: &Path,
            _prefix: &str,
            _max_attempts: u32,
        ) -> VcsResult<String> {
            Ok(self.branch_name.clone())
        }

        async fn worktree_add(
            &self,
            _repo_root: &Path,
            worktree_path: &str,
            _branch_name: &str,
            _base_commit: &str,
        ) -> VcsResult<()> {
            self.worktree_add_calls.fetch_add(1, Ordering::SeqCst);
            std::fs::create_dir_all(worktree_path).unwrap();
            Ok(())
        }

        async fn validate_worktree_command_cwd(
            &self,
            _repo_root: &Path,
            _worktree_path: &Path,
        ) -> VcsResult<()> {
            let call = self.validation_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_validation_at == Some(call) {
                return Err(crate::vcs::VcsError::git_command(format!(
                    "validation failed at call {call}"
                )));
            }
            Ok(())
        }

        async fn run_worktree_setup(
            &self,
            _repo_root: &Path,
            _worktree_path: &Path,
        ) -> VcsResult<()> {
            self.setup_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(message) = &self.setup_error {
                return Err(crate::vcs::VcsError::git_command(message.clone()));
            }
            Ok(())
        }

        async fn worktree_remove_after_setup_failure(
            &self,
            _repo_root: &Path,
            _worktree_path: &str,
        ) -> VcsResult<()> {
            self.cleanup_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(message) = &self.cleanup_error {
                return Err(crate::vcs::VcsError::git_command(message.clone()));
            }
            Ok(())
        }

        async fn execute_worktree_command(
            &self,
            _terminal: &mut DefaultTerminal,
            _command: &str,
            _worktree_path: &Path,
            _ai_runner: &AiCommandRunner,
            _app: &mut AppState,
        ) -> Result<()> {
            self.execute_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn plus_config(template: &str) -> OrchestratorConfig {
        OrchestratorConfig {
            worktree_command: Some(template.to_string()),
            ..Default::default()
        }
    }

    fn worktrees_app() -> AppState {
        let mut app = AppState::new(vec![]);
        app.view_mode = crate::tui::types::ViewMode::Worktrees;
        app
    }

    #[tokio::test]
    async fn plus_prepare_suppresses_setup_when_created_worktree_validation_fails() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let config = plus_config("true");
        let runtime = StubPlusRuntime {
            fail_validation_at: Some(1),
            ..StubPlusRuntime::new()
        };

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &temp_dir.path().join("worktrees"),
            &runtime,
        )
        .await;

        assert!(prepared.is_none());
        assert_eq!(runtime.worktree_add_calls.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.setup_calls.load(Ordering::SeqCst), 0);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 0);
        assert!(app.logs.iter().any(|entry| {
            entry
                .message
                .contains("Suppressing worktree command launch: invalid cwd")
                && entry.message.contains("after create")
        }));
    }

    #[tokio::test]
    async fn plus_prepare_suppresses_command_when_worktree_invalid_after_setup() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let config = plus_config("true");
        let runtime = StubPlusRuntime {
            fail_validation_at: Some(2),
            ..StubPlusRuntime::new()
        };

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &temp_dir.path().join("worktrees"),
            &runtime,
        )
        .await;

        assert!(prepared.is_none());
        assert_eq!(runtime.setup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 0);
        assert!(app.logs.iter().any(|entry| {
            entry
                .message
                .contains("Suppressing worktree command launch: invalid cwd")
                && entry.message.contains("after setup")
        }));
    }

    #[tokio::test]
    async fn plus_prepare_suppresses_command_when_worktree_invalid_before_launch() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let config = plus_config("true");
        let runtime = StubPlusRuntime {
            fail_validation_at: Some(3),
            ..StubPlusRuntime::new()
        };

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &temp_dir.path().join("worktrees"),
            &runtime,
        )
        .await;

        assert!(prepared.is_none());
        assert_eq!(runtime.setup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 0);
        assert!(app.logs.iter().any(|entry| {
            entry
                .message
                .contains("Suppressing worktree command launch: invalid cwd")
                && entry.message.contains("before command launch")
        }));
    }

    #[tokio::test]
    async fn plus_prepare_logs_setup_failure_cleanup_and_suppresses_command() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let config = plus_config("true");
        let runtime = StubPlusRuntime {
            setup_error: Some("setup exploded".to_string()),
            ..StubPlusRuntime::new()
        };

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &temp_dir.path().join("worktrees"),
            &runtime,
        )
        .await;

        assert!(prepared.is_none());
        assert_eq!(runtime.cleanup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 0);
        assert!(app.logs.iter().any(|entry| entry
            .message
            .contains("Failed to run worktree setup")
            && entry.message.contains("setup exploded")));
        assert!(app.logs.iter().any(|entry| entry
            .message
            .contains("Cleaning up worktree after setup failure")));
        assert!(app.logs.iter().any(|entry| entry
            .message
            .contains("Cleaned up worktree after setup failure")));
    }

    async fn init_plus_git_repo(repo: &Path) {
        let init = tokio::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        assert!(init.status.success(), "git init failed: {init:?}");
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        std::fs::write(repo.join("README.md"), "test").unwrap();
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        let commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        assert!(commit.status.success(), "git commit failed: {commit:?}");
    }

    #[tokio::test]
    async fn plus_prepare_returns_expanded_command_for_valid_worktree() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let config = plus_config("tmux new-window -n wt -c {workspace_dir} -- echo {repo_root}");
        let runtime = StubPlusRuntime::new();

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &temp_dir.path().join("worktrees"),
            &runtime,
        )
        .await
        .expect("valid worktree should prepare command launch");

        assert_eq!(runtime.validation_calls.load(Ordering::SeqCst), 3);
        assert_eq!(runtime.setup_calls.load(Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            prepared.worktree_path.parent().unwrap(),
            temp_dir.path().join("worktrees")
        );
        assert!(prepared.command.contains("tmux new-window -n wt -c"));
        assert!(prepared
            .command
            .contains(prepared.worktree_path.to_str().unwrap()));
        assert!(prepared.command.contains(temp_dir.path().to_str().unwrap()));
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Running worktree command in")));
    }

    fn test_ai_runner() -> AiCommandRunner {
        let queue_config = crate::command_queue::CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: Vec::new(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 0,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        AiCommandRunner::new(queue_config, Arc::new(tokio::sync::Mutex::new(None)))
    }

    #[tokio::test]
    async fn plus_handle_invokes_command_runner_boundary_for_valid_worktree() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut app = worktrees_app();
        let mut terminal = ratatui::Terminal::with_options(
            ratatui::backend::CrosstermBackend::new(std::io::stdout()),
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 80, 24)),
            },
        )
        .unwrap();
        let config = plus_config("echo {workspace_dir}");
        let (tx, _rx) = mpsc::channel(1);
        let (cmd_tx, _cmd_rx) = mpsc::channel(1);
        let ai_runner = test_ai_runner();
        let graceful_stop_flag = inert_stop_flag();
        let runtime = StubPlusRuntime::new();
        let worktree_base_dir = temp_dir.path().join("worktrees");
        let orchestrator_cancel = None;
        let orchestrator_handle = None;
        let mut ctx = KeyEventContext {
            app: &mut app,
            terminal: &mut terminal,
            repo_root: temp_dir.path(),
            config: &config,
            worktree_base_dir: &worktree_base_dir,
            tx: &tx,
            cmd_tx: &cmd_tx,
            ai_runner: &ai_runner,
            graceful_stop_flag: &graceful_stop_flag,
            orchestrator_cancel: &orchestrator_cancel,
            orchestrator_handle: &orchestrator_handle,
        };

        handle_plus_key_with_runtime(&mut ctx, &runtime)
            .await
            .unwrap();

        assert_eq!(runtime.validation_calls.load(Ordering::SeqCst), 3);
        assert_eq!(runtime.execute_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn plus_prepare_with_production_runtime_creates_registered_materialized_worktree() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        init_plus_git_repo(temp_dir.path()).await;
        let mut app = worktrees_app();
        let config = plus_config("printf ok");
        let worktree_base_dir = temp_dir.path().join("worktrees");

        let prepared = prepare_plus_worktree_command(
            &mut app,
            &config,
            temp_dir.path(),
            &worktree_base_dir,
            &ProductionWorktreePlusRuntime,
        )
        .await
        .expect("valid Git fixture should prepare command launch");

        assert!(prepared.worktree_path.exists());
        assert!(prepared.worktree_path.join(".git").exists());
        assert!(prepared.worktree_path.join("README.md").exists());
        assert_eq!(prepared.command, "printf ok");
        crate::vcs::git::commands::validate_worktree_command_cwd(
            temp_dir.path(),
            &prepared.worktree_path,
        )
        .await
        .unwrap();
        let _ = crate::vcs::git::commands::worktree_remove(
            temp_dir.path(),
            prepared.worktree_path.to_str().unwrap(),
        )
        .await;
    }

    /// Builds a Changes-view app for bulk toggle boundary tests.
    fn bulk_toggle_app(rows: &[(&str, &str, bool)]) -> AppState {
        let changes = rows
            .iter()
            .map(|(id, _, _)| create_test_change(id))
            .collect();
        let mut app = AppState::new(changes);
        app.mode = AppMode::Running;
        for (index, (_, status, selected)) in rows.iter().enumerate() {
            app.changes[index].display_status_cache = status.to_string();
            app.changes[index].selected = *selected;
        }
        app
    }

    #[test]
    fn bulk_toggle_key_surfaces_excluded_rows_with_reasons() {
        let mut app = bulk_toggle_app(&[
            ("active", "applying", false),
            ("rejected", "rejected", false),
            ("eligible", "not queued", false),
        ]);

        let commands = handle_bulk_toggle_key(&mut app);

        assert!(matches!(&commands[..], [TuiCommand::AddToQueue(id)] if id == "eligible"));
        assert!(app.changes[2].selected);
        assert!(!app.changes[0].selected);
        assert!(!app.changes[1].selected);

        let warning = app
            .warning_message
            .as_ref()
            .expect("x must surface excluded rows in the Changes view");
        assert!(
            warning.contains("2 excluded")
                && warning.contains("in progress")
                && warning.contains("rejected"),
            "warning must explain the exclusions: {}",
            warning
        );
        assert!(app.logs.iter().any(|entry| entry.message == *warning));
    }

    #[test]
    fn bulk_toggle_key_with_zero_eligible_targets_is_not_silent() {
        let mut app = bulk_toggle_app(&[
            ("active", "applying", false),
            ("rejected", "rejected", true),
        ]);

        let commands = handle_bulk_toggle_key(&mut app);

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected);
        assert!(app.changes[1].selected, "ineligible rows must not change");
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("no eligible changes")));
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.level == LogLevel::Warn && entry.message.contains("no eligible")));
    }

    #[test]
    fn bulk_toggle_key_is_ignored_outside_changes_view() {
        let mut app = bulk_toggle_app(&[("eligible", "not queued", false)]);
        app.view_mode = ViewMode::Worktrees;

        let commands = handle_bulk_toggle_key(&mut app);

        assert!(commands.is_empty());
        assert!(!app.changes[0].selected);
        assert!(app.warning_message.is_none());
    }

    #[test]
    fn ctrl_c_quit_cancels_local_orchestrator_token() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let token = CancellationToken::new();

        request_local_tui_quit(&mut app, &Some(token.clone()));

        assert!(app.should_quit);
        assert!(token.is_cancelled());
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("cancelling local orchestration")));
    }

    #[test]
    fn ctrl_c_quit_without_local_orchestrator_only_sets_quit() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        request_local_tui_quit(&mut app, &None);

        assert!(app.should_quit);
    }
}
