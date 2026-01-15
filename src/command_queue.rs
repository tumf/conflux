use crate::error::{OrchestratorError, Result};
use regex::Regex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Configuration for command execution queue
#[derive(Debug, Clone)]
pub struct CommandQueueConfig {
    /// Delay between command executions (milliseconds)
    pub stagger_delay_ms: u64,

    /// Maximum number of retries
    #[allow(dead_code)] // Used in execute_with_retry (not yet integrated)
    pub max_retries: u32,

    /// Delay between retries (milliseconds)
    #[allow(dead_code)] // Used in execute_with_retry (not yet integrated)
    pub retry_delay_ms: u64,

    /// Error patterns that trigger retry (regex)
    #[allow(dead_code)] // Used in execute_with_retry (not yet integrated)
    pub retry_error_patterns: Vec<String>,

    /// Retry if execution duration is under this threshold (seconds)
    #[allow(dead_code)] // Used in execute_with_retry (not yet integrated)
    pub retry_if_duration_under_secs: u64,
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
    #[allow(dead_code)] // Not yet integrated, reserved for future retry logic
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
    /// - Error pattern match OR short execution duration
    #[allow(dead_code)] // Not yet integrated, reserved for future retry logic
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

        // Retry if either condition is true (OR logic)
        matches_pattern || is_short_execution
    }

    /// Execute a command with automatic retry on transient errors
    ///
    /// Retries command execution based on:
    /// - Error pattern matching (e.g., "Cannot find module")
    /// - Execution duration (short runs may indicate environment issues)
    #[allow(dead_code)] // Not yet integrated, reserved for future retry logic
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
    fn test_no_retry_on_long_duration_without_pattern() {
        let queue = CommandQueue::new(test_config());
        let duration = Duration::from_secs(10); // Long duration
        let stderr = "Test assertion failed";
        let exit_code = 1;

        assert!(!queue.should_retry(1, duration, stderr, exit_code));
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
}
