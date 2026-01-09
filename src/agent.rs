//! Agent runner module for executing configurable agent commands.
//!
//! This module provides a generic agent runner that executes shell commands
//! based on configuration templates. It replaces the OpenCode-specific runner
//! with a configurable approach.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use std::process::{ExitStatus, Stdio};
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
}

impl AgentRunner {
    /// Create a new AgentRunner with the given configuration
    pub fn new(config: OrchestratorConfig) -> Self {
        Self { config }
    }

    /// Run apply command for the given change ID with output streaming
    /// Returns a child process handle and a receiver for output lines
    pub async fn run_apply_streaming(
        &self,
        change_id: &str,
    ) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_apply_command();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        info!("Running apply command: {}", command);
        self.execute_shell_command_streaming(&command).await
    }

    /// Run archive command for the given change ID with output streaming
    /// Returns a child process handle and a receiver for output lines
    pub async fn run_archive_streaming(
        &self,
        change_id: &str,
    ) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_archive_command();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        info!("Running archive command: {}", command);
        self.execute_shell_command_streaming(&command).await
    }

    /// Run apply command for the given change ID (blocking, no streaming)
    pub async fn run_apply(&self, change_id: &str) -> Result<ExitStatus> {
        let template = self.config.get_apply_command();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        info!("Running apply command: {}", command);
        self.execute_shell_command(&command).await
    }

    /// Run archive command for the given change ID (blocking, no streaming)
    pub async fn run_archive(&self, change_id: &str) -> Result<ExitStatus> {
        let template = self.config.get_archive_command();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
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
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to spawn process: {}", e))
                })?
        } else {
            // Use interactive shell to ensure .zshrc/.bashrc are loaded
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            Command::new(&shell)
                .arg("-i")
                .arg("-c")
                .arg(command)
                .env_clear()
                .envs(std::env::vars())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
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
            hooks: None,
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
            archive_command: None,
            analyze_command: None,
            hooks: None,
        };
        let runner = AgentRunner::new(config);
        let status = runner.run_apply("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_run_archive_echo_command() {
        let config = OrchestratorConfig {
            apply_command: None,
            archive_command: Some("echo 'Archiving {change_id}'".to_string()),
            analyze_command: None,
            hooks: None,
        };
        let runner = AgentRunner::new(config);
        let status = runner.run_archive("test-change").await.unwrap();
        assert!(status.success());
    }

    #[tokio::test]
    async fn test_analyze_dependencies_echo_command() {
        let config = OrchestratorConfig {
            apply_command: None,
            archive_command: None,
            analyze_command: Some("echo '{prompt}'".to_string()),
            hooks: None,
        };
        let runner = AgentRunner::new(config);
        let result = runner.analyze_dependencies("test prompt").await.unwrap();
        assert!(result.contains("test prompt"));
    }

    #[tokio::test]
    async fn test_run_apply_streaming() {
        let config = OrchestratorConfig {
            apply_command: Some("echo 'line1' && echo 'line2'".to_string()),
            archive_command: None,
            analyze_command: None,
            hooks: None,
        };
        let runner = AgentRunner::new(config);
        let (mut child, mut rx) = runner.run_apply_streaming("test-change").await.unwrap();

        let mut lines = Vec::new();
        while let Some(line) = rx.recv().await {
            lines.push(line);
        }

        let status = child.wait().await.unwrap();
        assert!(status.success());
        assert!(!lines.is_empty());
    }
}
