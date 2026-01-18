//! Shared acceptance operations for CLI and TUI modes.
//!
//! Provides acceptance test execution after apply and before archive.

#![allow(dead_code)]

use crate::acceptance::{parse_acceptance_output, AcceptanceResult as ParseResult};
use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::history::{AcceptanceAttempt, OutputCollector};
use crate::openspec::Change;
use tracing::{info, warn};

use super::output::OutputHandler;

/// Result of an acceptance operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed - can proceed to archive.
    Pass,
    /// Acceptance failed - must return to apply loop.
    Fail { findings: Vec<String> },
    /// Acceptance command execution failed (non-zero exit).
    CommandFailed { error: String },
    /// Acceptance was cancelled (e.g., by user or timeout).
    Cancelled,
}

impl AcceptanceResult {
    /// Returns true if acceptance passed.
    pub fn is_pass(&self) -> bool {
        matches!(self, AcceptanceResult::Pass)
    }
}

/// Run acceptance test for a change with streaming output.
///
/// # Arguments
/// * `change` - The change to test
/// * `agent` - The agent runner for executing commands
/// * `output` - Output handler for streaming command output
/// * `cancel_check` - Function to check if operation should be cancelled
///
/// # Returns
/// * `Ok(AcceptanceResult::Pass)` - Acceptance passed
/// * `Ok(AcceptanceResult::Fail { findings })` - Acceptance failed with findings
/// * `Ok(AcceptanceResult::CommandFailed { error })` - Command execution failed
/// * `Ok(AcceptanceResult::Cancelled)` - Operation was cancelled
/// * `Err(e)` - An error occurred
pub async fn acceptance_test_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    output: &O,
    cancel_check: F,
) -> Result<AcceptanceResult>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::agent::OutputLine;

    info!("Running acceptance test for: {}", change.id);
    output.on_info(&format!("Acceptance test: {}", change.id));

    // Execute acceptance command with streaming
    let (mut child, mut output_rx, start_time) =
        agent.run_acceptance_streaming(&change.id, None).await?;

    // Create output collector for history and parsing
    let mut output_collector = OutputCollector::new();
    let mut full_stdout = String::new();

    // Stream output until channel closes
    while let Some(line) = output_rx.recv().await {
        // Check for cancellation
        if cancel_check() {
            warn!("Acceptance test cancelled for: {}", change.id);
            output.on_warn("Acceptance test cancelled");
            let _ = child.terminate();
            return Ok(AcceptanceResult::Cancelled);
        }

        match line {
            OutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                full_stdout.push_str(&s);
                full_stdout.push('\n');
                output.on_stdout(&s);
            }
            OutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);
                output.on_stderr(&s);
            }
        }
    }

    // Child has exited, wait for status
    let status = child.wait().await.map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed to wait for acceptance command for change '{}': {}",
            change.id, e
        ))
    })?;

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Parse acceptance output
    let parse_result = parse_acceptance_output(&full_stdout);

    // Check if command failed
    if !status.success() {
        let error_msg = format!(
            "Acceptance command failed with exit code: {:?}",
            status.code()
        );
        let attempt = AcceptanceAttempt {
            attempt: agent.next_acceptance_attempt_number(&change.id),
            passed: false,
            duration: start_time.elapsed(),
            findings: Some(vec![error_msg.clone()]),
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
        };
        agent.record_acceptance_attempt(&change.id, attempt);
        output.on_error(&error_msg);
        return Ok(AcceptanceResult::CommandFailed { error: error_msg });
    }

    // Process parsed result
    match parse_result {
        ParseResult::Pass => {
            info!("Acceptance passed for: {}", change.id);
            let attempt = AcceptanceAttempt {
                attempt: agent.next_acceptance_attempt_number(&change.id),
                passed: true,
                duration: start_time.elapsed(),
                findings: None,
                exit_code: status.code(),
                stdout_tail,
                stderr_tail: stderr_tail.clone(),
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_success("Acceptance test passed");
            Ok(AcceptanceResult::Pass)
        }
        ParseResult::Fail { findings } => {
            info!(
                "Acceptance failed for: {} with {} findings",
                change.id,
                findings.len()
            );
            let attempt = AcceptanceAttempt {
                attempt: agent.next_acceptance_attempt_number(&change.id),
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(findings.clone()),
                exit_code: status.code(),
                stdout_tail,
                stderr_tail,
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_warn(&format!(
                "Acceptance test failed with {} findings",
                findings.len()
            ));
            for finding in &findings {
                output.on_warn(&format!("  - {}", finding));
            }
            Ok(AcceptanceResult::Fail { findings })
        }
    }
}

/// Update tasks.md on acceptance failure.
///
/// Adds a follow-up task at the end of tasks.md with the acceptance failure reason.
/// The task is added in unchecked state to signal that work needs to be done.
///
/// # Arguments
/// * `change_id` - The change ID
/// * `findings` - The acceptance failure findings
/// * `workspace_path` - Optional workspace path for parallel execution
///
/// # Returns
/// * `Ok(())` - Task file updated successfully
/// * `Err(e)` - Failed to update task file
pub async fn update_tasks_on_acceptance_failure(
    change_id: &str,
    findings: &[String],
    workspace_path: Option<&std::path::Path>,
) -> Result<()> {
    use std::path::PathBuf;
    use tokio::fs;

    // Determine tasks.md path (worktree or base tree)
    let tasks_path: PathBuf = if let Some(wt_path) = workspace_path {
        wt_path
            .join("openspec/changes")
            .join(change_id)
            .join("tasks.md")
    } else {
        PathBuf::from("openspec/changes")
            .join(change_id)
            .join("tasks.md")
    };

    // Read current tasks.md content
    let content = fs::read_to_string(&tasks_path).await.map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read tasks file {:?}: {}", tasks_path, e))
    })?;

    // Format findings into a single follow-up task
    let findings_text = if findings.len() == 1 {
        findings[0].clone()
    } else {
        findings
            .iter()
            .enumerate()
            .map(|(i, f)| format!("  {}) {}", i + 1, f))
            .collect::<Vec<_>>()
            .join("\n")
    };

    // Create follow-up task
    let follow_up_task = format!(
        "\n\n## Acceptance Failure Follow-up\n- [ ] Address acceptance findings:\n{}",
        findings_text
    );

    // Append to tasks.md
    let updated_content = format!("{}{}", content, follow_up_task);
    fs::write(&tasks_path, updated_content).await.map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to write tasks file {:?}: {}",
            tasks_path, e
        ))
    })?;

    info!(
        "Updated tasks.md for {} with {} acceptance findings",
        change_id,
        findings.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acceptance_result_is_pass() {
        assert!(AcceptanceResult::Pass.is_pass());
        assert!(!AcceptanceResult::Fail {
            findings: vec!["error".to_string()]
        }
        .is_pass());
        assert!(!AcceptanceResult::CommandFailed {
            error: "test".to_string()
        }
        .is_pass());
        assert!(!AcceptanceResult::Cancelled.is_pass());
    }
}
