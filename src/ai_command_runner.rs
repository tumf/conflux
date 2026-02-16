//! Common AI command runner layer for unified stagger state management.
//!
//! This module provides a shared execution layer for all AI-driven commands
//! (apply, archive, resolve, analyze) to ensure consistent stagger delays
//! across parallel and serial execution modes.

use crate::command_queue::{CommandQueue, CommandQueueConfig};
use crate::error::{OrchestratorError, Result};
use crate::process_manager::ManagedChild;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tracing::debug;

/// Shared stagger state type: Arc<Mutex<Option<Instant>>>
/// This type is shared across all AI command executions to coordinate stagger delays
pub type SharedStaggerState = Arc<Mutex<Option<Instant>>>;

/// Output line from a child process
#[derive(Debug, Clone)]
#[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

/// Common AI command runner with shared stagger state.
///
/// This runner wraps CommandQueue and provides streaming execution
/// for AI-driven commands (apply, archive, resolve, analyze).
/// The shared stagger state ensures consistent delays across all
/// parallel workspaces and command types.
#[derive(Clone)]
#[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
pub struct AiCommandRunner {
    command_queue: CommandQueue,
}

impl AiCommandRunner {
    /// Create a new AiCommandRunner with shared stagger state.
    ///
    /// # Arguments
    ///
    /// * `config` - CommandQueue configuration
    /// * `shared_state` - Shared last execution timestamp for stagger coordination
    pub fn new(config: CommandQueueConfig, shared_state: SharedStaggerState) -> Self {
        Self {
            command_queue: CommandQueue::new_with_shared_state(config, shared_state),
        }
    }

    /// Get access to the underlying CommandQueue configuration.
    ///
    /// This is useful for implementing custom retry logic that respects
    /// the configured retry parameters.
    #[allow(dead_code)] // Used by parallel executor for retry logic
    pub fn queue_config(&self) -> &crate::command_queue::CommandQueueConfig {
        self.command_queue.config()
    }

    /// Execute a command with streaming output, stagger delay, and automatic retry.
    ///
    /// This method executes a command through CommandQueue with both stagger and retry.
    /// It automatically retries transient failures based on the CommandQueue configuration.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute (will be run via `sh -c`)
    /// * `cwd` - Optional working directory (for worktree execution)
    ///
    /// # Returns
    ///
    /// A tuple of (ManagedChild, Receiver<OutputLine>) for output streaming.
    /// Note: The child process may have already completed by the time this returns,
    /// as retry logic runs in a background task.
    ///
    /// # Retry Behavior
    ///
    /// Retries are attempted based on CommandQueue configuration:
    /// - Error pattern matching (retry_error_patterns)
    /// - Short execution duration (retry_if_duration_under_secs)
    /// - Non-zero exit code (agent crash)
    ///
    /// Retry notifications are sent through the output channel as stderr lines.
    pub async fn execute_streaming_with_retry(
        &self,
        command: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        use crate::command_queue::StreamingOutputLine;
        use std::process::ExitStatus;

        // Create output channel
        let (tx, rx) = mpsc::channel::<OutputLine>(1024);

        // Clone tx for callback
        let tx_clone = tx.clone();

        // Create callback to forward streaming output
        let output_callback = move |line: StreamingOutputLine| {
            let tx = tx_clone.clone();
            async move {
                let output_line = match line {
                    StreamingOutputLine::Stdout(s) => OutputLine::Stdout(s),
                    StreamingOutputLine::Stderr(s) => OutputLine::Stderr(s),
                };
                let _ = tx.send(output_line).await;
            }
        };

        // Clone command queue, command string, and cwd for background task
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();
        let cwd_owned = cwd.map(|p| p.to_path_buf());

        // Create oneshot channel to communicate final status
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<ExitStatus>();

        // Spawn background task to run retry logic
        tokio::spawn(async move {
            let result = command_queue
                .execute_with_retry_streaming(
                    || {
                        // Build command for each retry attempt
                        let mut cmd = Command::new("sh");
                        cmd.arg("-c")
                            .arg(&command_str)
                            .stdin(Stdio::null())
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped());

                        if let Some(ref dir) = cwd_owned {
                            cmd.current_dir(dir);
                        }
                        cmd
                    },
                    Some(output_callback),
                    None, // operation_type not available at this level
                    None, // change_id not available at this level
                )
                .await;

            match result {
                Ok((status, _stderr)) => {
                    let _ = status_tx.send(status);
                }
                Err(e) => {
                    tracing::error!("Command retry failed: {}", e);
                    // Send a synthetic error status
                    #[cfg(unix)]
                    let status = std::os::unix::process::ExitStatusExt::from_raw(1);
                    #[cfg(windows)]
                    let status = std::os::windows::process::ExitStatusExt::from_raw(1);
                    let _ = status_tx.send(status);
                }
            }
        });

