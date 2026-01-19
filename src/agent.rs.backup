//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

use crate::command_queue::{CommandQueue, CommandQueueConfig};
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Legacy hardcoded system prompt for apply commands.
/// Kept only for compatibility in tests; actual prompt is sourced from OpenCode command files.
pub const APPLY_SYSTEM_PROMPT: &str = "";
use crate::config::defaults::ACCEPTANCE_SYSTEM_PROMPT;
use crate::error::{OrchestratorError, Result};
use crate::history::{
    AcceptanceAttempt, AcceptanceHistory, ApplyAttempt, ApplyHistory, ArchiveAttempt,
    ArchiveHistory,
};
use crate::process_manager::ManagedChild;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, info};

/// Output line from a child process
#[derive(Debug, Clone)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

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
        let duration = start.elapsed();
        let attempt = ApplyAttempt {
            attempt: self.apply_history.count(change_id) + 1,
            success: status.success(),
            duration,
            error: if status.success() {
                None
            } else {
                Some(format!("Exit code: {:?}", status.code()))
            },
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
        };
        self.apply_history.record(change_id, attempt);
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
        let duration = start.elapsed();

        // Record the attempt (no output captured in non-streaming mode)
        let attempt = ApplyAttempt {
            attempt: self.apply_history.count(change_id) + 1,
            success: status.success(),
            duration,
            error: if status.success() {
                None
            } else {
                Some(format!("Exit code: {:?}", status.code()))
            },
            exit_code: status.code(),
            stdout_tail: None,
            stderr_tail: None,
        };
        self.apply_history.record(change_id, attempt);

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
        let duration = start.elapsed();
        let attempt = ArchiveAttempt {
            attempt: self.archive_history.count(change_id) + 1,
            success: status.success(),
            duration,
            error: if status.success() && verification_result.is_none() {
                None
            } else if verification_result.is_some() {
                Some(format!(
                    "Archive command succeeded but verification failed: {}",
                    verification_result.as_ref().unwrap()
                ))
            } else {
                Some(format!("Exit code: {:?}", status.code()))
            },
            verification_result,
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
        };
        self.archive_history.record(change_id, attempt);
    }

    /// Clear apply history for a change (call after archiving)
    pub fn clear_apply_history(&mut self, change_id: &str) {
        self.apply_history.clear(change_id);
    }

    /// Clear archive history for a change (call after successful archiving)
    pub fn clear_archive_history(&mut self, change_id: &str) {
        self.archive_history.clear(change_id);
    }

    /// Clear acceptance history for a change (call after successful archiving)
    #[allow(dead_code)]
    pub fn clear_acceptance_history(&mut self, change_id: &str) {
        self.acceptance_history.clear(change_id);
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
        self.acceptance_history.record(change_id, attempt);
    }

    /// Get the next acceptance attempt number for a change.
    pub fn next_acceptance_attempt_number(&self, change_id: &str) -> u32 {
        self.acceptance_history.count(change_id) + 1
    }

    /// Get the count of consecutive CONTINUE attempts for a change.
    pub fn count_consecutive_acceptance_continues(&self, change_id: &str) -> u32 {
        self.acceptance_history
            .count_consecutive_continues(change_id)
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
        use crate::command_queue::StreamingOutputLine;

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
                .execute_with_retry_streaming(
                    || {
                        // Build command for each retry attempt
                        if cfg!(target_os = "windows") {
                            let mut cmd = Command::new("cmd");
                            cmd.arg("/C")
                                .arg(&command_str)
                                .env_clear()
                                .envs(std::env::vars())
                                .env("NO_COLOR", "1")
                                .env("CLICOLOR", "0")
                                .env("CLICOLOR_FORCE", "0")
                                .env("CI", "true")
                                .env("PAGER", "type")
                                .env("GIT_PAGER", "type")
                                .env("LESS", "")
                                .env("MORE", "")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());
                            cmd
                        } else {
                            let shell =
                                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                            let mut cmd = Command::new(&shell);
                            cmd.arg("-l")
                                .arg("-c")
                                .arg(&command_str)
                                .env_clear()
                                .envs(std::env::vars())
                                .env("TERM", "dumb")
                                .env("NO_COLOR", "1")
                                .env("CLICOLOR", "0")
                                .env("CLICOLOR_FORCE", "0")
                                .env("CI", "true")
                                .env("CONTINUOUS_INTEGRATION", "true")
                                .env("NON_INTERACTIVE", "1")
                                .env("PAGER", "cat")
                                .env("GIT_PAGER", "cat")
                                .env("LESS", "-FX")
                                .env("MORE", "-E")
                                .env("MANPAGER", "cat")
                                .env("SYSTEMD_PAGER", "cat")
                                .env("GIT_TERMINAL_PROMPT", "0")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());

                            #[cfg(unix)]
                            unsafe {
                                #[allow(unused_imports)]
                                use std::os::unix::process::CommandExt;
                                cmd.pre_exec(|| {
                                    use nix::unistd::{setpgid, Pid};
                                    setpgid(Pid::from_raw(0), Pid::from_raw(0))
                                        .map_err(std::io::Error::other)?;
                                    Ok(())
                                });
                            }

                            cmd
                        }
                    },
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

    /// Build a command for execution (extracted for use with command queue)
    #[allow(dead_code)]
    fn build_command(&self, command: &str) -> Command {
        if cfg!(target_os = "windows") {
            debug!(
                module = module_path!(),
                "Building shell command: cmd /C {}", command
            );
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
            debug!(
                module = module_path!(),
                "Building shell command: {} -l -c {}", shell, command
            );
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
                        setpgid(Pid::from_raw(0), Pid::from_raw(0))
                            .map_err(std::io::Error::other)?;

                        Ok(())
                    });
                }
            }

            cmd
        }
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
        use crate::command_queue::StreamingOutputLine;

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
                    || {
                        // Build command for each retry attempt
                        if cfg!(target_os = "windows") {
                            let mut cmd = Command::new("cmd");
                            cmd.arg("/C")
                                .arg(&command_str)
                                .current_dir(&cwd_path)
                                .env_clear()
                                .envs(std::env::vars())
                                .env("NO_COLOR", "1")
                                .env("CLICOLOR", "0")
                                .env("CLICOLOR_FORCE", "0")
                                .env("CI", "true")
                                .env("PAGER", "type")
                                .env("GIT_PAGER", "type")
                                .env("LESS", "")
                                .env("MORE", "")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());
                            cmd
                        } else {
                            let shell =
                                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                            let mut cmd = Command::new(&shell);
                            cmd.arg("-l")
                                .arg("-c")
                                .arg(&command_str)
                                .current_dir(&cwd_path)
                                .env_clear()
                                .envs(std::env::vars())
                                .env("TERM", "dumb")
                                .env("NO_COLOR", "1")
                                .env("CLICOLOR", "0")
                                .env("CLICOLOR_FORCE", "0")
                                .env("CI", "true")
                                .env("CONTINUOUS_INTEGRATION", "true")
                                .env("NON_INTERACTIVE", "1")
                                .env("PAGER", "cat")
                                .env("GIT_PAGER", "cat")
                                .env("LESS", "-FX")
                                .env("MORE", "-E")
                                .env("MANPAGER", "cat")
                                .env("SYSTEMD_PAGER", "cat")
                                .env("GIT_TERMINAL_PROMPT", "0")
                                .stdin(Stdio::null())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());

                            #[cfg(unix)]
                            unsafe {
                                #[allow(unused_imports)]
                                use std::os::unix::process::CommandExt;
                                cmd.pre_exec(|| {
                                    use nix::unistd::{setpgid, Pid};
                                    setpgid(Pid::from_raw(0), Pid::from_raw(0))
                                        .map_err(std::io::Error::other)?;
                                    Ok(())
                                });
                            }

                            cmd
                        }
                    },
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

    /// Build a command for execution in a specific directory
    #[allow(dead_code)]
    fn build_command_in_dir(&self, command: &str, cwd: &Path) -> Command {
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
                        setpgid(Pid::from_raw(0), Pid::from_raw(0))
                            .map_err(std::io::Error::other)?;

                        Ok(())
                    });
                }
            }

            cmd
        }
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

