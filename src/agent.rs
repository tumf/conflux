//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::{ApplyAttempt, ApplyHistory};
use std::process::{ExitStatus, Stdio};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
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
    pub async fn run_apply_streaming(
        &self,
        change_id: &str,
    ) -> Result<(Child, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_apply_command();
        let base_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);
        let full_prompt = if history_context.is_empty() {
            base_prompt.to_string()
        } else {
            format!("{}\n\n{}", base_prompt, history_context)
        };
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
    ) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_archive_command();
        let prompt = self.config.get_archive_prompt();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, prompt);
        info!("Running archive command: {}", command);
        self.execute_shell_command_streaming(&command).await
    }

    /// Run apply command for the given change ID (blocking, no streaming)
    /// Records the attempt result in history for subsequent retries.
    pub async fn run_apply(&mut self, change_id: &str) -> Result<ExitStatus> {
        let start = Instant::now();

        let template = self.config.get_apply_command();
        let base_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);
        let full_prompt = if history_context.is_empty() {
            base_prompt.to_string()
        } else {
            format!("{}\n\n{}", base_prompt, history_context)
        };
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

    /// Analyze dependencies using the configured analyze command
    pub async fn analyze_dependencies(&self, prompt: &str) -> Result<String> {
        let template = self.config.get_analyze_command();
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!("Running analyze command");
        debug!("Analyze command: {}", command);

        let output = self.execute_shell_command_with_output(&command).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)?;
        debug!("Analysis result: {}", stdout);
        Ok(stdout)
    }

    /// Execute a shell command with output streaming
    /// Returns a child process handle and a receiver for output lines
    async fn execute_shell_command_streaming(
        &self,
        command: &str,
    ) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
        let mut child = if cfg!(target_os = "windows") {
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
                // Detach from controlling terminal completely
                unsafe {
                    #[allow(unused_imports)]
                    use std::os::unix::process::CommandExt;
                    cmd.pre_exec(|| {
                        use std::os::unix::io::RawFd;

                        // Create a new session - this detaches from the controlling terminal
                        if libc::setsid() == -1 {
                            return Err(std::io::Error::last_os_error());
                        }

                        // Close /dev/tty to prevent any direct terminal access
                        // Open /dev/null and redirect any attempts to access /dev/tty
                        let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
                        if devnull != -1 {
                            // Try to open /dev/tty and close it if successful
                            let tty_fd: RawFd = libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR);
                            if tty_fd != -1 {
                                libc::close(tty_fd);
                            }
                            libc::close(devnull);
                        }

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

        Ok((child, rx))
    }

    /// Execute a shell command and wait for completion (blocking, no streaming)
    async fn execute_shell_command(&self, command: &str) -> Result<ExitStatus> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            Command::new(&shell)
                .arg("-i")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
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
            Command::new("cmd")
                .arg("/C")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to execute command: {}", e))
                })?
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            Command::new(&shell)
                .arg("-i")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
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
        let config = OrchestratorConfig {
            apply_command: Some("echo 'Apply {change_id} with {prompt}'".to_string()),
            apply_prompt: Some("custom instructions".to_string()),
            ..Default::default()
        };
        let mut runner = AgentRunner::new(config);
        let status = runner.run_apply("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_archive_with_prompt_expansion() {
        // Test archive command with both {change_id} and {prompt} placeholders
        let config = OrchestratorConfig {
            archive_command: Some("echo 'Archive {change_id} with {prompt}'".to_string()),
            archive_prompt: Some("archive instructions".to_string()),
            ..Default::default()
        };
        let runner = AgentRunner::new(config);
        let status = runner.run_archive("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_apply_with_default_prompt() {
        // Test that apply uses default prompt when not specified
        let config = OrchestratorConfig {
            apply_command: Some("echo 'Apply {change_id} {prompt}'".to_string()),
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
        let config = OrchestratorConfig {
            apply_command: Some("echo '{change_id}:{prompt}'".to_string()),
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
        // Verify the output contains expanded placeholders
        let output: String = lines
            .iter()
            .map(|l| match l {
                OutputLine::Stdout(s) => s.clone(),
                OutputLine::Stderr(s) => s.clone(),
            })
            .collect();
        assert!(output.contains("my-change"));
        assert!(output.contains("streaming test"));
    }
}
