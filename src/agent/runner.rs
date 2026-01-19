//! Agent runner implementation for executing commands.

use super::history_ops;
use super::output::OutputLine;
use super::prompt::{build_acceptance_prompt, build_apply_prompt, build_archive_prompt};
use crate::command_queue::{CommandQueue, CommandQueueConfig, StreamingOutputLine};
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::{AcceptanceAttempt, AcceptanceHistory, ApplyHistory, ArchiveHistory};
use crate::process_manager::ManagedChild;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Manages agent process execution based on configuration
pub struct AgentRunner {
    config: OrchestratorConfig,
    /// Command execution queue with staggered start and retry mechanism
    command_queue: CommandQueue,
    /// History of apply attempts per change for context injection
    apply_history: ApplyHistory,
    /// History of archive attempts per change for context injection
    archive_history: ArchiveHistory,
    /// History of acceptance attempts per change for context injection
    acceptance_history: AcceptanceHistory,
}

impl AgentRunner {
    /// Create a new AgentRunner with the given configuration
    pub fn new(config: OrchestratorConfig) -> Self {
        // Build command queue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };

        Self {
            config,
            command_queue: CommandQueue::new(queue_config),
            apply_history: ApplyHistory::new(),
            archive_history: ArchiveHistory::new(),
            acceptance_history: AcceptanceHistory::new(),
        }
    }

