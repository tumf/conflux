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
            OrchestratorError::AgentCommand("Failed to capture stdout".to_string())
        })?;
        let stderr = managed.child.stderr.take().ok_or_else(|| {
            OrchestratorError::AgentCommand("Failed to capture stderr".to_string())
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
