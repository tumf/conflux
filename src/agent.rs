//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

use crate::config::OrchestratorConfig;

/// Hardcoded system prompt for apply commands.
/// This is always appended after the user-configurable apply_prompt.
/// These instructions are non-negotiable and cannot be disabled.
pub const APPLY_SYSTEM_PROMPT: &str = r#"

<system-reminder>
Your operational mode has changed from plan to build.
You are no longer in read-only mode.
You are permitted to make file changes, run shell commands, and utilize your arsenal of tools as needed.
</system-reminder>

Remove tasks only if they meet one of these criteria:
- Out-of-scope: belongs to a different change/proposal
- Requires human decision or external action (e.g., 'Ask user...', 'Deploy to production', 'Get API key')
- Requires long waiting periods (e.g., 'Check after one week', 'Wait for approval')
- Explicitly marked as 'future work' (deferred for later implementation)

Do NOT remove:
- Tests (unit/integration) - agent can write and run them
- Linting/formatting (cargo clippy, cargo fmt) - agent can execute
- Documentation updates - agent can write
- Any task the agent can execute autonomously

Special handling for 'future work' tasks:
- If a task is marked '(future work)', remove it from tasks.md
- This indicates deferred work, not current implementation scope"#;
use crate::error::{OrchestratorError, Result};
use crate::history::{ApplyAttempt, ApplyHistory};
use crate::process_manager::ManagedChild;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
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
    /// History of apply attempts per change for context injection
    apply_history: ApplyHistory,
}