    /// Create a new AgentRunner with shared stagger state.
    ///
    /// This allows multiple AgentRunner instances to coordinate their
    /// stagger delays through a shared last_execution timestamp.
    /// Useful for parallel execution modes where multiple runners
    /// need to avoid simultaneous command starts.
    ///
    /// # Arguments
    ///
    /// * `config` - Orchestrator configuration
    /// * `shared_state` - Shared last execution timestamp (Arc<Mutex<Option<Instant>>>)
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 4.1-4.3)
    pub fn new_with_shared_state(
        config: OrchestratorConfig,
        shared_state: Arc<Mutex<Option<Instant>>>,
    ) -> Self {
        // Build command queue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };

        Self {
            config,
            command_queue: CommandQueue::new_with_shared_state(queue_config, shared_state),
            apply_history: ApplyHistory::new(),
            archive_history: ArchiveHistory::new(),
            acceptance_history: AcceptanceHistory::new(),
        }
    }

    /// Run apply command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_apply_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    pub async fn run_apply_streaming(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_apply_command();
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(&command, dir)
                    .await?
            }
            None => self.execute_shell_command_streaming(&command).await?,
        };
        Ok((child, rx, start))
    }

    /// Record an apply attempt after streaming execution completes.
    /// Call this after `run_apply_streaming()` child process finishes.
    pub fn record_apply_attempt(
        &mut self,
        change_id: &str,
        status: &ExitStatus,
        start: Instant,
        stdout_tail: Option<String>,
        stderr_tail: Option<String>,
    ) {
        history_ops::record_apply_attempt(
            &mut self.apply_history,
            change_id,
            status,
            start,
            stdout_tail,
            stderr_tail,
        );
    }

    /// Run archive command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_archive_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + history_context
    /// - user_prompt: from config.archive_prompt (user-customizable)
    /// - history_context: previous archive attempts (if any)
    pub async fn run_archive_streaming(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_archive_command();
        let user_prompt = self.config.get_archive_prompt();
        let history_context = self.archive_history.format_context(change_id);

        // Build full prompt: user_prompt + history_context
        let full_prompt = build_archive_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running archive command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(&command, dir)
                    .await?
            }
            None => self.execute_shell_command_streaming(&command).await?,
        };
        Ok((child, rx, start))
    }

    /// Run apply command for the given change ID (blocking, no streaming)
    /// Records the attempt result in history for subsequent retries.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    pub async fn run_apply(&mut self, change_id: &str) -> Result<ExitStatus> {
        let start = Instant::now();

        let template = self.config.get_apply_command();
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command: {}", command
        );

        let status = self.execute_shell_command(&command).await?;

        // Record the attempt
        history_ops::record_apply_attempt(
            &mut self.apply_history,
            change_id,
            &status,
            start,
            None,
            None,
        );

        Ok(status)
    }

    /// Record an archive attempt after streaming execution completes.
    /// Call this after `run_archive_streaming()` child process finishes.
    pub fn record_archive_attempt(
        &mut self,
        change_id: &str,
        status: &ExitStatus,
        start: Instant,
        verification_result: Option<String>,
        stdout_tail: Option<String>,
        stderr_tail: Option<String>,
    ) {
        history_ops::record_archive_attempt(
            &mut self.archive_history,
            change_id,
            status,
            start,
            verification_result,
            stdout_tail,
            stderr_tail,
        );
    }

    /// Clear apply history for a change (call after archiving)
    pub fn clear_apply_history(&mut self, change_id: &str) {
        history_ops::clear_apply_history(&mut self.apply_history, change_id);
    }

    /// Clear archive history for a change (call after successful archiving)
    pub fn clear_archive_history(&mut self, change_id: &str) {
        history_ops::clear_archive_history(&mut self.archive_history, change_id);
    }

    /// Clear acceptance history for a change (call after successful archiving)
    #[allow(dead_code)]
    pub fn clear_acceptance_history(&mut self, change_id: &str) {
        history_ops::clear_acceptance_history(&mut self.acceptance_history, change_id);
    }

    /// Run acceptance command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_acceptance_attempt()`.
    ///
    /// The prompt is constructed as: system_prompt + user_prompt + history_context
    /// - system_prompt: ACCEPTANCE_SYSTEM_PROMPT constant (always included)
    /// - user_prompt: from config.acceptance_prompt (user-customizable)
    /// - history_context: previous acceptance attempts (if any)
    pub async fn run_acceptance_streaming(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_acceptance_command();
        let user_prompt = self.config.get_acceptance_prompt();
        let history_context = self.acceptance_history.format_context(change_id);

        // Build full prompt: system_prompt + user_prompt + history_context
        let full_prompt = build_acceptance_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running acceptance command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(&command, dir)
                    .await?
            }
            None => self.execute_shell_command_streaming(&command).await?,
        };
        Ok((child, rx, start))
    }

    /// Record an acceptance attempt after streaming execution completes.
    /// Call this after `run_acceptance_streaming()` child process finishes.
    pub fn record_acceptance_attempt(&mut self, change_id: &str, attempt: AcceptanceAttempt) {
        history_ops::record_acceptance_attempt(&mut self.acceptance_history, change_id, attempt);
    }

    /// Get the next acceptance attempt number for a change.
    pub fn next_acceptance_attempt_number(&self, change_id: &str) -> u32 {
        history_ops::next_acceptance_attempt_number(&self.acceptance_history, change_id)
    }

    /// Get the count of consecutive CONTINUE attempts for a change.
    pub fn count_consecutive_acceptance_continues(&self, change_id: &str) -> u32 {
        history_ops::count_consecutive_acceptance_continues(&self.acceptance_history, change_id)
    }

    /// Run archive command for the given change ID (blocking, no streaming)
    pub async fn run_archive(&self, change_id: &str) -> Result<ExitStatus> {
        let template = self.config.get_archive_command();
        let prompt = self.config.get_archive_prompt();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, prompt);
        info!(
            module = module_path!(),
            "Running archive command: {}", command
        );
        self.execute_shell_command(&command).await
    }

    /// Analyze dependencies using the configured analyze command (blocking)
    pub async fn analyze_dependencies(&self, prompt: &str) -> Result<String> {
        let template = self.config.get_analyze_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running analyze command: {}", template
        );
        info!("Expanded command length: {} chars", command.len());

        let output = self.execute_shell_command_with_output(&command).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)?;
        info!(
            "Analyze command completed, output length: {} chars",
            stdout.len()
        );
        debug!("Raw analysis output: {}", stdout);

        // Extract result from stream-json format if applicable
        let result = self.extract_stream_json_result(&stdout);
        info!("Extracted result length: {} chars", result.len());
        debug!("Extracted analysis result: {}", result);
        Ok(result)
    }

    /// Analyze dependencies using the configured analyze command with streaming output
    /// Returns a child process handle and a receiver for output lines
    pub async fn analyze_dependencies_streaming(
        &self,
        prompt: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_analyze_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running analyze command (streaming): {}", template
        );
        self.execute_shell_command_streaming(&command).await
    }

    /// Execute resolve command with streaming output in a specific directory.
    /// Returns a child process handle and a receiver for output lines.
    pub async fn run_resolve_streaming_in_dir(
        &self,
        prompt: &str,
        cwd: &Path,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_resolve_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running resolve command (streaming) in {:?}: {}", cwd, command
        );
        self.execute_shell_command_streaming_in_dir(&command, cwd)
            .await
    }

    /// Extract the result from stream-json output format
    /// stream-json outputs multiple JSON lines, the last one with type="result" contains the actual result
    fn extract_stream_json_result(&self, output: &str) -> String {
        // Try to find and parse the result line from stream-json output
        for line in output.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                // Check if this is a result message
                if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                    if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                        return result.to_string();
                    }
                }
                // Also check for assistant message content
                if json.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                    if let Some(message) = json.get("message") {
                        if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                            for item in content {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        // If not stream-json format, return as-is
        output.to_string()
    }

    /// Execute a shell command with output streaming and automatic retry
    /// Returns a child process handle and a receiver for output lines
    ///
    /// This function uses the command queue's retry logic to automatically retry
    /// transient failures. Retry notifications are sent through the output channel.
    async fn execute_shell_command_streaming(
        &self,
        command: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        // Create output channel
        let (tx, rx) = mpsc::channel::<OutputLine>(100);

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

        // Clone command queue and command string for background task
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();

        // Create oneshot channel to communicate final status
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<ExitStatus>();

        // Spawn background task to run retry logic
        tokio::spawn(async move {
            let result = command_queue
                .execute_with_retry_streaming(|| build_command(&command_str), Some(output_callback))
                .await;

            // Send final status
            match result {
                Ok((status, _stderr)) => {
                    let _ = status_tx.send(status);
                }
                Err(_) => {
                    // Send failure status
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        let _ = status_tx.send(ExitStatus::from_raw(1));
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, we can't create ExitStatus easily
                        // Just drop the channel
                    }
                }
            }

            // Close output channel
            drop(tx);
        });

        // Create a dummy child process that waits for stdin to close
        // When the background task completes, we'll close stdin which causes the process to exit
        let mut dummy_child = create_dummy_child()?;

        // Take stdin so we can close it later
        let dummy_stdin = dummy_child.stdin.take();

        // Wrap dummy child in ManagedChild
        let managed_child = ManagedChild::new(dummy_child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        // Spawn a task to close stdin when real command completes
        // This will cause the dummy process to exit cleanly
        tokio::spawn(async move {
            // Wait for status from background task
            let _ = status_rx.await;
            // Close stdin - this causes cat/findstr to exit cleanly
            drop(dummy_stdin);
        });

        Ok((managed_child, rx))
    }

    /// Execute a shell command with output streaming and automatic retry in a specific directory
    /// Returns a child process handle and a receiver for output lines
    ///
    /// This function uses the command queue's retry logic to automatically retry
    /// transient failures. Retry notifications are sent through the output channel.
    async fn execute_shell_command_streaming_in_dir(
        &self,
        command: &str,
        cwd: &Path,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        // Create output channel
        let (tx, rx) = mpsc::channel::<OutputLine>(100);

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
        let cwd_path = cwd.to_path_buf();

        // Create oneshot channel to communicate final status
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<ExitStatus>();

        // Spawn background task to run retry logic
        tokio::spawn(async move {
            let result = command_queue
                .execute_with_retry_streaming(
                    || build_command_in_dir(&command_str, &cwd_path),
                    Some(output_callback),
                )
                .await;

            // Send final status
            match result {
                Ok((status, _stderr)) => {
                    let _ = status_tx.send(status);
                }
                Err(_) => {
                    // Send failure status
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        let _ = status_tx.send(ExitStatus::from_raw(1));
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, we can't create ExitStatus easily
                        // Just drop the channel
                    }
                }
            }

            // Close output channel
            drop(tx);
        });

        // Create a dummy child process that waits for stdin to close
        // When the background task completes, we'll close stdin which causes the process to exit
        let mut dummy_child = create_dummy_child()?;

        // Take stdin so we can close it later
        let dummy_stdin = dummy_child.stdin.take();

        // Wrap dummy child in ManagedChild
        let managed_child = ManagedChild::new(dummy_child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        // Spawn a task to close stdin when real command completes
        // This will cause the dummy process to exit cleanly
        tokio::spawn(async move {
            // Wait for status from background task
            let _ = status_rx.await;
            // Close stdin - this causes cat/findstr to exit cleanly
            drop(dummy_stdin);
        });

        Ok((managed_child, rx))
    }

    /// Execute a shell command and wait for completion (blocking, no streaming)
    async fn execute_shell_command(&self, command: &str) -> Result<ExitStatus> {
        let output = if cfg!(target_os = "windows") {
            debug!(
                module = module_path!(),
                "Executing shell command: cmd /C {}", command
            );
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            debug!(
                module = module_path!(),
                "Executing shell command: {} -l -c {}", shell, command
            );
            Command::new(&shell)
                .arg("-l")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        };

        debug!("Command exited with status: {:?}", output.status);
        Ok(output.status)
    }

    /// Execute a shell command and capture its output
    async fn execute_shell_command_with_output(
        &self,
        command: &str,
    ) -> Result<std::process::Output> {
        let output = if cfg!(target_os = "windows") {
            debug!(
                module = module_path!(),
                "Executing shell command: cmd /C {}", command
            );
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to execute command: {}", e))
                })?
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            debug!(
                module = module_path!(),
                "Executing shell command: {} -l -c {}", shell, command
            );
            Command::new(&shell)
                .arg("-l")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdin(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to execute command: {}", e))
                })?
        };

        Ok(output)
    }

    /// Get the underlying configuration (for testing)
    #[cfg(test)]
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }
}

