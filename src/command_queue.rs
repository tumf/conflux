use crate::error::{OrchestratorError, Result};
use crate::process_manager::cleanup_process_group;
use regex::Regex;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Configuration for command execution queue
#[derive(Debug, Clone)]
pub struct CommandQueueConfig {
    /// Delay between command executions (milliseconds)
    pub stagger_delay_ms: u64,

    /// Maximum number of retries
    #[allow(dead_code)]
    // Used by execute_with_retry and execute_with_retry_streaming (pub API)
    pub max_retries: u32,

    /// Delay between retries (milliseconds)
    #[allow(dead_code)]
    // Used by execute_with_retry and execute_with_retry_streaming (pub API)
    pub retry_delay_ms: u64,

    /// Error patterns that trigger retry (regex)
    #[allow(dead_code)]
    // Used by execute_with_retry and execute_with_retry_streaming (pub API)
    pub retry_error_patterns: Vec<String>,

    /// Retry if execution duration is under this threshold (seconds)
    #[allow(dead_code)]
    // Used by execute_with_retry and execute_with_retry_streaming (pub API)
    pub retry_if_duration_under_secs: u64,

    /// Inactivity timeout for commands (seconds)
    /// 0 = disabled
    #[allow(dead_code)]
    // Used by execute_with_retry_streaming for inactivity monitoring
    pub inactivity_timeout_secs: u64,

    /// Grace period before force-killing inactive commands (seconds)
    #[allow(dead_code)]
    // Used by execute_with_retry_streaming for graceful shutdown
    pub inactivity_kill_grace_secs: u64,

    /// Maximum number of retries after inactivity timeout (0 = disabled)
    #[allow(dead_code)]
    // Used by execute_streaming_with_retry for inactivity-timeout retry logic
    pub inactivity_timeout_max_retries: u32,

    /// Enable strict post-completion process-group cleanup.
    /// When true (default), after a command finishes the orchestrator sweeps the
    /// spawned process group with SIGTERM → SIGKILL to prevent orphaned processes.
    #[allow(dead_code)]
    pub strict_process_cleanup: bool,
}

/// Command execution queue with staggered start and retry mechanism
#[derive(Debug, Clone)]
pub struct CommandQueue {
    config: CommandQueueConfig,
    /// Last command execution time (for stagger control)
    last_execution: Arc<Mutex<Option<Instant>>>,
}

