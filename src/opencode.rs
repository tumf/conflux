//! OpenCode runner module (legacy - use agent.rs instead)
//!
//! This module is kept for backwards compatibility but is not actively used.
//! The AgentRunner in agent.rs provides the same functionality with configurable commands.

#![allow(dead_code)]

use crate::error::{OrchestratorError, Result};
use std::path::PathBuf;
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

/// Manages OpenCode process execution in headless mode
pub struct OpenCodeRunner {
    opencode_path: PathBuf,
}

impl OpenCodeRunner {
    /// Create a new OpenCodeRunner
    pub fn new(opencode_path: impl Into<PathBuf>) -> Self {
        Self {
            opencode_path: opencode_path.into(),
        }
    }

    /// Run OpenCode command in headless mode with output streaming
    /// Returns a child process handle and a receiver for output lines
    pub async fn run_command_streaming(
        &self,
        command: &str,
        args: &str,
    ) -> Result<(Child, mpsc::Receiver<OutputLine>)> {
        info!("Running OpenCode command: {} {}", command, args);

        let full_command = format!("{} {}", command, args);

        let mut child = Command::new(&self.opencode_path)
            .arg("run")
            .arg(&full_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                OrchestratorError::OpenCodeCommand(format!("Failed to spawn process: {}", e))
            })?;

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

    /// Run OpenCode command in headless mode (blocking, no streaming)
    /// Process exit = command completion
    pub async fn run_command(&self, command: &str, args: &str) -> Result<ExitStatus> {
        info!("Running OpenCode command: {} {}", command, args);

        let full_command = format!("{} {}", command, args);

        let output = Command::new(&self.opencode_path)
            .arg("run")
            .arg(&full_command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::OpenCodeCommand(format!("Failed to spawn process: {}", e))
            })?;

        debug!("OpenCode command exited with status: {:?}", output.status);
        Ok(output.status)
    }

    /// Analyze dependencies using OpenCode with JSON output
    pub async fn analyze_dependencies(&self, prompt: &str) -> Result<String> {
        info!("Analyzing dependencies with OpenCode");

        let output = Command::new(&self.opencode_path)
            .arg("run")
            .arg("--format")
            .arg("json")
            .arg(prompt)
            .output()
            .await
            .map_err(|e| {
                OrchestratorError::OpenCodeCommand(format!("Failed to execute analysis: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::OpenCodeCommand(format!(
                "Analysis failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)?;
        debug!("Analysis result: {}", stdout);
        Ok(stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_runner_creation() {
        let runner = OpenCodeRunner::new("opencode");
        assert_eq!(runner.opencode_path, PathBuf::from("opencode"));
    }
}