/// Build a command for execution (extracted for use with command queue)
fn build_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        debug!("Building shell command: cmd /C {}", command);
        let mut cmd = Command::new("cmd");
        cmd.arg("/C")
            .arg(command)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            .env("CI", "true")
            // Disable pagers
            .env("PAGER", "type")
            .env("GIT_PAGER", "type")
            .env("LESS", "")
            .env("MORE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    } else {
        // Use login shell to load .zprofile/.profile for PATH and environment setup
        // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        debug!("Building shell command: {} -l -c {}", shell, command);
        let mut cmd = Command::new(&shell);
        cmd.arg("-l")
            .arg("-c")
            .arg(command)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            // Disable interactive features
            .env("CI", "true")
            .env("CONTINUOUS_INTEGRATION", "true")
            .env("NON_INTERACTIVE", "1")
            // Disable pagers completely
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("LESS", "-FX") // -F: quit if one screen, -X: no init
            .env("MORE", "-E") // -E: quit at EOF
            // Prevent any pager from being used
            .env("MANPAGER", "cat")
            .env("SYSTEMD_PAGER", "cat")
            // Disable git interactive features
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            // Detach from controlling terminal and create new process group
            unsafe {
                #[allow(unused_imports)]
                use std::os::unix::process::CommandExt;
                cmd.pre_exec(|| {
                    use nix::unistd::{setpgid, Pid};

                    // Create a new process group (replacing setsid for better cleanup)
                    setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(std::io::Error::other)?;

                    Ok(())
                });
            }
        }

        cmd
    }
}