impl AgentRunner {
    /// Create a new AgentRunner with the given configuration
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            config,
            apply_history: ApplyHistory::new(),
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
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_apply_command();
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!("Running apply command: {}", command);
        let (child, rx) = self.execute_shell_command_streaming(&command).await?;
        Ok((child, rx, start))
    }

    /// Record an apply attempt after streaming execution completes.
    /// Call this after `run_apply_streaming()` child process finishes.
    pub fn record_apply_attempt(&mut self, change_id: &str, status: &ExitStatus, start: Instant) {
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
        };
        self.apply_history.record(change_id, attempt);
    }

    /// Run archive command for the given change ID with output streaming
    /// Returns a child process handle and a receiver for output lines
    pub async fn run_archive_streaming(
        &self,
        change_id: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_archive_command();
        let prompt = self.config.get_archive_prompt();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, prompt);
        info!("Running archive command: {}", command);
        self.execute_shell_command_streaming(&command).await
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
        info!("Running apply command: {}", command);

        let status = self.execute_shell_command(&command).await?;
        let duration = start.elapsed();

        // Record the attempt
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
        };
        self.apply_history.record(change_id, attempt);

        Ok(status)
    }

    /// Clear apply history for a change (call after archiving)
    pub fn clear_apply_history(&mut self, change_id: &str) {
        self.apply_history.clear(change_id);
    }

    /// Run archive command for the given change ID (blocking, no streaming)
    pub async fn run_archive(&self, change_id: &str) -> Result<ExitStatus> {
        let template = self.config.get_archive_command();
        let prompt = self.config.get_archive_prompt();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, prompt);
        info!("Running archive command: {}", command);
        self.execute_shell_command(&command).await
    }

    /// Analyze dependencies using the configured analyze command (blocking)
    pub async fn analyze_dependencies(&self, prompt: &str) -> Result<String> {
        let template = self.config.get_analyze_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!("Running analyze command: {}", template);
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
        info!("Running analyze command (streaming): {}", template);
        self.execute_shell_command_streaming(&command).await
    }

    /// Execute resolve command with streaming output
    /// Returns a child process handle and a receiver for output lines
    pub async fn run_resolve_streaming(
        &self,
        prompt: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_resolve_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!("Running resolve command (streaming): {}", command);
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
            "Running resolve command (streaming) in {:?}: {}",
            cwd, command
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

    /// Execute a shell command with output streaming
    /// Returns a child process handle and a receiver for output lines
    async fn execute_shell_command_streaming(
        &self,
        command: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let mut child = if cfg!(target_os = "windows") {
            debug!("Spawning shell command: cmd /C {}", command);
            Command::new("cmd")
                .arg("/C")
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
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        } else {
            // Use login shell to load .zprofile/.profile for PATH and environment setup
            // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            debug!("Spawning shell command: {} -l -c {}", shell, command);
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

            cmd.spawn().map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
            })?
        };

        let (tx, rx) = mpsc::channel::<OutputLine>(100);

        // Take ownership of stdout and stderr
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Spawn task to read stdout
        if let Some(stdout) = stdout {
            let tx_stdout = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stdout.send(OutputLine::Stdout(line)).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Spawn task to read stderr
        if let Some(stderr) = stderr {
            let tx_stderr = tx;
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stderr.send(OutputLine::Stderr(line)).await.is_err() {
                        break;
                    }
                }
            });
        }

        // Wrap child in ManagedChild for reliable cleanup
        let managed_child = ManagedChild::new(child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        Ok((managed_child, rx))
    }

    async fn execute_shell_command_streaming_in_dir(
        &self,
        command: &str,
        cwd: &Path,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let mut child = if cfg!(target_os = "windows") {
            debug!("Spawning shell command: cmd /C {}", command);
            Command::new("cmd")
                .arg("/C")
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
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        } else {
            // Use login shell to load .zprofile/.profile for PATH and environment setup
            // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            debug!("Spawning shell command: {} -l -c {}", shell, command);
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

            cmd.spawn().map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
            })?
        };

        let (tx, rx) = mpsc::channel::<OutputLine>(100);

        // Take ownership of stdout and stderr
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if let Some(stdout) = stdout {
            let tx_stdout = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stdout.send(OutputLine::Stdout(line)).await.is_err() {
                        break;
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let tx_stderr = tx;
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if tx_stderr.send(OutputLine::Stderr(line)).await.is_err() {
                        break;
                    }
                }
            });
        }

        let managed_child = ManagedChild::new(child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        Ok((managed_child, rx))
    }

    /// Execute a shell command and wait for completion (blocking, no streaming)
    async fn execute_shell_command(&self, command: &str) -> Result<ExitStatus> {
        let output = if cfg!(target_os = "windows") {
            debug!("Executing shell command: cmd /C {}", command);
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
            debug!("Executing shell command: {} -l -c {}", shell, command);
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
            debug!("Executing shell command: cmd /C {}", command);
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
            debug!("Executing shell command: {} -l -c {}", shell, command);
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
        let (mut child, mut rx, _start) = runner.run_apply_streaming("test-change").await.unwrap();

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
        // Test that archive uses empty default prompt when not specified
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
        let (mut child, mut rx, _start) = runner.run_apply_streaming("my-change").await.unwrap();

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

        // Should contain all three parts in order
        assert!(result.contains("Focus on implementation."));
        assert!(result.contains(APPLY_SYSTEM_PROMPT));
        assert!(result.contains("Previous attempt failed."));

        // Verify order: user_prompt comes before system_prompt
        let user_pos = result.find("Focus on implementation.").unwrap();
        let system_pos = result.find(APPLY_SYSTEM_PROMPT).unwrap();
        let history_pos = result.find("Previous attempt failed.").unwrap();
        assert!(
            user_pos < system_pos,
            "user_prompt should come before system_prompt"
        );
        assert!(
            system_pos < history_pos,
            "system_prompt should come before history_context"
        );
    }

    #[test]
    fn test_build_apply_prompt_with_empty_user_prompt() {
        let user_prompt = "";
        let history_context = "Previous attempt failed.";
        let result = build_apply_prompt(user_prompt, history_context);

        // Should contain system prompt and history, but no user prompt
        assert!(result.contains(APPLY_SYSTEM_PROMPT));
        assert!(result.contains("Previous attempt failed."));

        // System prompt should come first (no empty string at start)
        assert!(result.starts_with(APPLY_SYSTEM_PROMPT));
    }

    #[test]
    fn test_build_apply_prompt_with_empty_history() {
        let user_prompt = "Focus on implementation.";
        let history_context = "";
        let result = build_apply_prompt(user_prompt, history_context);

        // Should contain user prompt and system prompt
        assert!(result.contains("Focus on implementation."));
        assert!(result.contains(APPLY_SYSTEM_PROMPT));

        // Verify order
        let user_pos = result.find("Focus on implementation.").unwrap();
        let system_pos = result.find(APPLY_SYSTEM_PROMPT).unwrap();
        assert!(user_pos < system_pos);
    }

    #[test]
    fn test_build_apply_prompt_with_only_system_prompt() {
        let user_prompt = "";
        let history_context = "";
        let result = build_apply_prompt(user_prompt, history_context);

        // Should only contain system prompt
        assert_eq!(result, APPLY_SYSTEM_PROMPT);
    }

    #[test]
    fn test_apply_system_prompt_content() {
        // Verify the hardcoded system prompt contains expected instructions
        assert!(APPLY_SYSTEM_PROMPT.contains("Out-of-scope"));
        assert!(APPLY_SYSTEM_PROMPT.contains("human decision"));
        assert!(APPLY_SYSTEM_PROMPT.contains("Do NOT remove"));
    }
}