/// Build the full apply prompt by combining user prompt, system prompt, and history context.
///
/// The prompt is constructed as:
/// 1. user_prompt (if not empty)
/// 2. APPLY_SYSTEM_PROMPT (always included)
/// 3. history_context (if not empty)
///
/// Parts are joined with double newlines.
pub fn build_apply_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    // System prompt is always included
    parts.push(APPLY_SYSTEM_PROMPT.to_string());

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build archive prompt from user prompt and history context
/// Format: user_prompt + history_context
pub fn build_archive_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build acceptance prompt from user prompt and history context
///
/// The prompt is constructed as:
/// 1. ACCEPTANCE_SYSTEM_PROMPT (always included)
/// 2. user_prompt (if not empty)
/// 3. history_context (if not empty)
///
/// Parts are joined with double newlines.
pub fn build_acceptance_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    // System prompt is always included first
    parts.push(ACCEPTANCE_SYSTEM_PROMPT.to_string());

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_runner_creation() {
        let config = OrchestratorConfig::default();
        let runner = AgentRunner::new(config);
        assert_eq!(
            runner.config().get_apply_command(),
            crate::config::DEFAULT_APPLY_COMMAND
        );
    }

    #[test]
    fn test_agent_runner_with_custom_config() {
        let config = OrchestratorConfig {
            apply_command: Some("custom-agent apply {change_id}".to_string()),
            archive_command: Some("custom-agent archive {change_id}".to_string()),
            analyze_command: Some("custom-agent analyze '{prompt}'".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        assert_eq!(
            runner.config().get_apply_command(),
            "custom-agent apply {change_id}"
        );
        assert_eq!(
            runner.config().get_archive_command(),
            "custom-agent archive {change_id}"
        );
        assert_eq!(
            runner.config().get_analyze_command(),
            "custom-agent analyze '{prompt}'"
        );
    }

    #[tokio::test]
    async fn test_run_apply_echo_command() {
        // Test with a simple echo command
        let config = OrchestratorConfig {
            apply_command: Some("echo 'Applying {change_id}'".to_string()),
            ..Default::default()
        };
        let mut runner = AgentRunner::new(config);
        let status = runner.run_apply("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_archive_echo_command() {
        let config = OrchestratorConfig {
            archive_command: Some("echo 'Archiving {change_id}'".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        let status = runner.run_archive("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_analyze_dependencies_echo_command() {
        let config = OrchestratorConfig {
            analyze_command: Some("echo '{prompt}'".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        let result = runner.analyze_dependencies("test prompt").await.unwrap();
        assert!(result.contains("test prompt"));
    }

    #[tokio::test]
    async fn test_run_apply_streaming() {
        let config = OrchestratorConfig {
            apply_command: Some("echo 'line1' && echo 'line2'".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        let (mut child, mut rx, _start) = runner
            .run_apply_streaming("test-change", None)
            .await
            .unwrap();

        let mut lines = Vec::new();
        while let Some(line) = rx.recv().await {
            lines.push(line);
        }

        let status = child.wait().await.unwrap();
        assert!(status.success());
        assert!(!lines.is_empty());
    }

    #[tokio::test]
    async fn test_run_apply_with_prompt_expansion() {
        // Test apply command with both {change_id} and {prompt} placeholders
        // Just verify that command succeeds (prompt is expanded internally)
        let config = OrchestratorConfig {
            apply_command: Some("true".to_string()),
            apply_prompt: Some("custom instructions".to_string()),
            ..Default::default()
        };
        let mut runner = AgentRunner::new(config);
        let status = runner.run_apply("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_apply_with_default_prompt() {
        // Test that apply uses default prompt when not specified
        let config = OrchestratorConfig {
            apply_command: Some("true".to_string()),
            ..Default::default()
        };
        let mut runner = AgentRunner::new(config);
        // Default apply_prompt should be used
        assert_eq!(
            runner.config().get_apply_prompt(),
            crate::config::DEFAULT_APPLY_PROMPT
        );
        let status = runner.run_apply("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_archive_with_empty_default_prompt() {
        // Test that archive uses default prompt (which is empty)
        let config = OrchestratorConfig {
            archive_command: Some("echo 'Archive {change_id}:{prompt}:end'".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        // Default archive_prompt should be empty
        assert_eq!(
            runner.config().get_archive_prompt(),
            crate::config::DEFAULT_ARCHIVE_PROMPT
        );
        // Verify default is empty
        assert_eq!(runner.config().get_archive_prompt(), "");
        let status = runner.run_archive("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_apply_streaming_with_prompt() {
        // Test streaming with simple echo that outputs placeholders
        let config = OrchestratorConfig {
            apply_command: Some("echo '{change_id}' && echo 'prompt-marker'".to_string()),
            apply_prompt: Some("streaming test".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        let (mut child, mut rx, _start) =
            runner.run_apply_streaming("my-change", None).await.unwrap();

        let mut lines = Vec::new();
        while let Some(line) = rx.recv().await {
            lines.push(line);
        }

        let status = child.wait().await.unwrap();
        assert!(status.success());
        // Verify the output contains expanded change_id
        let output: String = lines
            .iter()
            .map(|l| match l {
                OutputLine::Stdout(s) => s.clone(),
                OutputLine::Stderr(s) => s.clone(),
            })
            .collect();
        assert!(output.contains("my-change"));
        assert!(output.contains("prompt-marker"));
    }

    // Tests for build_apply_prompt function and prompt construction order

    #[test]
    fn test_build_apply_prompt_with_all_parts() {
        let user_prompt = "Focus on implementation.";
        let history_context = "Previous attempt failed.";
        let result = build_apply_prompt(user_prompt, history_context);

        assert!(result.contains("Focus on implementation."));
        assert!(result.contains("Previous attempt failed."));
    }

    #[test]
    fn test_build_apply_prompt_with_empty_user_prompt() {
        let user_prompt = "";
        let history_context = "Previous attempt failed.";
        let result = build_apply_prompt(user_prompt, history_context);

        assert!(result.contains("Previous attempt failed."));
    }

    #[test]
    fn test_build_apply_prompt_with_empty_history() {
        let user_prompt = "Focus on implementation.";
        let history_context = "";
        let result = build_apply_prompt(user_prompt, history_context);

        assert!(result.contains("Focus on implementation."));
    }

    #[test]
    fn test_build_apply_prompt_with_only_system_prompt() {
        let user_prompt = "";
        let history_context = "";
        let result = build_apply_prompt(user_prompt, history_context);

        assert_eq!(result, APPLY_SYSTEM_PROMPT);
    }

    #[test]
    fn test_apply_system_prompt_content() {
        assert_eq!(APPLY_SYSTEM_PROMPT, "");
    }

    #[test]
    fn test_build_archive_prompt_with_all_parts() {
        let user_prompt = "Please archive this change";
        let history_context = "<last_archive attempt=\"1\">\nstatus: failed\n</last_archive>";
        let result = build_archive_prompt(user_prompt, history_context);

        assert!(result.contains("Please archive this change"));
        assert!(result.contains("<last_archive attempt=\"1\">"));
        assert!(result.contains("status: failed"));
    }

    #[test]
    fn test_build_archive_prompt_with_empty_user_prompt() {
        let user_prompt = "";
        let history_context = "<last_archive attempt=\"1\">\nstatus: failed\n</last_archive>";
        let result = build_archive_prompt(user_prompt, history_context);

        // Should only contain history
        assert!(result.contains("<last_archive attempt=\"1\">"));
        assert!(!result.contains("\n\n\n")); // No triple newlines
    }

    #[test]
    fn test_build_archive_prompt_with_empty_history() {
        let user_prompt = "Please archive this change";
        let history_context = "";
        let result = build_archive_prompt(user_prompt, history_context);

        // Should only contain user prompt
        assert_eq!(result, "Please archive this change");
    }

    #[test]
    fn test_build_archive_prompt_both_empty() {
        let user_prompt = "";
        let history_context = "";
        let result = build_archive_prompt(user_prompt, history_context);

        // Should be empty
        assert!(result.is_empty());
    }
}
