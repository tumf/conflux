//! Key event handlers for TUI
//!
//! This module contains helper functions to handle keyboard input in the TUI.

use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::state::AppState;
use crate::tui::types::{AppExecutionMode, ModalState, StopMode};
use crate::vcs::VcsResult;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
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
        // The TUI-created worktree path only needs success or the actionable
        // failure; the setup report's diagnostics are already logged.
        crate::vcs::git::commands::run_worktree_setup(repo_root, worktree_path)
            .await
            .map(|_report| ())
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
    /// The local run supervisor, shared with the run-control service.
    pub supervisor: &'a Arc<crate::tui::run_supervisor::TuiRunSupervisor>,
}

fn request_local_tui_quit(
    app: &mut AppState,
    supervisor: &crate::tui::run_supervisor::TuiRunSupervisor,
) {
    app.should_quit = true;
    if let Some(cancel) = supervisor.cancel_token() {
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

/// What the Esc key must do for the current typed TUI state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EscStopAction {
    /// First Esc: request a graceful stop at the next safe boundary.
    RequestGracefulStop,
    /// Second Esc: enqueue the shared immediate-stop command.
    ///
    /// The key path never applies stop effects itself, so it cannot claim a
    /// force stop from `AppExecutionMode::Stopping` alone.
    RequestImmediateStop,
    /// Esc is not a stop control in this state, or an immediate stop was already
    /// requested and must not be duplicated.
    None,
}

/// Decide the Esc stop action from typed TUI state only.
pub(crate) fn esc_stop_action(mode: &AppExecutionMode, stop_mode: &StopMode) -> EscStopAction {
    match mode {
        AppExecutionMode::Running => EscStopAction::RequestGracefulStop,
        AppExecutionMode::Stopping if *stop_mode == StopMode::ImmediatePending => {
            EscStopAction::None
        }
        AppExecutionMode::Stopping => EscStopAction::RequestImmediateStop,
        _ => EscStopAction::None,
    }
}

pub(crate) async fn handle_esc_key_inner(app: &mut AppState, cmd_tx: &mpsc::Sender<TuiCommand>) {
    match esc_stop_action(&app.execution_mode, &app.stop_mode) {
        EscStopAction::RequestGracefulStop => {
            // The stop effect itself belongs to the shared service, so the first
            // Esc enqueues the same command a remote `stop` submits. Recording
            // the pending mode here is what makes the second Esc an immediate
            // stop rather than a duplicate graceful request.
            app.stop_mode = StopMode::GracefulPending;
            let _ = cmd_tx.send(TuiCommand::Stop).await;
        }
        EscStopAction::RequestImmediateStop => {
            app.stop_mode = StopMode::ImmediatePending;
            let _ = cmd_tx.send(TuiCommand::ForceStop).await;
        }
        EscStopAction::None => {}
    }
}

/// Handle Esc key: Graceful stop or immediate stop
///
/// The second Esc does not apply stop effects here. It enqueues the shared
/// [`TuiCommand::ForceStop`] so both the key path and the command-dispatch path
/// consume one runtime activity snapshot, issue one cancellation request, and
/// report force stop only when an agent execution was actually active.
pub async fn handle_esc_key(ctx: &mut KeyEventContext<'_>) {
    handle_esc_key_inner(ctx.app, ctx.cmd_tx).await;
}

fn handle_start_key_inner(app: &mut AppState) -> Option<TuiCommand> {
    // Handle the configured start key in Stopping mode to cancel graceful stop.
    // Whether a cancellable run still exists is a runtime fact the shared service
    // owns, so the key path only expresses the intent.
    if app.execution_mode == AppExecutionMode::Stopping {
        return Some(TuiCommand::CancelStop);
    }

    // The configured start key is a cursor-independent orchestration control.
    // It must not inspect the selected row for MergeWait/ResolveWait and must
    // not resolve cursor-local merge waits; Changes-view M is the cursor-local
    // resolve-intent key. Which of start/resume/retry applies is decided by the
    // shared service from the mode it is given.
    Some(TuiCommand::StartProcessing(Vec::new()))
}

/// Handle the configured start key: start, resume, or retry orchestration; or cancel stop.
pub fn handle_start_key(ctx: &mut KeyEventContext<'_>) -> Option<TuiCommand> {
    handle_start_key_inner(ctx.app)
}

/// Handle Enter key: Execute worktree command in selected worktree
pub async fn handle_enter_key(ctx: &mut KeyEventContext<'_>) -> Result<()> {
    use crate::tui::types::ViewMode;

    if ctx.app.view_mode != ViewMode::Worktrees {
        // Enter on an `error` row opens the Error Details popup. Every other
        // Changes-view row keeps the pre-existing behavior below.
        if ctx.app.open_error_details_popup() {
            return Ok(());
        }
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

/// Handle input for the Error Details popup.
///
/// The popup owns every key it handles, so scrolling, copying, and closing it
/// cannot move the Changes cursor, the Logs panel, or an interaction modal
/// underneath. It sits below the warning popup (which is dispatched first) and
/// above interaction modals.
///
/// `Ctrl`- and `Alt`-modified keys are deliberately not claimed at all, so
/// `Ctrl+C` keeps its global quit meaning rather than being redefined as the
/// popup copy action. Copy itself is spec'd as *unmodified* `c`, so any other
/// modifier (`Shift`, `Super`, …) is swallowed by the popup without copying.
///
/// Returns true when the key was consumed by the popup.
pub(crate) fn handle_error_details_popup_key(app: &mut AppState, key: KeyEvent) -> bool {
    if app.error_details_popup.is_none() {
        return false;
    }

    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return false;
    }

    match key.code {
        KeyCode::Esc => {
            app.close_error_details_popup();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_error_details_popup(-1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_error_details_popup(1);
        }
        KeyCode::PageUp => {
            app.scroll_error_details_popup(-5);
        }
        KeyCode::PageDown => {
            app.scroll_error_details_popup(5);
        }
        KeyCode::Char('c') if key.modifiers.is_empty() => {
            app.copy_error_details();
        }
        _ => {}
    }

    true
}

/// Handle input for the active typed modal.
///
/// Modal input sits between warning-popup input (highest priority) and ordinary
/// view input. An active modal consumes *every* key: QR closes on any key, and a
/// confirmation acts only on its documented keys while still swallowing the rest,
/// so `x`, navigation, stop, and retry can never leak to the view underneath.
///
/// Returns true when the key was consumed by a modal.
pub(crate) async fn handle_modal_key(key: KeyEvent, ctx: &mut KeyEventContext<'_>) -> bool {
    let Some(modal) = ctx.app.modal.clone() else {
        return false;
    };

    match modal {
        ModalState::QrPopup => {
            ctx.app.hide_qr_popup();
        }
        ModalState::ConfirmForceKill { .. } => match (key.code, key.modifiers) {
            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                if let Some(cmd) = ctx.app.confirm_force_kill() {
                    let _ = ctx.cmd_tx.send(cmd).await;
                }
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                ctx.app.cancel_force_kill();
            }
            _ => {}
        },
        ModalState::ConfirmWorktreeDelete { .. } => match (key.code, key.modifiers) {
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
        },
        // Uppercase `X` alone discards uncommitted work. The keys that opened
        // this confirmation — `Y` and `S` — deliberately do nothing here: an
        // operator repeating the keypress that got them a refusal must not
        // thereby grant the permission the refusal was about. Lowercase `x` is
        // the Changes view's bulk-mark key and is likewise inert, so a mistimed
        // habit cannot destroy work.
        ModalState::ConfirmDirtyDiscard { .. } => match (key.code, key.modifiers) {
            (KeyCode::Char('X'), _) => {
                if let Some(cmd) = ctx.app.confirm_dirty_discard() {
                    let _ = ctx.cmd_tx.send(cmd).await;
                }
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                ctx.app.cancel_worktree_action();
            }
            _ => {}
        },
        // Same fail-safe key set, for the heavier decision: `X` here also
        // deletes an unmerged branch, so `Y`, `S`, and lowercase `x` stay just
        // as inert as they are in the dirty confirmation.
        ModalState::ConfirmAheadDiscard { .. } => match (key.code, key.modifiers) {
            (KeyCode::Char('X'), _) => {
                if let Some(cmd) = ctx.app.confirm_ahead_discard() {
                    let _ = ctx.cmd_tx.send(cmd).await;
                }
            }
            (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) | (KeyCode::Esc, _) => {
                ctx.app.cancel_worktree_action();
            }
            _ => {}
        },
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

    if handle_error_details_popup_key(ctx.app, key) {
        return Ok(None);
    }

    if handle_modal_key(key, ctx).await {
        return Ok(None);
    }

    let mut cmd_to_start: Option<TuiCommand> = None;

    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            request_local_tui_quit(ctx.app, ctx.supervisor);
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
            handle_esc_key(ctx).await;
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
            // Open the force-kill confirmation overlay for active changes in
            // Running mode. Only the modal axis moves; execution stays Running.
            ctx.app.request_force_kill_confirmation();
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
    use std::sync::atomic::Ordering;

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

    /// A supervisor with no launch context behind it.
    ///
    /// Key handling only ever asks the supervisor for the live cancellation
    /// token, so an unstarted supervisor is the whole surface these tests need.
    fn idle_supervisor() -> Arc<crate::tui::run_supervisor::TuiRunSupervisor> {
        let (tx, _rx) = mpsc::channel(1);
        Arc::new(crate::tui::run_supervisor::TuiRunSupervisor::new(
            PathBuf::from("."),
            OrchestratorConfig::default(),
            tx,
            crate::tui::queue::DynamicQueue::new(),
            Arc::new(tokio::sync::RwLock::new(
                crate::orchestration::state::OrchestratorState::new(Vec::new(), 1),
            )),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            crate::parallel::PostArchiveAction::MergeToBase,
            None,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(feature = "web-monitoring")]
            None,
        ))
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
        let custom = TuiConfig::parse_jsonc(
            r#"{"keybindings":{"start":["F5","!"]}}"#,
            std::path::Path::new("/tmp/tui.jsonc"),
        )
        .unwrap();

        let mut f5_app = AppState::new(vec![create_test_change("run-me")]);
        f5_app.set_tui_config(custom.clone());
        f5_app.changes[0].selected = true;
        let f5_command = if f5_app.tui_config.matches_start_key(&key(KeyCode::F(5))) {
            handle_start_key_inner(&mut f5_app)
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
            handle_start_key_inner(&mut bang_app)
        } else {
            None
        };

        assert_eq!(format!("{:?}", f5_command), format!("{:?}", bang_command));
        assert!(matches!(
            bang_command,
            Some(TuiCommand::StartProcessing(ids)) if ids.is_empty()
        ));
    }

    /// The start key is cursor-independent: it never inspects the row under the
    /// cursor, so a merge-wait row can never turn it into a resolve.
    #[test]
    fn start_key_never_emits_resolve_merge_for_the_row_under_the_cursor() {
        let mut app = AppState::new(vec![
            create_test_change("merge-wait"),
            create_test_change("run-me"),
        ]);
        app.execution_mode = AppExecutionMode::Select;
        app.cursor_index = 0;
        app.changes[0].display_status_cache = "merge wait".to_string();
        app.changes[1].selected = true;

        let command = handle_start_key_inner(&mut app);

        assert!(matches!(
            command,
            Some(TuiCommand::StartProcessing(ids)) if ids.is_empty()
        ));
        assert_eq!(app.changes[0].display_status_cache, "merge wait");
    }

    /// Target selection belongs to the shared service, so the key path emits the
    /// same command in Select, Stopped, and Error mode; only Stopping differs,
    /// where the start key withdraws a pending graceful stop.
    #[test]
    fn start_key_emits_one_command_per_mode_class() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = AppState::new(vec![create_test_change("a")]);
            app.execution_mode = mode;
            app.set_resolving("__active__");
            assert!(
                matches!(
                    handle_start_key_inner(&mut app),
                    Some(TuiCommand::StartProcessing(ref ids)) if ids.is_empty()
                ),
                "{mode:?} must delegate start to the shared service"
            );
            assert!(app.warning_message.is_none());
        }

        let mut stopping = AppState::new(vec![create_test_change("a")]);
        stopping.execution_mode = AppExecutionMode::Stopping;
        assert!(matches!(
            handle_start_key_inner(&mut stopping),
            Some(TuiCommand::CancelStop)
        ));
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
        let runtime = StubPlusRuntime::new();
        let worktree_base_dir = temp_dir.path().join("worktrees");
        let supervisor = idle_supervisor();
        let mut ctx = KeyEventContext {
            app: &mut app,
            terminal: &mut terminal,
            repo_root: temp_dir.path(),
            config: &config,
            worktree_base_dir: &worktree_base_dir,
            tx: &tx,
            cmd_tx: &cmd_tx,
            ai_runner: &ai_runner,
            supervisor: &supervisor,
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
        app.execution_mode = AppExecutionMode::Running;
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

    #[tokio::test]
    async fn ctrl_c_quit_cancels_local_orchestrator_token() {
        use crate::orchestration::run_control::RunSchedulerPort;

        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let supervisor = idle_supervisor();
        supervisor
            .start_run(Vec::new(), false)
            .await
            .expect("an idle supervisor accepts a launch");
        let token = supervisor.cancel_token().expect("a live run owns a token");

        request_local_tui_quit(&mut app, &supervisor);

        assert!(app.should_quit);
        assert!(token.is_cancelled());
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("cancelling local orchestration")));
        let (handle, _) = supervisor.take_run();
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    #[test]
    fn ctrl_c_quit_without_local_orchestrator_only_sets_quit() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        request_local_tui_quit(&mut app, &idle_supervisor());

        assert!(app.should_quit);
    }

    #[test]
    fn idle_parallel_stop_second_esc_requests_the_shared_immediate_stop_command() {
        assert_eq!(
            esc_stop_action(&AppExecutionMode::Running, &StopMode::None),
            EscStopAction::RequestGracefulStop
        );
        assert_eq!(
            esc_stop_action(&AppExecutionMode::Stopping, &StopMode::GracefulPending),
            EscStopAction::RequestImmediateStop
        );
    }

    #[test]
    fn idle_parallel_stop_repeated_esc_does_not_duplicate_the_stop_request() {
        assert_eq!(
            esc_stop_action(&AppExecutionMode::Stopping, &StopMode::ImmediatePending),
            EscStopAction::None
        );
        assert_eq!(
            esc_stop_action(&AppExecutionMode::Stopped, &StopMode::None),
            EscStopAction::None
        );
    }

    #[tokio::test]
    async fn idle_parallel_stop_second_esc_routes_through_force_stop_command() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Stopping;
        app.stop_mode = StopMode::GracefulPending;
        let (cmd_tx, mut cmd_rx) = mpsc::channel(4);

        handle_esc_key_inner(&mut app, &cmd_tx).await;

        assert!(
            matches!(cmd_rx.try_recv(), Ok(TuiCommand::ForceStop)),
            "the second Esc must route through the shared stop command"
        );
        assert_eq!(app.stop_mode, StopMode::ImmediatePending);
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopping,
            "the key path must not apply terminal stop effects itself"
        );
        assert!(
            !app.logs
                .iter()
                .any(|entry| entry.message.contains("Force stopped")),
            "the key path must not claim a force stop from AppExecutionMode::Stopping alone"
        );
        assert!(
            !app.logs
                .iter()
                .any(|entry| entry.message.contains("Processing stopped")),
            "terminal stop reporting is owned by the Stopped transition"
        );
    }

    #[tokio::test]
    async fn idle_parallel_stop_repeated_esc_sends_one_stop_command() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Stopping;
        app.stop_mode = StopMode::GracefulPending;
        let (cmd_tx, mut cmd_rx) = mpsc::channel(4);

        handle_esc_key_inner(&mut app, &cmd_tx).await;
        handle_esc_key_inner(&mut app, &cmd_tx).await;

        assert!(matches!(cmd_rx.try_recv(), Ok(TuiCommand::ForceStop)));
        assert!(
            cmd_rx.try_recv().is_err(),
            "a repeated Esc must not enqueue a second cancellation request"
        );
    }

    #[tokio::test]
    async fn idle_parallel_stop_first_esc_keeps_graceful_stop_contract() {
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        app.execution_mode = AppExecutionMode::Running;
        let (cmd_tx, mut cmd_rx) = mpsc::channel(4);

        handle_esc_key_inner(&mut app, &cmd_tx).await;

        // The key path records the pending mode so a second Esc is an immediate
        // stop, but the graceful-stop effect itself is the shared service's.
        assert_eq!(app.stop_mode, StopMode::GracefulPending);
        assert!(matches!(cmd_rx.try_recv(), Ok(TuiCommand::Stop)));
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "the key path must not apply the stop transition itself"
        );
    }

    // ========================================================================
    // Input routing priority: warning popup → typed modal → view/execution keys
    // ========================================================================

    /// Keys that would visibly mutate underlying state if an overlay leaked them.
    const HIGH_IMPACT_KEYS: [KeyCode; 10] = [
        KeyCode::Char('x'),
        KeyCode::Char(' '),
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Char('j'),
        KeyCode::Char('K'),
        KeyCode::Char('M'),
        KeyCode::Esc,
        KeyCode::F(5),
        KeyCode::Tab,
    ];

    /// Underlying state a leaked key would be visible in.
    #[derive(Debug, PartialEq)]
    struct UnderlyingState {
        execution_mode: AppExecutionMode,
        stop_mode: StopMode,
        cursor_index: usize,
        view_mode: ViewMode,
        marks: Vec<bool>,
        statuses: Vec<String>,
    }

    impl UnderlyingState {
        fn capture(app: &AppState) -> Self {
            Self {
                execution_mode: app.execution_mode,
                stop_mode: app.stop_mode.clone(),
                cursor_index: app.cursor_index,
                view_mode: app.view_mode,
                marks: app.changes.iter().map(|c| c.selected).collect(),
                statuses: app
                    .changes
                    .iter()
                    .map(|c| c.display_status_cache.clone())
                    .collect(),
            }
        }
    }

    fn routing_app() -> AppState {
        let mut app = AppState::new(vec![
            create_test_change("change-a"),
            create_test_change("change-b"),
        ]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("applying");
        app.changes[1].set_display_status_cache("not queued");
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app
    }

    fn test_terminal() -> DefaultTerminal {
        ratatui::Terminal::with_options(
            ratatui::backend::CrosstermBackend::new(std::io::stdout()),
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 80, 24)),
            },
        )
        .unwrap()
    }

    /// Feed one key through the full routing entry point.
    async fn route_key(app: &mut AppState, code: KeyCode) -> Vec<TuiCommand> {
        route_key_event(app, key(code)).await
    }

    /// Feed one fully specified key event through the full routing entry point.
    async fn route_key_event(app: &mut AppState, event: KeyEvent) -> Vec<TuiCommand> {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut terminal = test_terminal();
        let config = OrchestratorConfig::default();
        let (tx, _rx) = mpsc::channel(8);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(8);
        let ai_runner = test_ai_runner();
        let supervisor = idle_supervisor();
        let worktree_base_dir = temp_dir.path().join("worktrees");
        let mut ctx = KeyEventContext {
            app,
            terminal: &mut terminal,
            repo_root: temp_dir.path(),
            config: &config,
            worktree_base_dir: &worktree_base_dir,
            tx: &tx,
            cmd_tx: &cmd_tx,
            ai_runner: &ai_runner,
            supervisor: &supervisor,
        };

        handle_key_event(event, &mut ctx).await.unwrap();

        let mut commands = Vec::new();
        while let Ok(command) = cmd_rx.try_recv() {
            commands.push(command);
        }
        commands
    }

    #[tokio::test]
    async fn warning_popup_owns_input_before_the_typed_modal() {
        let mut app = routing_app();
        app.show_qr_popup();
        app.show_warning_popup("warning", "line 1\nline 2\nline 3");

        // A popup scroll key must reach the popup, not the QR overlay under it.
        let commands = route_key(&mut app, KeyCode::Down).await;

        assert!(commands.is_empty());
        assert!(app.warning_popup.is_some());
        assert_eq!(app.warning_popup_scroll, 1);
        assert_eq!(
            app.modal,
            Some(ModalState::QrPopup),
            "the interaction modal must not process a warning-popup key"
        );
        assert_eq!(app.execution_mode, AppExecutionMode::Running);
    }

    #[tokio::test]
    async fn every_overlay_consumes_high_impact_keys_without_touching_the_view() {
        type OpenOverlay = fn(&mut AppState);
        let overlays: Vec<(&str, OpenOverlay)> = vec![
            ("qr", |app: &mut AppState| app.show_qr_popup()),
            ("worktree-delete", |app: &mut AppState| {
                app.modal = Some(ModalState::ConfirmWorktreeDelete {
                    path: PathBuf::from("/tmp/wt-a"),
                    branch: "change-b".to_string(),
                });
            }),
            ("force-kill", |app: &mut AppState| {
                app.modal = Some(ModalState::ConfirmForceKill {
                    change_id: "change-a".to_string(),
                });
            }),
            ("warning-popup", |app: &mut AppState| {
                app.show_warning_popup("warning", "diagnostic")
            }),
            ("error-details", |app: &mut AppState| {
                app.changes[1].set_error_message_cache("boom".to_string());
                app.cursor_index = 1;
                assert!(app.open_error_details_popup());
            }),
        ];

        for (name, open) in overlays {
            for code in HIGH_IMPACT_KEYS {
                let mut app = routing_app();
                open(&mut app);
                let before = UnderlyingState::capture(&app);

                let commands = route_key(&mut app, code).await;

                assert!(
                    commands.is_empty(),
                    "{name} overlay leaked {code:?} as a command"
                );
                assert_eq!(
                    UnderlyingState::capture(&app),
                    before,
                    "{name} overlay leaked {code:?} into the underlying view"
                );
            }
        }
    }

    #[tokio::test]
    async fn an_overlay_consumes_the_resolve_key_without_emitting_resolve_merge() {
        let overlays = [
            ModalState::QrPopup,
            ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/wt-a"),
                branch: "change-b".to_string(),
            },
            ModalState::ConfirmForceKill {
                change_id: "change-a".to_string(),
            },
        ];

        for modal in overlays {
            let mut app = routing_app();
            app.execution_mode = AppExecutionMode::Select;
            // A row the resolve key would otherwise act on.
            app.changes[1].set_display_status_cache("merge wait");
            app.cursor_index = 1;
            app.modal = Some(modal.clone());

            let commands = route_key(&mut app, KeyCode::Char('M')).await;

            assert!(
                commands.is_empty(),
                "{modal:?} must not let the resolve key through"
            );
            assert_eq!(
                app.execution_mode,
                AppExecutionMode::Select,
                "{modal:?} must not let the resolve key move the execution axis"
            );
        }
    }

    #[tokio::test]
    async fn qr_closes_on_any_key_and_exposes_the_latest_execution_mode() {
        for code in [KeyCode::Esc, KeyCode::Char('x'), KeyCode::Enter] {
            let mut app = routing_app();
            app.show_qr_popup();

            // A background transition lands while the popup is open.
            app.execution_mode = AppExecutionMode::Stopping;

            let commands = route_key(&mut app, code).await;

            assert!(commands.is_empty());
            assert!(app.modal.is_none(), "{code:?} must close the QR popup");
            assert_eq!(
                app.execution_mode,
                AppExecutionMode::Stopping,
                "closing QR must expose the latest execution mode, not a captured one"
            );
        }
    }

    #[tokio::test]
    async fn force_kill_confirm_dispatches_stop_and_dequeue_without_rewriting_execution() {
        let mut app = routing_app();
        app.execution_mode = AppExecutionMode::Stopping;
        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        });

        let commands = route_key(&mut app, KeyCode::Char('y')).await;

        assert!(
            matches!(commands.as_slice(), [TuiCommand::DequeueChange(id)] if id == "change-a"),
            "expected a single stop-and-dequeue command, got {commands:?}"
        );
        assert!(app.modal.is_none());
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopping,
            "confirming must not rewrite execution back to Running"
        );
    }

    #[tokio::test]
    async fn force_kill_cancel_preserves_the_current_execution_mode() {
        let mut app = routing_app();
        app.execution_mode = AppExecutionMode::Stopping;
        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        });

        let commands = route_key(&mut app, KeyCode::Esc).await;

        assert!(commands.is_empty());
        assert!(app.modal.is_none());
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopping,
            "cancel must not restore Running unconditionally"
        );
        assert_eq!(app.stop_mode, StopMode::None);
    }

    #[tokio::test]
    async fn force_kill_confirm_refuses_a_stale_target_without_dispatching() {
        let mut app = routing_app();
        app.modal = Some(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        });
        // The target settled between display and confirmation input.
        app.changes[0].set_display_status_cache("archived");

        let commands = route_key(&mut app, KeyCode::Char('y')).await;

        assert!(
            commands.is_empty(),
            "a stale force-kill must not reach the shared service"
        );
        assert!(app.modal.is_none());
        assert_eq!(app.changes[0].display_status_cache, "archived");
        assert!(app
            .warning_message
            .as_ref()
            .is_some_and(|msg| msg.contains("Force-kill canceled")));
    }

    #[tokio::test]
    async fn worktree_delete_confirm_dispatches_when_identity_still_matches() {
        let mut app = routing_app();
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/tmp/wt-a"),
            head: "abc1234".to_string(),
            branch: "change-b".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];
        app.modal = Some(ModalState::ConfirmWorktreeDelete {
            path: PathBuf::from("/tmp/wt-a"),
            branch: "change-b".to_string(),
        });

        let commands = route_key(&mut app, KeyCode::Char('y')).await;

        assert!(
            matches!(
                commands.as_slice(),
                [TuiCommand::DeleteWorktree(intent)]
                    if intent.path == *"/tmp/wt-a"
                        && intent.branch == "change-b"
                        && !intent.skip_teardown
                        && !intent.allow_known_dirty
            ),
            "expected a single worktree delete command, got {commands:?}"
        );
        assert!(app.modal.is_none());
        assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")));
    }

    #[tokio::test]
    async fn worktree_delete_confirm_refuses_stale_identity_without_mutating_state() {
        for (name, mutate) in [
            (
                "absent",
                (|app: &mut AppState| app.worktrees.clear()) as fn(&mut AppState),
            ),
            ("rebranded", |app: &mut AppState| {
                app.worktrees[0].branch = "change-z".to_string()
            }),
            ("main", |app: &mut AppState| app.worktrees[0].is_main = true),
        ] {
            for code in [KeyCode::Char('y'), KeyCode::Char('s')] {
                let mut app = routing_app();
                app.view_mode = ViewMode::Worktrees;
                app.worktrees = vec![crate::tui::types::WorktreeInfo {
                    path: PathBuf::from("/tmp/wt-a"),
                    head: "abc1234".to_string(),
                    branch: "change-b".to_string(),
                    is_detached: false,
                    is_main: false,
                    merge_conflict: None,
                    has_commits_ahead: false,
                    is_merging: false,
                }];
                app.modal = Some(ModalState::ConfirmWorktreeDelete {
                    path: PathBuf::from("/tmp/wt-a"),
                    branch: "change-b".to_string(),
                });
                mutate(&mut app);

                let commands = route_key(&mut app, code).await;

                assert!(
                    commands.is_empty(),
                    "{name}: a stale worktree delete must not be dispatched for {code:?}"
                );
                assert!(
                    app.modal.is_none(),
                    "{name}: the stale modal must be cleared"
                );
                assert!(
                    !app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")),
                    "{name}: refusing must not mark the worktree as deleting"
                );
                assert!(app
                    .warning_message
                    .as_ref()
                    .is_some_and(|msg| msg.contains("Worktree delete canceled")));
            }
        }
    }

    /// A named way the confirmation's target can drift before the keypress.
    type DriftCase = (&'static str, fn(&mut AppState));

    /// An app in the destructive dirty-discard confirmation over `/tmp/wt-a`.
    fn tui_dirty_worktree_delete_app(skip_teardown: bool) -> AppState {
        let mut app = routing_app();
        app.view_mode = ViewMode::Worktrees;
        app.worktrees = vec![crate::tui::types::WorktreeInfo {
            path: PathBuf::from("/tmp/wt-a"),
            head: "abc1234".to_string(),
            branch: "change-b".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: false,
            is_merging: false,
        }];
        app.modal = Some(ModalState::ConfirmDirtyDiscard {
            path: PathBuf::from("/tmp/wt-a"),
            identity: "gitdir: /tmp/wt-a/.git".to_string(),
            branch: "change-b".to_string(),
            head: "abc1234".to_string(),
            skip_teardown,
        });
        app
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_uppercase_x_is_the_only_key_that_discards() {
        for skip_teardown in [false, true] {
            let mut app = tui_dirty_worktree_delete_app(skip_teardown);

            let commands = route_key(&mut app, KeyCode::Char('X')).await;

            let [TuiCommand::DeleteWorktree(intent)] = commands.as_slice() else {
                panic!("expected a single delete command, got {commands:?}");
            };
            assert!(intent.allow_known_dirty);
            assert_eq!(intent.skip_teardown, skip_teardown);
            assert_eq!(intent.path, PathBuf::from("/tmp/wt-a"));
            assert_eq!(intent.branch, "change-b");
            assert_eq!(intent.identity.as_deref(), Some("gitdir: /tmp/wt-a/.git"));
            assert_eq!(intent.head.as_deref(), Some("abc1234"));
            assert!(app.modal.is_none());
            assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")));
        }
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_confirmation_ignores_every_other_key() {
        // `y`/`Y`/`s`/`S` are the keys that got the operator here, and lowercase
        // `x` is the Changes view's bulk-mark key. None of them may become the
        // destructive decision by habit or by shift key slipping.
        for code in [
            KeyCode::Char('y'),
            KeyCode::Char('Y'),
            KeyCode::Char('s'),
            KeyCode::Char('S'),
            KeyCode::Char('x'),
            KeyCode::Char('d'),
            KeyCode::Char('D'),
            KeyCode::Enter,
            KeyCode::Char(' '),
            KeyCode::Down,
            KeyCode::Tab,
            KeyCode::F(5),
        ] {
            let mut app = tui_dirty_worktree_delete_app(false);
            let before = app.modal.clone();

            let commands = route_key(&mut app, code).await;

            assert!(
                commands.is_empty(),
                "{code:?} must not dispatch anything from the destructive confirmation"
            );
            assert_eq!(
                app.modal, before,
                "{code:?} must leave the destructive confirmation exactly as it was"
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")),
                "{code:?} must not start a deletion"
            );
        }
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_n_and_esc_cancel_and_retain_the_content() {
        for code in [KeyCode::Char('n'), KeyCode::Char('N'), KeyCode::Esc] {
            let mut app = tui_dirty_worktree_delete_app(false);

            let commands = route_key(&mut app, code).await;

            assert!(commands.is_empty(), "{code:?} must dispatch nothing");
            assert!(app.modal.is_none(), "{code:?} must close the confirmation");
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")),
                "{code:?} must retain the worktree and its uncommitted work"
            );
        }
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_refuses_a_target_that_became_active_before_dispatch() {
        // The confirmation is advisory presentation state; the target can move
        // underneath it. Re-checking at input time is what stops an activation
        // that lands between the escalation and the keypress.
        let cases: [DriftCase; 6] = [
            ("absent", |app: &mut AppState| app.worktrees.clear()),
            ("main", |app: &mut AppState| app.worktrees[0].is_main = true),
            ("rebranded", |app: &mut AppState| {
                app.worktrees[0].branch = "change-z".to_string()
            }),
            ("head-moved", |app: &mut AppState| {
                app.worktrees[0].head = "def5678".to_string()
            }),
            ("active", |app: &mut AppState| {
                app.changes[1].set_display_status_cache("applying")
            }),
            ("deleting", |app: &mut AppState| {
                app.mark_worktree_deleting(PathBuf::from("/tmp/wt-a"))
            }),
        ];

        for (name, mutate) in cases {
            let mut app = tui_dirty_worktree_delete_app(false);
            mutate(&mut app);

            let commands = route_key(&mut app, KeyCode::Char('X')).await;

            assert!(
                commands.is_empty(),
                "{name}: a drifted target must not be discarded"
            );
            assert!(
                app.modal.is_none(),
                "{name}: the stale confirmation must be cleared"
            );
            assert!(app
                .warning_message
                .as_ref()
                .is_some_and(|msg| msg.contains("Dirty worktree discard canceled")));
        }
    }

    /// An app in the destructive ahead-discard confirmation over `/tmp/wt-a`.
    fn tui_ahead_worktree_delete_app(skip_teardown: bool, dirty: bool) -> AppState {
        let mut app = tui_dirty_worktree_delete_app(skip_teardown);
        app.modal = Some(ModalState::ConfirmAheadDiscard {
            path: PathBuf::from("/tmp/wt-a"),
            identity: "gitdir: /tmp/wt-a/.git".to_string(),
            branch: "change-b".to_string(),
            head: "abc1234".to_string(),
            dirty,
            skip_teardown,
        });
        app
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_uppercase_x_is_the_only_key_that_discards() {
        for dirty in [false, true] {
            for skip_teardown in [false, true] {
                let mut app = tui_ahead_worktree_delete_app(skip_teardown, dirty);

                let commands = route_key(&mut app, KeyCode::Char('X')).await;

                let [TuiCommand::DeleteWorktree(intent)] = commands.as_slice() else {
                    panic!("expected a single delete command, got {commands:?}");
                };
                assert!(intent.allow_commits_ahead);
                assert_eq!(
                    intent.allow_known_dirty, dirty,
                    "the one keypress grants dirty discard only where the modal disclosed it"
                );
                assert_eq!(intent.skip_teardown, skip_teardown);
                assert_eq!(intent.path, PathBuf::from("/tmp/wt-a"));
                assert_eq!(intent.branch, "change-b");
                assert_eq!(intent.identity.as_deref(), Some("gitdir: /tmp/wt-a/.git"));
                assert_eq!(intent.head.as_deref(), Some("abc1234"));
                assert!(app.modal.is_none());
                assert!(app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")));
            }
        }
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_confirmation_ignores_every_other_key() {
        // The keys that got the operator here, the bulk-mark key, and the
        // lowercase twin of the one destructive key: all inert. This modal
        // authorizes deleting commits, so a slipped shift may not decide it.
        for code in [
            KeyCode::Char('y'),
            KeyCode::Char('Y'),
            KeyCode::Char('s'),
            KeyCode::Char('S'),
            KeyCode::Char('x'),
            KeyCode::Char('d'),
            KeyCode::Char('D'),
            KeyCode::Enter,
            KeyCode::Char(' '),
            KeyCode::Down,
            KeyCode::Tab,
            KeyCode::F(5),
        ] {
            let mut app = tui_ahead_worktree_delete_app(false, true);
            let before = app.modal.clone();

            let commands = route_key(&mut app, code).await;

            assert!(
                commands.is_empty(),
                "{code:?} must not dispatch anything from the ahead confirmation"
            );
            assert_eq!(
                app.modal, before,
                "{code:?} must leave the ahead confirmation exactly as it was"
            );
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")),
                "{code:?} must not start a deletion"
            );
        }
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_n_and_esc_cancel_and_retain_both_resources() {
        for code in [KeyCode::Char('n'), KeyCode::Char('N'), KeyCode::Esc] {
            let mut app = tui_ahead_worktree_delete_app(false, false);

            let commands = route_key(&mut app, code).await;

            assert!(commands.is_empty(), "{code:?} must dispatch nothing");
            assert!(app.modal.is_none(), "{code:?} must close the confirmation");
            assert!(
                !app.is_worktree_deleting(&PathBuf::from("/tmp/wt-a")),
                "{code:?} must retain the worktree, its content, and its branch"
            );
        }
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_refuses_a_target_that_drifted_before_dispatch() {
        let cases: [DriftCase; 6] = [
            ("absent", |app: &mut AppState| app.worktrees.clear()),
            ("main", |app: &mut AppState| app.worktrees[0].is_main = true),
            ("rebranded", |app: &mut AppState| {
                app.worktrees[0].branch = "change-z".to_string()
            }),
            ("head-moved", |app: &mut AppState| {
                app.worktrees[0].head = "def5678".to_string()
            }),
            ("active", |app: &mut AppState| {
                app.changes[1].set_display_status_cache("applying")
            }),
            ("deleting", |app: &mut AppState| {
                app.mark_worktree_deleting(PathBuf::from("/tmp/wt-a"))
            }),
        ];

        for (name, mutate) in cases {
            let mut app = tui_ahead_worktree_delete_app(false, false);
            mutate(&mut app);

            let commands = route_key(&mut app, KeyCode::Char('X')).await;

            assert!(
                commands.is_empty(),
                "{name}: a drifted target must not be discarded"
            );
            assert!(
                app.modal.is_none(),
                "{name}: the stale confirmation must be cleared"
            );
            assert!(app
                .warning_message
                .as_ref()
                .is_some_and(|msg| msg.contains("Ahead worktree discard canceled")));
        }
    }

    #[tokio::test]
    async fn k_key_opens_force_kill_without_leaving_running_execution() {
        let mut app = routing_app();

        let commands = route_key(&mut app, KeyCode::Char('K')).await;

        assert!(commands.is_empty());
        assert_eq!(
            app.modal,
            Some(ModalState::ConfirmForceKill {
                change_id: "change-a".to_string()
            })
        );
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "opening a confirmation must not move the execution axis"
        );
    }

    #[tokio::test]
    async fn w_key_opens_qr_over_the_current_execution_mode() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            let mut app = routing_app();
            app.execution_mode = mode;

            route_key(&mut app, KeyCode::Char('w')).await;

            assert_eq!(app.modal, Some(ModalState::QrPopup));
            assert_eq!(app.execution_mode, mode);
        }
    }

    #[tokio::test]
    async fn w_key_is_ignored_when_web_monitoring_is_disabled() {
        let mut app = routing_app();
        app.web_url = None;
        let before = UnderlyingState::capture(&app);

        route_key(&mut app, KeyCode::Char('w')).await;

        assert!(app.modal.is_none());
        assert_eq!(UnderlyingState::capture(&app), before);
    }

    #[tokio::test]
    async fn bulk_mark_key_applies_across_the_admitted_execution_modes() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopped,
        ] {
            let mut app = routing_app();
            app.execution_mode = mode;

            route_key(&mut app, KeyCode::Char('x')).await;

            assert!(
                app.changes[1].selected,
                "{mode:?} must admit bulk mark for eligible rows"
            );
        }

        for mode in [AppExecutionMode::Stopping, AppExecutionMode::Error] {
            let mut app = routing_app();
            app.execution_mode = mode;

            route_key(&mut app, KeyCode::Char('x')).await;

            assert!(
                !app.changes[1].selected,
                "{mode:?} must refuse bulk mark through the shared matrix"
            );
        }
    }

    // ========================================================================
    // Error Details popup input ownership
    // ========================================================================

    /// A Changes-view app whose cursor sits on an `error` row.
    fn error_details_app() -> AppState {
        let mut app = routing_app();
        app.changes[1].set_error_message_cache("Apply failed: stalled".to_string());
        app.cursor_index = 1;
        app
    }

    #[tokio::test]
    async fn enter_opens_the_error_details_popup_on_an_error_row() {
        let mut app = error_details_app();

        let commands = route_key(&mut app, KeyCode::Enter).await;

        assert!(commands.is_empty(), "opening a popup emits no command");
        let popup = app
            .error_details_popup
            .as_ref()
            .expect("Enter opens the Error Details popup");
        assert_eq!(popup.change_id, "change-b");
        assert_eq!(popup.error, "Apply failed: stalled");
        assert_eq!(popup.scroll, 0);
        assert!(popup.copy_feedback.is_none());
    }

    #[tokio::test]
    async fn enter_on_a_non_error_row_keeps_its_existing_behavior() {
        let mut app = error_details_app();
        app.cursor_index = 0; // `applying`
        let before = UnderlyingState::capture(&app);

        let commands = route_key(&mut app, KeyCode::Enter).await;

        assert!(commands.is_empty());
        assert!(
            app.error_details_popup.is_none(),
            "only an error row opens the popup"
        );
        assert_eq!(UnderlyingState::capture(&app), before);
        assert!(
            app.logs
                .iter()
                .any(|log| log.message.contains("Enter ignored: not in Worktrees view")),
            "the pre-existing Enter behavior is unchanged"
        );
    }

    #[tokio::test]
    async fn popup_scroll_keys_do_not_move_the_underlying_views() {
        for code in [KeyCode::Down, KeyCode::Char('j'), KeyCode::PageDown] {
            let mut app = error_details_app();
            for index in 0..12 {
                app.add_log(LogEntry::info(format!("log {index}")));
            }
            app.scroll_logs_up(3);
            assert!(app.open_error_details_popup());
            let before = UnderlyingState::capture(&app);
            let log_scroll_before = app.log_scroll_offset;

            let commands = route_key_event(&mut app, key(code)).await;

            assert!(commands.is_empty(), "{code:?} leaked a command");
            assert!(
                app.error_details_popup
                    .as_ref()
                    .is_some_and(|popup| popup.scroll > 0),
                "{code:?} must scroll the popup"
            );
            assert_eq!(
                UnderlyingState::capture(&app),
                before,
                "{code:?} moved the Changes list underneath the popup"
            );
            assert_eq!(
                app.log_scroll_offset, log_scroll_before,
                "{code:?} moved the Logs panel underneath the popup"
            );
        }
    }

    #[tokio::test]
    async fn escape_closes_the_popup_without_a_workflow_transition() {
        let mut app = error_details_app();
        assert!(app.open_error_details_popup());
        let before = UnderlyingState::capture(&app);

        let commands = route_key(&mut app, KeyCode::Esc).await;

        assert!(commands.is_empty(), "closing a popup emits no command");
        assert!(app.error_details_popup.is_none());
        assert_eq!(UnderlyingState::capture(&app), before);
        assert_eq!(app.stop_mode, StopMode::None, "Esc must not reach the view");
    }

    #[tokio::test]
    async fn warning_popup_retains_first_claim_over_the_error_details_popup() {
        let mut app = error_details_app();
        assert!(app.open_error_details_popup());
        app.show_warning_popup("warning", "line 1\nline 2\nline 3");
        let before = UnderlyingState::capture(&app);

        route_key(&mut app, KeyCode::Down).await;

        assert_eq!(app.warning_popup_scroll, 1, "the warning popup scrolled");
        assert_eq!(
            app.error_details_popup.as_ref().map(|popup| popup.scroll),
            Some(0),
            "the Error Details popup must not process the same key"
        );
        assert_eq!(UnderlyingState::capture(&app), before);
    }

    #[tokio::test]
    async fn ctrl_c_keeps_its_global_quit_meaning_while_the_popup_is_open() {
        let mut app = error_details_app();
        app.set_clipboard(Arc::new(
            crate::tui::clipboard::test_doubles::RecordingClipboard::default(),
        ));
        assert!(app.open_error_details_popup());

        route_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        )
        .await;

        assert!(app.should_quit, "Ctrl+C must still quit");
        assert!(
            app.error_details_popup
                .as_ref()
                .is_some_and(|popup| popup.copy_feedback.is_none()),
            "a modified key must not be treated as the popup copy action"
        );
    }

    /// Copying is unit-scoped: the clipboard boundary is injected, so the
    /// developer's real clipboard is never touched.
    #[test]
    fn unmodified_c_copies_stable_plain_text_and_keeps_the_popup_open() {
        let mut app = error_details_app();
        let clipboard =
            Arc::new(crate::tui::clipboard::test_doubles::RecordingClipboard::default());
        app.set_clipboard(clipboard.clone());
        assert!(app.open_error_details_popup());

        assert!(handle_error_details_popup_key(
            &mut app,
            key(KeyCode::Char('c'))
        ));

        assert_eq!(
            clipboard.copies(),
            vec!["Change: change-b\nError: Apply failed: stalled".to_string()]
        );
        let popup = app.error_details_popup.as_ref().expect("popup stays open");
        assert_eq!(popup.error, "Apply failed: stalled");
        assert_eq!(
            popup.copy_feedback,
            Some(crate::tui::state::CopyFeedback::Copied)
        );
    }

    /// Copy is spec'd as *unmodified* `c`. Any other modifier combination stays
    /// owned by the popup but must not reach the clipboard, and `Ctrl+C` keeps
    /// falling through to the global quit binding.
    #[tokio::test]
    async fn only_unmodified_c_copies_and_ctrl_c_still_quits() {
        for (code, modifiers) in [
            (KeyCode::Char('c'), KeyModifiers::SHIFT),
            (KeyCode::Char('C'), KeyModifiers::SHIFT),
            (KeyCode::Char('c'), KeyModifiers::SUPER),
            (
                KeyCode::Char('c'),
                KeyModifiers::SHIFT | KeyModifiers::SUPER,
            ),
        ] {
            let mut app = error_details_app();
            let clipboard =
                Arc::new(crate::tui::clipboard::test_doubles::RecordingClipboard::default());
            app.set_clipboard(clipboard.clone());
            assert!(app.open_error_details_popup());

            assert!(
                handle_error_details_popup_key(&mut app, KeyEvent::new(code, modifiers)),
                "{code:?}+{modifiers:?} stays owned by the popup"
            );

            assert!(
                clipboard.copies().is_empty(),
                "{code:?}+{modifiers:?} must not copy"
            );
            let popup = app.error_details_popup.as_ref().expect("popup stays open");
            assert!(
                popup.copy_feedback.is_none(),
                "{code:?}+{modifiers:?} reports no copy feedback"
            );
        }

        let mut app = error_details_app();
        let clipboard =
            Arc::new(crate::tui::clipboard::test_doubles::RecordingClipboard::default());
        app.set_clipboard(clipboard.clone());
        assert!(app.open_error_details_popup());

        assert!(handle_error_details_popup_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE)
        ));

        assert_eq!(
            clipboard.copies(),
            vec!["Change: change-b\nError: Apply failed: stalled".to_string()],
            "unmodified c is the one binding that copies"
        );
        assert!(!app.should_quit, "unmodified c never quits");

        let mut app = error_details_app();
        let clipboard =
            Arc::new(crate::tui::clipboard::test_doubles::RecordingClipboard::default());
        app.set_clipboard(clipboard.clone());
        assert!(app.open_error_details_popup());

        route_key_event(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        )
        .await;

        assert!(app.should_quit, "Ctrl+C keeps its global quit meaning");
        assert!(clipboard.copies().is_empty(), "Ctrl+C must not copy");
    }

    #[test]
    fn a_refused_copy_keeps_the_popup_open_with_actionable_feedback() {
        let mut app = error_details_app();
        app.set_clipboard(Arc::new(
            crate::tui::clipboard::test_doubles::FailingClipboard::new("no clipboard provider"),
        ));
        assert!(app.open_error_details_popup());

        assert!(handle_error_details_popup_key(
            &mut app,
            key(KeyCode::Char('c'))
        ));

        let popup = app.error_details_popup.as_ref().expect("popup stays open");
        assert_eq!(popup.error, "Apply failed: stalled");
        let feedback = popup.copy_feedback.clone().expect("failure is reported");
        assert_eq!(
            feedback,
            crate::tui::state::CopyFeedback::Failed("no clipboard provider".to_string())
        );
        let message = feedback.message();
        assert!(message.contains("no clipboard provider"), "{message}");
        assert!(message.contains("manually"), "{message}");
    }

    #[test]
    fn popup_keys_are_ignored_when_no_popup_is_open() {
        let mut app = error_details_app();

        for code in [
            KeyCode::Char('c'),
            KeyCode::Char('j'),
            KeyCode::Esc,
            KeyCode::PageDown,
        ] {
            assert!(
                !handle_error_details_popup_key(&mut app, key(code)),
                "{code:?} must fall through when the popup is closed"
            );
        }
    }
}