/// Build a command for execution in a specific directory
fn build_command_in_dir(command: &str, cwd: &Path) -> Command {
    if cfg!(target_os = "windows") {
        debug!("Building shell command: cmd /C {}", command);
        let mut cmd = Command::new("cmd");
        cmd.arg("/C")
            .arg(command)
            .current_dir(cwd)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            .env("CI", "true")
            // Disable pagers
            .env("PAGER", "type")
            .env("GIT_PAGER", "type")
            .env("LESS", "")
            .env("MORE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    } else {
        // Use login shell to load .zprofile/.profile for PATH and environment setup
        // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        debug!("Building shell command: {} -l -c {}", shell, command);
        let mut cmd = Command::new(&shell);
        cmd.arg("-l")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            // Disable interactive features
            .env("CI", "true")
            .env("CONTINUOUS_INTEGRATION", "true")
            .env("NON_INTERACTIVE", "1")
            // Disable pagers completely
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("LESS", "-FX") // -F: quit if one screen, -X: no init
            .env("MORE", "-E") // -E: quit at EOF
            // Prevent any pager from being used
            .env("MANPAGER", "cat")
            .env("SYSTEMD_PAGER", "cat")
            // Disable git interactive features
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            // Detach from controlling terminal and create new process group
            unsafe {
                #[allow(unused_imports)]
                use std::os::unix::process::CommandExt;
                cmd.pre_exec(|| {
                    use nix::unistd::{setpgid, Pid};

                    // Create a new process group (replacing setsid for better cleanup)
                    setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(std::io::Error::other)?;

                    Ok(())
                });
            }
        }

        cmd
    }
}

/// Create a dummy child process that waits for stdin to close
fn create_dummy_child() -> Result<tokio::process::Child> {
    let child = if cfg!(target_os = "windows") {
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
    Ok(child)
}