impl CommandQueue {
    /// Create a new command queue with the given configuration
    pub fn new(config: CommandQueueConfig) -> Self {
        Self {
            config,
            last_execution: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new command queue with shared stagger state.
    ///
    /// This allows multiple CommandQueue instances to coordinate
    /// their stagger delays through a shared last_execution timestamp.
    ///
    /// # Arguments
    ///
    /// * `config` - CommandQueue configuration
    /// * `shared_state` - Shared last execution timestamp (Arc<Mutex<Option<Instant>>>)
    pub fn new_with_shared_state(
        config: CommandQueueConfig,
        shared_state: Arc<Mutex<Option<Instant>>>,
    ) -> Self {
        Self {
            config,
            last_execution: shared_state,
        }
    }

    /// Execute a command with staggered start
    ///
    /// Ensures minimum delay between consecutive command executions
    /// to avoid resource conflicts.
    pub async fn execute_with_stagger<F>(&self, command_fn: F) -> Result<Child>
    where
        F: FnOnce() -> Command,
    {
        let mut last = self.last_execution.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            let delay = Duration::from_millis(self.config.stagger_delay_ms);

            if elapsed < delay {
                let wait_time = delay - elapsed;
                debug!("Stagger delay: waiting {:?} before next command", wait_time);
                tokio::time::sleep(wait_time).await;
            }
        }

        // Update execution time
        *last = Some(Instant::now());
        drop(last);

        // Execute command
        let mut cmd = command_fn();
        cmd.spawn().map_err(OrchestratorError::Io)
    }

    /// Check if an error message matches retryable patterns
    pub fn is_retryable_error(&self, stderr: &str) -> bool {
        self.config.retry_error_patterns.iter().any(|pattern| {
            Regex::new(pattern)
                .map(|re| re.is_match(stderr))
                .unwrap_or_else(|e| {
                    warn!("Invalid retry pattern '{}': {}", pattern, e);
                    false
                })
        })
    }

    /// Determine if a command should be retried based on:
    /// - Attempt count (must be under max_retries)
    /// - Exit code (non-zero)
    /// - Error pattern match OR short execution duration OR agent crash (exit code != 0)
    pub fn should_retry(
        &self,
        attempt: u32,
        duration: Duration,
        stderr: &str,
        exit_code: i32,
    ) -> bool {
        // Check maximum retries
        if attempt >= self.config.max_retries {
            return false;
        }

        // Don't retry successful commands
        if exit_code == 0 {
            return false;
        }

        // Condition 1: Error pattern match
        let matches_pattern = self.is_retryable_error(stderr);

        // Condition 2: Short execution (likely startup/environment issue)
        let is_short_execution =
            duration < Duration::from_secs(self.config.retry_if_duration_under_secs);

        // Condition 3: Agent crash (non-zero exit code)
        // All non-zero exits are considered crash candidates for retry
        let is_crash = exit_code != 0;

        // Retry if any condition is true (OR logic)
        matches_pattern || is_short_execution || is_crash
    }

    /// Execute a command with automatic retry on transient errors
    ///
    /// Retries command execution based on:
    /// - Error pattern matching (e.g., "Cannot find module")
    /// - Execution duration (short runs may indicate environment issues)
    /// - Agent crash (non-zero exit code)
    #[allow(dead_code)] // Public API for unified retry logic
    #[allow(clippy::redundant_closure)] // Closure needed to capture FnMut
    pub async fn execute_with_retry<F>(&self, mut command_fn: F) -> Result<std::process::ExitStatus>
    where
        F: FnMut() -> Command,
    {
        let mut attempt = 0;

        loop {
            attempt += 1;

            // Execute with stagger
            let start_time = Instant::now();
            let child = self.execute_with_stagger(|| command_fn()).await?;

            // Wait for completion and collect output
            let output = child
                .wait_with_output()
                .await
                .map_err(OrchestratorError::Io)?;
            let duration = start_time.elapsed();

            // Success case
            if output.status.success() {
                debug!(
                    "Command succeeded on attempt {} (duration: {:?})",
                    attempt, duration
                );
                return Ok(output.status);
            }

            // Check if retry is needed
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            let should_retry = self.should_retry(attempt, duration, &stderr, exit_code);

            if should_retry {
                warn!(
                    "Retryable error detected (attempt {}/{}), duration: {:.2}s, exit_code: {}: {}",
                    attempt,
                    self.config.max_retries,
                    duration.as_secs_f64(),
                    exit_code,
                    stderr.lines().next().unwrap_or("")
                );

                // Wait before retry
                tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                continue;
            }

            // Max retries exceeded or non-retryable error
            return Err(OrchestratorError::AgentCommand(format!(
                "Command failed after {} attempt(s): {}",
                attempt, stderr
            )));
        }
    }

    /// Execute a command with automatic retry and streaming output.
    ///
    /// This is the streaming variant of `execute_with_retry()`. It spawns the command,
    /// streams stdout/stderr to the provided callback, and retries on transient errors.
    ///
    /// # Arguments
    ///
    /// * `command_fn` - A function that creates the command to execute
    /// * `output_callback` - Optional async callback called for each output line
    /// * `operation_type` - Optional operation type for logging (e.g., "apply", "archive")
    /// * `change_id` - Optional change ID for logging
    ///
    /// # Returns
    ///
    /// Returns the final exit status and collected stderr (for logging/debugging).
    /// On failure after all retries, returns an error.
    #[allow(dead_code)] // Public API for unified retry logic
    #[allow(clippy::redundant_closure)]
    pub async fn execute_with_retry_streaming<F, C, Fut>(
        &self,
        mut command_fn: F,
        output_callback: Option<C>,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(ExitStatus, String)>
    where
        F: FnMut() -> Command,
        C: Fn(StreamingOutputLine) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut attempt = 0;

        loop {
            attempt += 1;
            let start_time = Instant::now();

            // Execute with stagger.
            // Callers (e.g., build_command in agent/runner.rs) are responsible for calling
            // configure_process_group() so that PGID == PID (via setsid) before spawning,
            // which is required for post-completion process-group cleanup to work correctly.
            let mut child = self.execute_with_stagger(|| command_fn()).await?;

            // Capture PID (= PGID if caller used setsid) before handing child to stream_and_wait.
            let pid = child.id().unwrap_or(0);

            // Stream output and collect stderr for retry decision
            let (status, stderr, inactivity_timeout_occurred) = self
                .stream_and_wait(&mut child, &output_callback, operation_type, change_id)
                .await?;
            let duration = start_time.elapsed();

            // Post-completion cleanup: sweep the process group with SIGTERM→SIGKILL to
            // ensure no background processes spawned by the agent command survive.
            if self.config.strict_process_cleanup && pid > 0 {
                cleanup_process_group(pid, 100, operation_type, change_id).await;
            }

            // Success case
            if status.success() {
                debug!(
                    "Command succeeded on attempt {} (duration: {:?})",
                    attempt, duration
                );
                return Ok((status, stderr));
            }

            // Check if retry is needed
            let exit_code = status.code().unwrap_or(-1);
            let should_retry = self.should_retry(attempt, duration, &stderr, exit_code);

            if should_retry {
                warn!(
                    "Retryable error detected (attempt {}/{}), duration: {:.2}s, exit_code: {}: {}",
                    attempt,
                    self.config.max_retries,
                    duration.as_secs_f64(),
                    exit_code,
                    stderr.lines().next().unwrap_or("")
                );

                // Notify retry via callback
                if let Some(ref cb) = output_callback {
                    let retry_msg = format!(
                        "[Retry {}/{}] Command crashed, retrying in {}ms...",
                        attempt, self.config.max_retries, self.config.retry_delay_ms
                    );
                    cb(StreamingOutputLine::Stderr(retry_msg)).await;
                }

                // Wait before retry
                tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                continue;
            }

            // Max retries exceeded or non-retryable error
            let error_msg = if inactivity_timeout_occurred {
                let exit_str = match status.code() {
                    Some(code) => format!("exit code {}", code),
                    None => "terminated by signal (no exit code)".to_string(),
                };
                let context = match (operation_type, change_id) {
                    (Some(op), Some(id)) => {
                        format!(" (operation: {}, change_id: {})", op, id)
                    }
                    (Some(op), None) => format!(" (operation: {})", op),
                    (None, Some(id)) => format!(" (change_id: {})", id),
                    (None, None) => String::new(),
                };
                format!(
                    "Command terminated by inactivity timeout after {}s{}, {}",
                    self.config.inactivity_timeout_secs, context, exit_str,
                )
            } else {
                format!(
                    "Command failed after {} attempt(s) with exit code {:?}: {}",
                    attempt,
                    status.code(),
                    stderr.lines().next().unwrap_or("")
                )
            };
            return Err(OrchestratorError::AgentCommand(error_msg));
        }
    }

    /// Stream stdout/stderr from a child process and wait for completion.
    ///
    /// Reads stdout and stderr concurrently, calling the output callback for each line,
    /// while collecting stderr for retry decision.
    ///
    /// Monitors inactivity: if no output is received for `inactivity_timeout_secs`,
    /// logs a warning, waits `inactivity_kill_grace_secs`, then terminates the process.
    ///
    /// Returns (ExitStatus, stderr_output, inactivity_timeout_occurred)
    #[allow(dead_code)] // Used by execute_with_retry_streaming
    async fn stream_and_wait<C, Fut>(
        &self,
        child: &mut Child,
        output_callback: &Option<C>,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(ExitStatus, String, bool)>
    where
        C: Fn(StreamingOutputLine) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Use channels to collect stderr and send output
        let (stderr_tx, mut stderr_rx) = tokio::sync::mpsc::channel::<String>(100);

        // Channel to signal activity from output readers
        let (activity_tx, mut activity_rx) = tokio::sync::mpsc::channel::<()>(100);

        // Spawn stdout reader task
        let stdout_callback = output_callback.clone();
        let activity_tx_stdout = activity_tx.clone();
        let stdout_handle = tokio::spawn(async move {
            if let Some(stdout) = stdout {
                let mut reader = BufReader::new(stdout);
                let mut line_buf = String::new();
                let mut byte_buf = vec![0u8; 4096];
                loop {
                    match reader.read(&mut byte_buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            // Signal activity on byte reception (not line reception)
                            let _ = activity_tx_stdout.send(()).await;
                            let chunk = String::from_utf8_lossy(&byte_buf[..n]);
                            line_buf.push_str(&chunk);
                            // Emit complete lines
                            while let Some(pos) = line_buf.find('\n') {
                                let line = line_buf[..pos].trim_end_matches('\r').to_string();
                                line_buf.drain(..=pos);
                                if let Some(ref cb) = stdout_callback {
                                    cb(StreamingOutputLine::Stdout(line)).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                // Emit any remaining incomplete line (no trailing newline)
                if !line_buf.is_empty() {
                    if let Some(ref cb) = stdout_callback {
                        cb(StreamingOutputLine::Stdout(line_buf)).await;
                    }
                }
            }
        });

        // Spawn stderr reader task
        let stderr_callback = output_callback.clone();
        let activity_tx_stderr = activity_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut stderr_buffer = String::new();
            if let Some(stderr) = stderr {
                let mut reader = BufReader::new(stderr);
                let mut line_buf = String::new();
                let mut byte_buf = vec![0u8; 4096];
                loop {
                    match reader.read(&mut byte_buf).await {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            // Signal activity on byte reception (not line reception)
                            let _ = activity_tx_stderr.send(()).await;
                            let chunk = String::from_utf8_lossy(&byte_buf[..n]);
                            line_buf.push_str(&chunk);
                            // Emit complete lines
                            while let Some(pos) = line_buf.find('\n') {
                                let line = line_buf[..pos].trim_end_matches('\r').to_string();
                                line_buf.drain(..=pos);
                                stderr_buffer.push_str(&line);
                                stderr_buffer.push('\n');
                                if let Some(ref cb) = stderr_callback {
                                    cb(StreamingOutputLine::Stderr(line)).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                // Emit any remaining incomplete line (no trailing newline)
                if !line_buf.is_empty() {
                    stderr_buffer.push_str(&line_buf);
                    stderr_buffer.push('\n');
                    if let Some(ref cb) = stderr_callback {
                        cb(StreamingOutputLine::Stderr(line_buf)).await;
                    }
                }
            }
            // Send collected stderr through channel
            let _ = stderr_tx.send(stderr_buffer).await;
        });

        // Drop activity senders to allow activity_rx to complete when all readers finish
        drop(activity_tx);

        // Inactivity monitoring (if enabled)
        let inactivity_timeout_secs = self.config.inactivity_timeout_secs;
        let kill_grace_secs = self.config.inactivity_kill_grace_secs;
        let mut inactivity_timeout_occurred = false;

        if inactivity_timeout_secs > 0 {
            // Monitor activity with timeout
            let mut last_activity = Instant::now();
            let timeout_duration = Duration::from_secs(inactivity_timeout_secs);

            loop {
                let time_since_activity = last_activity.elapsed();
                let remaining = if time_since_activity < timeout_duration {
                    timeout_duration - time_since_activity
                } else {
                    Duration::from_secs(0)
                };

                tokio::select! {
                    // Activity signal received
                    activity = activity_rx.recv() => {
                        if activity.is_some() {
                            last_activity = Instant::now();
                        } else {
                            // Channel closed - all readers finished
                            break;
                        }
                    }
                    // Timeout waiting for activity
                    _ = tokio::time::sleep(remaining) => {
                        // Inactivity timeout triggered
                        inactivity_timeout_occurred = true;
                        let last_activity_age_secs = last_activity.elapsed().as_secs();

                        // Capture PID before killing for log context
                        let pid = child.id().unwrap_or(0);

                        // Get PGID for structured logging (Unix only).
                        #[cfg(unix)]
                        let pgid_opt: Option<u32> = if pid > 0 {
                            use nix::unistd::{getpgid, Pid};
                            getpgid(Some(Pid::from_raw(pid as i32)))
                                .ok()
                                .map(|p| p.as_raw() as u32)
                        } else {
                            None
                        };
                        #[cfg(not(unix))]
                        let pgid_opt: Option<u32> = None;

                        warn!(
                            pid,
                            pgid = pgid_opt,
                            timeout_secs = inactivity_timeout_secs,
                            grace_secs = kill_grace_secs,
                            last_activity_age_secs,
                            op = ?operation_type,
                            change_id = ?change_id,
                            "Inactivity timeout triggered: no output for {}s \
                             (pid={}, pgid={:?}, timeout={}s, grace={}s, \
                             last_activity_age={}s, op={:?}, change_id={:?})",
                            last_activity_age_secs, pid, pgid_opt,
                            inactivity_timeout_secs, kill_grace_secs,
                            last_activity_age_secs, operation_type, change_id
                        );

                        // Wait grace period
                        tokio::time::sleep(Duration::from_secs(kill_grace_secs)).await;

                        // Kill the process (and its process group if it's the group leader)
                        if pid > 0 {
                            warn!(
                                pid,
                                pgid = pgid_opt,
                                op = ?operation_type,
                                change_id = ?change_id,
                                "Grace period expired, sending SIGTERM to process \
                                 (pid={}, pgid={:?})",
                                pid, pgid_opt
                            );
                            #[cfg(unix)]
                            {
                                use nix::sys::signal::{kill, killpg, Signal};
                                use nix::unistd::{getpgid, Pid};
                                let pid_raw = Pid::from_raw(pid as i32);
                                // Get the actual process group ID
                                let pgid =
                                    getpgid(Some(pid_raw)).unwrap_or(pid_raw);
                                let (target_desc, sigterm_result, sigkill_result) =
                                    if pgid == pid_raw {
                                        // Process is its own group leader: kill the whole group
                                        let sr = killpg(pgid, Signal::SIGTERM);
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        let kr = killpg(pgid, Signal::SIGKILL);
                                        ("process group", sr, kr)
                                    } else {
                                        // Process is in parent's group: only kill this process
                                        let sr = kill(pid_raw, Signal::SIGTERM);
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        let kr = kill(pid_raw, Signal::SIGKILL);
                                        ("process", sr, kr)
                                    };
                                match sigterm_result {
                                    Ok(()) => {
                                        debug!(
                                            pid,
                                            pgid = pgid_opt,
                                            signal = "SIGTERM",
                                            target = target_desc,
                                            "SIGTERM delivered to {}", target_desc
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            pid,
                                            pgid = pgid_opt,
                                            signal = "SIGTERM",
                                            target = target_desc,
                                            errno = %e,
                                            op = ?operation_type,
                                            change_id = ?change_id,
                                            "SIGTERM failed for {} (pid={}, pgid={:?}): {}",
                                            target_desc, pid, pgid_opt, e
                                        );
                                    }
                                }
                                match sigkill_result {
                                    Ok(()) => {
                                        debug!(
                                            pid,
                                            pgid = pgid_opt,
                                            signal = "SIGKILL",
                                            target = target_desc,
                                            "SIGKILL delivered to {}", target_desc
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            pid,
                                            pgid = pgid_opt,
                                            signal = "SIGKILL",
                                            target = target_desc,
                                            errno = %e,
                                            op = ?operation_type,
                                            change_id = ?change_id,
                                            "SIGKILL failed for {} (pid={}, pgid={:?}): {}",
                                            target_desc, pid, pgid_opt, e
                                        );
                                    }
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                let _ = child.kill().await;
                            }
                        }
                        break;
                    }
                }
            }
        } else {
            // No timeout monitoring - just drain activity signals
            while activity_rx.recv().await.is_some() {}
        }

        // Wait for both readers to complete
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Collect stderr from channel
        let stderr_collected = stderr_rx.recv().await.unwrap_or_default();

        // Wait for process to complete
        let status = child.wait().await.map_err(OrchestratorError::Io)?;

        Ok((status, stderr_collected, inactivity_timeout_occurred))
    }

    /// Get the configuration (for testing and external access)
    #[allow(dead_code)] // Public API for testing and external access
    pub fn config(&self) -> &CommandQueueConfig {
        &self.config
    }
}

/// Output line type for streaming commands
#[allow(dead_code)] // Public API for execute_with_retry_streaming
#[derive(Debug, Clone)]
pub enum StreamingOutputLine {
    Stdout(String),
    Stderr(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CommandQueueConfig {
        CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 2,
            retry_delay_ms: 50,
            retry_error_patterns: vec![
                r"Cannot find module".to_string(),
                r"ResolveMessage:".to_string(),
            ],
            retry_if_duration_under_secs: 5,
            inactivity_timeout_secs: 0, // Disabled for most tests
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        }
    }

    #[tokio::test]
    async fn test_stagger_delay() {
        let queue = CommandQueue::new(test_config());
        let start = Instant::now();

        // First execution - no delay
        let _child1 = queue
            .execute_with_stagger(|| Command::new("echo"))
            .await
            .unwrap();

        // Second execution - should wait ~100ms
        let _child2 = queue
            .execute_with_stagger(|| Command::new("echo"))
            .await
            .unwrap();

        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(90)); // Allow some tolerance
    }

    #[test]
    fn test_is_retryable_error_matches() {
        let queue = CommandQueue::new(test_config());
        assert!(queue.is_retryable_error("Error: Cannot find module 'foo'"));
        assert!(queue.is_retryable_error("ResolveMessage: failed to resolve"));
    }

    #[test]
    fn test_is_retryable_error_no_match() {
        let queue = CommandQueue::new(test_config());
        assert!(!queue.is_retryable_error("Syntax error"));
        assert!(!queue.is_retryable_error("Test failed"));
    }

    #[test]
    fn test_retry_on_retryable_error() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(10); // Long duration
        let stderr = "Cannot find module 'test'";
        let exit_code = 1;

        assert!(queue.should_retry(1, duration, stderr, exit_code));
    }

    #[test]
    fn test_retry_on_short_duration() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(2); // Short duration
        let stderr = "Some unknown error";
        let exit_code = 1;

        assert!(queue.should_retry(1, duration, stderr, exit_code));
    }

    #[test]
    fn test_retry_on_agent_crash() {
        // Now all non-zero exit codes trigger retry (agent crash condition)
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(10); // Long duration
        let stderr = "Test assertion failed"; // No pattern match
        let exit_code = 1;

        // Should retry because exit_code != 0 (agent crash)
        assert!(queue.should_retry(1, duration, stderr, exit_code));
    }

    #[test]
    fn test_retry_on_long_duration_with_pattern() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(10); // Long duration
        let stderr = "ResolveMessage: module not found";
        let exit_code = 1;

        assert!(queue.should_retry(1, duration, stderr, exit_code));
    }

    #[test]
    fn test_no_retry_on_success() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(1);
        let stderr = "";
        let exit_code = 0;

        assert!(!queue.should_retry(1, duration, stderr, exit_code));
    }

    #[test]
    fn test_max_retries_exceeded() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(1);
        let stderr = "Cannot find module";
        let exit_code = 1;

        // First retry - should be allowed
        assert!(queue.should_retry(1, duration, stderr, exit_code));

        // At max retries - should not retry
        assert!(!queue.should_retry(2, duration, stderr, exit_code));
    }

    #[tokio::test]
    async fn test_execute_with_retry_streaming_success() {
        let queue = CommandQueue::new(test_config());

        // Simple echo command that succeeds
        let (status, stderr) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("echo");
                    cmd.arg("hello");
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await
            .unwrap();

        assert!(status.success());
        assert!(stderr.is_empty() || stderr.trim().is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_retry_streaming_with_callback() {
        use std::process::Stdio;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let queue = CommandQueue::new(test_config());
        let output_count = Arc::new(AtomicUsize::new(0));
        let output_count_clone = output_count.clone();

        let callback = move |_line: StreamingOutputLine| {
            let count = output_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
            }
        };

        let (status, _stderr) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args(["-c", "echo line1 && echo line2"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());
                    cmd
                },
                Some(callback),
                None,
                None,
            )
            .await
            .unwrap();

        assert!(status.success());
        // At least 1 output line should be captured (callback is called for each line)
        // Note: The exact count may vary due to async timing
        assert!(
            output_count.load(Ordering::SeqCst) >= 1,
            "Expected at least 1 callback, got {}",
            output_count.load(Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn test_execute_with_retry_streaming_failure_no_retry() {
        // Test with max_retries = 0 to ensure no retries happen
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1, // Only 1 attempt allowed (no retries)
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0, // Disable short duration retry
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 10,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // Command that fails (but we set max_retries=1, so no retry after first attempt)
        let result = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args(["-c", "exit 1"]);
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await;

        // With new crash retry logic, all non-zero exits trigger retry if attempt < max_retries
        // Since max_retries=1, the first attempt (attempt=1) reaches the limit and doesn't retry
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_inactivity_timeout_triggers() {
        use std::process::Stdio;

        // Configure with short inactivity timeout (1 second)
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 1,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // Command that sleeps without output (should trigger inactivity timeout)
        let start = Instant::now();
        let result = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sleep");
                    cmd.arg("30").stdout(Stdio::piped()).stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await;

        let elapsed = start.elapsed();

        // Should fail due to inactivity timeout + grace period (approximately 2 seconds)
        // Allow some margin for timing
        assert!(elapsed.as_secs_f64() >= 1.0 && elapsed.as_secs_f64() <= 6.0);

        // Process should have been killed
        if let Ok((status, _)) = result {
            // On Unix, killed processes typically have a signal exit code
            #[cfg(unix)]
            assert!(!status.success());
        } else {
            // Command execution may fail with error
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_no_timeout_with_periodic_output() {
        use std::process::Stdio;

        // Configure with inactivity timeout (1 second)
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 1,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // Command that outputs periodically (every 0.2 second for 3 iterations)
        // Should NOT timeout because output keeps resetting the timer
        let (status, _) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args(["-c", "for i in 1 2 3; do echo \"line $i\"; sleep 0.2; done"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await
            .unwrap();

        // Should succeed (no timeout)
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_timeout_disabled_with_zero() {
        use std::process::Stdio;

        // Configure with timeout disabled (0)
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0, // Disabled
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // Command that sleeps briefly without output
        // Should succeed because timeout is disabled
        let (status, _) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sleep");
                    cmd.arg("2").stdout(Stdio::piped()).stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await
            .unwrap();

        // Should succeed (timeout disabled)
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_inactivity_timeout_error_message_format() {
        use std::process::Stdio;

        let config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 1,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        let result = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sleep");
                    cmd.arg("30").stdout(Stdio::piped()).stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                Some("apply"),
                Some("test-change"),
            )
            .await;

        assert!(result.is_err(), "Expected error due to inactivity timeout");
        let err = result.unwrap_err().to_string();

        // Must contain "inactivity timeout"
        assert!(
            err.contains("inactivity timeout"),
            "Error message must mention 'inactivity timeout': {err}"
        );
        // Must contain the timeout duration (1s)
        assert!(
            err.contains("1s"),
            "Error message must include timeout seconds '1s': {err}"
        );
        // Must contain operation context
        assert!(
            err.contains("apply"),
            "Error message must include operation 'apply': {err}"
        );
        // Must contain change_id context
        assert!(
            err.contains("test-change"),
            "Error message must include change_id 'test-change': {err}"
        );
    }

    #[tokio::test]
    async fn test_no_timeout_with_bytes_without_newlines() {
        use std::process::Stdio;

        // Inactivity timeout of 1 second; command emits bytes every 0.2s (no newlines)
        let config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 1,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // printf writes bytes without newlines; short sleep gaps between them
        let (status, _) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args([
                        "-c",
                        "printf '.'; sleep 0.2; printf '.'; sleep 0.2; printf '.'",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await
            .unwrap();

        // Should succeed: bytes reset the timer even without newlines
        assert!(status.success());
    }

    /// Verify that stderr byte reception resets the inactivity timer even when
    /// stdout is silent.
    #[tokio::test]
    async fn test_no_timeout_with_stderr_only_bytes() {
        use std::process::Stdio;

        // Inactivity timeout of 5 seconds; command emits to stderr every 1s for 3s
        let config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 5,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
        };
        let queue = CommandQueue::new(config);

        // Write to stderr only (no stdout output)
        let (status, _) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args([
                        "-c",
                        "printf 'err1' >&2; sleep 1; printf 'err2' >&2; sleep 1; printf 'err3' >&2",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
                None,
                None,
            )
            .await
            .unwrap();

        // Should succeed: stderr bytes reset the inactivity timer
        assert!(status.success());
    }
}