        // Create a dummy child process that waits for stdin to close
        // When the background task completes, we'll close stdin which causes the process to exit
        let mut dummy_child = if cfg!(target_os = "windows") {
            // Use findstr with stdin - will exit when stdin closes
            Command::new("findstr")
                .arg(".*")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(OrchestratorError::Io)?
        } else {
            // Use cat with no args - reads stdin until it closes
            Command::new("cat")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(OrchestratorError::Io)?
        };

        // Take stdin so we can close it later
        let dummy_stdin = dummy_child.stdin.take();

        // Wrap dummy child in ManagedChild
        let managed_child = ManagedChild::new(dummy_child)?;

        // Spawn a task to close stdin when real command completes
        // This will cause the dummy process to exit cleanly
        tokio::spawn(async move {
            // Wait for status from background task
            let _ = status_rx.await;
            // Close stdin - this causes cat/findstr to exit cleanly
            drop(dummy_stdin);
            // Close output channel to signal completion
            drop(tx);
        });

        Ok((managed_child, rx))
    }

    /// Execute a command with streaming output and stagger delay.
    ///
    /// This is the core execution method used by all AI-driven commands.
    /// It spawns the command through CommandQueue (with stagger), then
    /// streams stdout/stderr to an mpsc channel.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute (will be run via `sh -c`)
    /// * `cwd` - Optional working directory (for worktree execution)
    ///
    /// # Returns
    ///
    /// A tuple of (ManagedChild, Receiver<OutputLine>) for process control and output streaming
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 3.2, 3.3, 4.1-4.3)
    pub async fn execute_streaming(
        &self,
        command: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        // Execute with stagger via CommandQueue
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).stdin(Stdio::null());

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        debug!(
            module = module_path!(),
            "Executing shell command with stagger: sh -c {} (cwd: {:?})", command, cwd
        );

        let child = self
            .command_queue
            .execute_with_stagger(move || {
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(command)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(dir) = cwd {
                    cmd.current_dir(dir);
                }
                cmd
            })
            .await?;

        // Wrap in ManagedChild for proper cleanup
        let mut managed = ManagedChild::new(child)?;

        // Take stdout/stderr from the child field
        let stdout = managed.child.stdout.take().ok_or_else(|| {
            OrchestratorError::AgentCommand(format!(
                "Failed to capture stdout for command '{}' (cwd: {:?})",
                command, cwd
            ))
        })?;
        let stderr = managed.child.stderr.take().ok_or_else(|| {
            OrchestratorError::AgentCommand(format!(
                "Failed to capture stderr for command '{}' (cwd: {:?})",
                command, cwd
            ))
        })?;

        // Create channel for output streaming
        let (tx, rx) = mpsc::channel(1024);

        // Spawn stdout reader
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx_stdout.send(OutputLine::Stdout(line)).await.is_err() {
                    break;
                }
            }
        });

        // Spawn stderr reader
        let tx_stderr = tx;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if tx_stderr.send(OutputLine::Stderr(line)).await.is_err() {
                    break;
                }
            }
        });

        Ok((managed, rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::*;

    #[tokio::test]
    async fn test_shared_stagger_state() {
        let shared_state = Arc::new(Mutex::new(None));

        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
        };

        let runner1 = AiCommandRunner::new(config.clone(), shared_state.clone());
        let runner2 = AiCommandRunner::new(config.clone(), shared_state.clone());

        // Execute first command
        let start = Instant::now();
        let (mut child1, _rx1) = runner1.execute_streaming("echo test1", None).await.unwrap();
        let _ = child1.wait().await;

        // Execute second command - should wait for stagger
        let (mut child2, _rx2) = runner2.execute_streaming("echo test2", None).await.unwrap();
        let elapsed = start.elapsed();
        let _ = child2.wait().await;

        // Second command should have waited at least 100ms
        assert!(
            elapsed.as_millis() >= 90,
            "Stagger delay not applied: {:?}",
            elapsed
        );
    }
}
