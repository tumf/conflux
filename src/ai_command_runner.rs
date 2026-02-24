//! Common AI command runner layer for unified stagger state management.
//!
//! This module provides a shared execution layer for all AI-driven commands
//! (apply, archive, resolve, analyze) to ensure consistent stagger delays
//! across parallel and serial execution modes.

use crate::command_queue::{CommandQueue, CommandQueueConfig};
use crate::error::{OrchestratorError, Result};
use crate::process_manager::{ManagedChild, StreamingChildHandle};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

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
    /// Returns a real process handle ([`StreamingChildHandle`]) that targets the actual
    /// spawned command (or its process group) rather than a placeholder. Cancellation and
    /// inactivity-timeout termination send SIGTERM/SIGKILL to the full process group, so
    /// pipeline children (e.g. `claude | jq`) cannot be left as orphans.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute (run via `sh -c`)
    /// * `cwd` - Optional working directory (for worktree execution)
    /// * `operation_type` - Optional operation type for logging (apply/archive/resolve/analyze/acceptance)
    /// * `change_id` - Optional change ID for logging context
    ///
    /// # Returns
    ///
    /// A tuple of (`StreamingChildHandle`, `Receiver<OutputLine>`). Drain the receiver first
    /// (it closes when all retries complete), then call `.wait()` on the handle to obtain
    /// the final exit status.
    ///
    /// # Retry Behaviour
    ///
    /// Retries are governed by the `CommandQueueConfig`:
    /// - Error pattern matching (`retry_error_patterns`)
    /// - Short execution duration (`retry_if_duration_under_secs`)
    /// - Non-zero exit code (agent crash)
    ///
    /// Retry notifications are emitted as stderr lines on the output channel.
    pub async fn execute_streaming_with_retry(
        &self,
        command: &str,
        cwd: Option<&Path>,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(StreamingChildHandle, mpsc::Receiver<OutputLine>)> {
        // Output channel that callers drain while the background task streams.
        let (out_tx, out_rx) = mpsc::channel::<OutputLine>(1024);

        // Cancel signal: StreamingChildHandle.terminate() → background task.
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        // Shared current PID (0 = no process running).
        let current_pid = Arc::new(AtomicU32::new(0));

        // Completion signal: background task → StreamingChildHandle.wait().
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<std::process::ExitStatus>();

        // Clone values for the background task.
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();
        let cwd_owned = cwd.map(|p| p.to_path_buf());
        let operation_type_owned = operation_type.map(|s| s.to_string());
        let change_id_owned = change_id.map(|s| s.to_string());
        let pid_arc = current_pid.clone();

        // Spawn the background retry task. It owns the real child processes and responds
        // to the cancel signal by terminating the current process group via SIGTERM/SIGKILL.
        tokio::spawn(async move {
            let max_retries = command_queue.config().max_retries;
            let retry_delay_ms = command_queue.config().retry_delay_ms;
            let inactivity_timeout_secs = command_queue.config().inactivity_timeout_secs;
            let kill_grace_secs = command_queue.config().inactivity_kill_grace_secs;

            // cancel_rx is wrapped in Option so we can neutralise it after first use.
            let mut cancel_rx_opt = Some(cancel_rx);
            let mut cancel_observed = false;

            let mut attempt = 0u32;
            let mut final_exit_status: Option<std::process::ExitStatus> = None;

            'retry: loop {
                attempt += 1;
                let start_time = Instant::now();

                // Build the real command and attach it to a new process group so the
                // entire pipeline (sh + agent + filter) can be killed as one unit.
                let mut cmd = Command::new("sh");
                cmd.arg("-c")
                    .arg(&command_str)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                if let Some(ref dir) = cwd_owned {
                    cmd.current_dir(dir);
                }

                // Set the spawned process as its own process group leader (PGID = PID).
                // This allows killpg to reach all pipeline children.
                #[cfg(unix)]
                {
                    use crate::process_manager::configure_process_group;
                    configure_process_group(&mut cmd);
                }

                // Apply stagger delay then spawn.
                let child = match command_queue.execute_with_stagger(|| cmd).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            attempt,
                            "Failed to spawn command: {}",
                            e
                        );
                        break 'retry;
                    }
                };

                let mut managed_child = match ManagedChild::new(child) {
                    Ok(mc) => mc,
                    Err(e) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Failed to wrap child in ManagedChild: {}",
                            e
                        );
                        break 'retry;
                    }
                };

                // Publish the real PID so StreamingChildHandle.id() is accurate.
                let pid = managed_child.id().unwrap_or(0);
                pid_arc.store(pid, Ordering::SeqCst);
                debug!(
                    pid,
                    op = ?operation_type_owned,
                    change_id = ?change_id_owned,
                    attempt,
                    "Streaming child started"
                );

                // Take stdout/stderr handles before lending managed_child to the
                // inactivity/cancel select loop.
                let stdout = managed_child.child.stdout.take();
                let stderr = managed_child.child.stderr.take();

                // Activity channel: readers signal liveness to the inactivity monitor.
                let (activity_tx, mut activity_rx) = mpsc::channel::<()>(100);

                // Stderr accumulator (for retry-condition check after exit).
                let (stderr_acc_tx, mut stderr_acc_rx) = mpsc::channel::<String>(2);

                // Spawn stdout reader.
                let out_tx_stdout = out_tx.clone();
                let activity_tx_stdout = activity_tx.clone();
                let stdout_handle = tokio::spawn(async move {
                    if let Some(stdout) = stdout {
                        let mut lines = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = activity_tx_stdout.send(()).await;
                            let _ = out_tx_stdout.send(OutputLine::Stdout(line)).await;
                        }
                    }
                });

                // Spawn stderr reader.
                let out_tx_stderr = out_tx.clone();
                let activity_tx_stderr = activity_tx.clone();
                let stderr_handle = tokio::spawn(async move {
                    let mut buf = String::new();
                    if let Some(stderr) = stderr {
                        let mut lines = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = lines.next_line().await {
                            let _ = activity_tx_stderr.send(()).await;
                            buf.push_str(&line);
                            buf.push('\n');
                            let _ = out_tx_stderr.send(OutputLine::Stderr(line)).await;
                        }
                    }
                    let _ = stderr_acc_tx.send(buf).await;
                });

                // Drop the extra activity sender so the channel closes naturally when
                // both reader tasks finish.
                drop(activity_tx);

                // --- Monitoring loop: activity reset, inactivity timeout, cancellation ---
                let mut inactivity_triggered = false;

                if inactivity_timeout_secs > 0 {
                    let mut last_activity = Instant::now();
                    let timeout_dur = Duration::from_secs(inactivity_timeout_secs);

                    'watch: loop {
                        let elapsed = last_activity.elapsed();
                        let remaining = if elapsed < timeout_dur {
                            timeout_dur - elapsed
                        } else {
                            Duration::from_secs(0)
                        };

                        tokio::select! {
                            biased;

                            // Cancellation from StreamingChildHandle.terminate().
                            result = async {
                                match cancel_rx_opt {
                                    Some(ref mut rx) => rx.await,
                                    None => std::future::pending().await,
                                }
                            }, if !cancel_observed => {
                                cancel_observed = true;
                                cancel_rx_opt = None;
                                if result.is_ok() {
                                    warn!(
                                        pid,
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        "Streaming command cancelled, terminating process group (pid={})", pid
                                    );
                                    let _ = managed_child
                                        .terminate_with_timeout(Duration::from_secs(5))
                                        .await;
                                    pid_arc.store(0, Ordering::SeqCst);
                                    let _ = status_tx.send(make_fail_status());
                                    return;
                                }
                                // Err = handle was dropped without calling terminate() — continue.
                            }

                            // Output activity resets the inactivity timer.
                            a = activity_rx.recv() => {
                                if a.is_some() {
                                    last_activity = Instant::now();
                                } else {
                                    // All readers finished.
                                    break 'watch;
                                }
                            }

                            // Inactivity timeout reached.
                            _ = tokio::time::sleep(remaining) => {
                                inactivity_triggered = true;
                                warn!(
                                    pid,
                                    op = ?operation_type_owned,
                                    change_id = ?change_id_owned,
                                    "Inactivity timeout after {}s (pid={}), grace {}s then terminating process group",
                                    inactivity_timeout_secs, pid, kill_grace_secs
                                );
                                tokio::time::sleep(Duration::from_secs(kill_grace_secs)).await;
                                if managed_child.id().is_some() {
                                    warn!(
                                        pid,
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        "Grace expired, killing process group (pid={})", pid
                                    );
                                    let _ = managed_child.terminate();
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    let _ = managed_child.force_kill().await;
                                }
                                break 'watch;
                            }
                        }
                    }
                } else {
                    // No inactivity timeout — only watch for cancel and reader completion.
                    'watch_no_timeout: loop {
                        tokio::select! {
                            biased;

                            result = async {
                                match cancel_rx_opt {
                                    Some(ref mut rx) => rx.await,
                                    None => std::future::pending().await,
                                }
                            }, if !cancel_observed => {
                                cancel_observed = true;
                                cancel_rx_opt = None;
                                if result.is_ok() {
                                    warn!(
                                        pid,
                                        op = ?operation_type_owned,
                                        change_id = ?change_id_owned,
                                        "Streaming command cancelled, terminating process group (pid={})", pid
                                    );
                                    let _ = managed_child
                                        .terminate_with_timeout(Duration::from_secs(5))
                                        .await;
                                    pid_arc.store(0, Ordering::SeqCst);
                                    let _ = status_tx.send(make_fail_status());
                                    return;
                                }
                            }

                            a = activity_rx.recv() => {
                                if a.is_none() {
                                    break 'watch_no_timeout;
                                }
                            }
                        }
                    }
                }

                // Wait for readers to finish before collecting status.
                let _ = stdout_handle.await;
                let _ = stderr_handle.await;

                let stderr_collected = stderr_acc_rx.recv().await.unwrap_or_default();

                // Collect the child's exit status.
                let status = match managed_child.wait().await {
                    Ok(s) => s,
                    Err(e) => {
                        error!(
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Failed to wait for child process: {}", e
                        );
                        break 'retry;
                    }
                };

                pid_arc.store(0, Ordering::SeqCst);

                // Check whether a retry is warranted (not for inactivity-triggered exits).
                if !status.success() && !inactivity_triggered {
                    let exit_code = status.code().unwrap_or(-1);
                    let duration = start_time.elapsed();

                    if command_queue.should_retry(attempt, duration, &stderr_collected, exit_code) {
                        warn!(
                            attempt,
                            max_retries,
                            exit_code,
                            op = ?operation_type_owned,
                            change_id = ?change_id_owned,
                            "Retryable error detected, retrying in {}ms", retry_delay_ms
                        );
                        let retry_msg = format!(
                            "[Retry {}/{}] Command crashed, retrying in {}ms...",
                            attempt, max_retries, retry_delay_ms
                        );
                        let _ = out_tx.send(OutputLine::Stderr(retry_msg)).await;
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                        continue 'retry;
                    }
                }

                final_exit_status = Some(status);
                break 'retry;
            }

            // Send final exit status (failure if we exited the retry loop without one).
            let final_status = final_exit_status.unwrap_or_else(make_fail_status);
            let _ = status_tx.send(final_status);
            // Dropping out_tx closes the output channel, signalling end-of-output to callers.
        });

        let handle = StreamingChildHandle::new(cancel_tx, current_pid, status_rx);
        Ok((handle, out_rx))
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

