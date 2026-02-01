//! Terminal helper functions for TUI
//!
//! This module contains helper functions for terminal operations, extracted from runner.rs
//! to eliminate circular dependencies.

use crate::error::Result;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::DefaultTerminal;
use std::path::Path;
use tracing::info;

use super::events::LogEntry;
use super::state::AppState;
use super::utils::clear_screen;

/// Restore terminal state (called on panic or normal exit)
pub fn restore_terminal() {
    // Always try to disable mouse capture, even if it wasn't enabled
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    let _ = clear_screen();
    ratatui::restore();
}

/// Execute a worktree command with terminal suspension and result logging
///
/// This helper executes a worktree command using AiCommandRunner, forwards output
/// to stdout/stderr, and logs the result to the app state.
pub async fn execute_worktree_command(
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
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    terminal.clear()?;

    result
}

/// Suspend terminal, execute a synchronous function, then restore terminal
pub fn suspend_terminal_and_execute_sync<F, T>(terminal: &mut DefaultTerminal, f: F) -> Result<T>
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
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    terminal.clear()?;

    result
}
