use crate::error::{OrchestratorError, Result};
use regex::Regex;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
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
    #[allow(dead_code)] // Public API for unified retry logic
    fn is_retryable_error(&self, stderr: &str) -> bool {
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
    #[allow(dead_code)] // Public API for unified retry logic
    fn should_retry(&self, attempt: u32, duration: Duration, stderr: &str, exit_code: i32) -> bool {
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

            // Execute with stagger
            let mut child = self.execute_with_stagger(|| command_fn()).await?;

            // Stream output and collect stderr for retry decision
            let (status, stderr) = self.stream_and_wait(&mut child, &output_callback).await?;
            let duration = start_time.elapsed();

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
            return Err(OrchestratorError::AgentCommand(format!(
                "Command failed after {} attempt(s) with exit code {:?}: {}",
                attempt,
                status.code(),
                stderr.lines().next().unwrap_or("")
            )));
        }
    }

    /// Stream stdout/stderr from a child process and wait for completion.
    ///
    /// Reads stdout and stderr concurrently, calling the output callback for each line,
    /// while collecting stderr for retry decision.
    ///
    /// Monitors inactivity: if no output is received for `inactivity_timeout_secs`,
    /// logs a warning, waits `inactivity_kill_grace_secs`, then terminates the process.
    #[allow(dead_code)] // Used by execute_with_retry_streaming
    async fn stream_and_wait<C, Fut>(
        &self,
        child: &mut Child,
        output_callback: &Option<C>,
    ) -> Result<(ExitStatus, String)>
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
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Signal activity
                    let _ = activity_tx_stdout.send(()).await;
                    if let Some(ref cb) = stdout_callback {
                        cb(StreamingOutputLine::Stdout(line)).await;
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
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Signal activity
                    let _ = activity_tx_stderr.send(()).await;
                    stderr_buffer.push_str(&line);
                    stderr_buffer.push('\n');
                    if let Some(ref cb) = stderr_callback {
                        cb(StreamingOutputLine::Stderr(line)).await;
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
                        warn!(
                            "Command inactivity timeout triggered after {} seconds, waiting {} seconds grace period before terminating",
                            inactivity_timeout_secs,
                            kill_grace_secs
                        );

                        // Wait grace period
                        tokio::time::sleep(Duration::from_secs(kill_grace_secs)).await;

                        // Check if process is still running
                        if child.id().is_some() {
                            warn!("Grace period expired, terminating inactive process");
                            // Kill the process
                            let _ = child.kill().await;
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

        Ok((status, stderr_collected))
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
            )
            .await;

        // With new crash retry logic, all non-zero exits trigger retry if attempt < max_retries
        // Since max_retries=1, the first attempt (attempt=1) reaches the limit and doesn't retry
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_inactivity_timeout_triggers() {
        use std::process::Stdio;

        // Configure with short inactivity timeout (3 seconds)
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 3,
            inactivity_kill_grace_secs: 1,
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
            )
            .await;

        let elapsed = start.elapsed();

        // Should fail due to inactivity timeout + grace period (approximately 4 seconds)
        // Allow some margin for timing
        assert!(elapsed.as_secs() >= 3 && elapsed.as_secs() <= 10);

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

        // Configure with inactivity timeout (5 seconds)
        let config = CommandQueueConfig {
            stagger_delay_ms: 100,
            max_retries: 1,
            retry_delay_ms: 50,
            retry_error_patterns: vec![],
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 5,
            inactivity_kill_grace_secs: 1,
        };
        let queue = CommandQueue::new(config);

        // Command that outputs periodically (every 1 second for 3 iterations)
        // Should NOT timeout because output keeps resetting the timer
        let (status, _) = queue
            .execute_with_retry_streaming(
                || {
                    let mut cmd = Command::new("sh");
                    cmd.args(["-c", "for i in 1 2 3; do echo \"line $i\"; sleep 1; done"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());
                    cmd
                },
                None::<fn(StreamingOutputLine) -> std::future::Ready<()>>,
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
            )
            .await
            .unwrap();

        // Should succeed (timeout disabled)
        assert!(status.success());
    }
}
