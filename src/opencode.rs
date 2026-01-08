use crate::error::{OrchestratorError, Result};
use std::path::PathBuf;
use std::process::ExitStatus;
use tokio::process::Command;
use tracing::{debug, info};

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

    /// Run OpenCode command in headless mode
    /// Process exit = command completion
    pub async fn run_command(&self, command: &str, args: &str) -> Result<ExitStatus> {
        info!("Running OpenCode command: {} {}", command, args);

        let full_command = format!("{} {}", command, args);

        let mut child = Command::new(&self.opencode_path)
            .arg("run")
            .arg(&full_command)
            .spawn()
            .map_err(|e| {
                OrchestratorError::OpenCodeCommand(format!("Failed to spawn process: {}", e))
            })?;

        let status = child.wait().await.map_err(|e| {
            OrchestratorError::OpenCodeCommand(format!("Failed to wait for process: {}", e))
        })?;

        debug!("OpenCode command exited with status: {:?}", status);
        Ok(status)
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