/// Construct a synthetic failure [`std::process::ExitStatus`] for error paths.
fn make_fail_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(1)
    }
    #[cfg(not(unix))]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(1)
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

    /// Verify that execute_streaming_with_retry returns a real child PID (not 0).
    #[tokio::test]
    async fn test_streaming_with_retry_real_pid() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry("sleep 1", None, Some("test"), None)
            .await
            .unwrap();

        // Give the background task time to spawn the real child.
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The handle must expose the PID of the real child, not 0.
        let pid = handle.id();
        assert!(pid.is_some(), "Expected a real PID, got None");
        assert!(pid.unwrap() > 0, "Expected PID > 0");

        // Drain output and wait.
        while rx.recv().await.is_some() {}
        let _ = handle.wait().await;
    }

    /// Verify that terminating a pipeline via StreamingChildHandle kills the entire
    /// process group (sh + children), leaving no orphaned processes.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_streaming_with_retry_terminates_pipeline() {
        let shared_state = Arc::new(Mutex::new(None));
        let config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
        };
        let runner = AiCommandRunner::new(config, shared_state);

        // Pipeline: sleep 999 | cat — both processes should be killed by terminate().
        let (mut handle, _rx) = runner
            .execute_streaming_with_retry("sleep 999 | cat", None, Some("test"), None)
            .await
            .unwrap();

        // Wait for the child to be spawned.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let pid = handle.id();
        assert!(pid.is_some(), "Should have a real PID");

        // Terminate the process group.
        let outcome = handle
            .terminate_with_timeout(Duration::from_secs(5))
            .await
            .unwrap();

        // Process should have exited (not timed out).
        assert!(
            !matches!(
                outcome,
                crate::process_manager::TerminationOutcome::TimedOut
            ),
            "Expected process to exit after termination, got TimedOut"
        );
    }
}
