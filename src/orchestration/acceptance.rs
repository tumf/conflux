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

const ACCEPTANCE_OUTPUT_FALLBACK: &str = "No acceptance output captured";

pub fn build_acceptance_tail_findings(
    stdout_tail: Option<String>,
    stderr_tail: Option<String>,
) -> Vec<String> {
    let stdout = stdout_tail.filter(|text| !text.trim().is_empty());
    let stderr = stderr_tail.filter(|text| !text.trim().is_empty());
    let selected = stdout
        .or(stderr)
        .unwrap_or_else(|| ACCEPTANCE_OUTPUT_FALLBACK.to_string());
    let lines = selected
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![ACCEPTANCE_OUTPUT_FALLBACK.to_string()]
    } else {
        lines
    }
}

/// Result of an acceptance operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed - can proceed to archive.
    Pass,
    /// Acceptance failed - must return to apply loop.
    Fail { findings: Vec<String> },
    /// Acceptance requires more investigation - retry acceptance.
    Continue,
    /// Acceptance command execution failed (non-zero exit).
    CommandFailed {
        error: String,
        findings: Vec<String>,
    },
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
/// * `agent` - The agent runner for history tracking
/// * `ai_runner` - The AI command runner for command execution
/// * `config` - Orchestrator configuration
/// * `output` - Output handler for streaming command output
/// * `cancel_check` - Function to check if operation should be cancelled
///
/// # Returns
/// * `Ok(AcceptanceResult::Pass)` - Acceptance passed
/// * `Ok(AcceptanceResult::Fail { findings })` - Acceptance failed with findings
/// * `Ok(AcceptanceResult::CommandFailed { error, findings })` - Command execution failed
/// * `Ok(AcceptanceResult::Cancelled)` - Operation was cancelled
/// * `Err(e)` - An error occurred
pub async fn acceptance_test_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    config: &crate::config::OrchestratorConfig,
    output: &O,
    cancel_check: F,
) -> Result<AcceptanceResult>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::ai_command_runner::OutputLine as AiOutputLine;

    info!("Running acceptance test for: {}", change.id);
    output.on_info(&format!("Acceptance test: {}", change.id));

    // Get the acceptance iteration number (attempt number that will be used)
    let _acceptance_iteration = agent.next_acceptance_attempt_number(&change.id);

    // Build prompt with system instructions and history context
    let user_prompt = config.get_acceptance_prompt();
    let history_context = agent.format_acceptance_history(&change.id);
    let full_prompt =
        crate::agent::build_acceptance_prompt(&change.id, user_prompt, &history_context);

    // Expand change_id and prompt in command
    let template = config.get_acceptance_command();
    let command = crate::config::OrchestratorConfig::expand_change_id(template, &change.id);
    let command = crate::config::OrchestratorConfig::expand_prompt(&command, &full_prompt);

    // Capture start time for history recording
    let start_time = std::time::Instant::now();

    // Execute command via AiCommandRunner (with stagger and retry)
    let (mut child, mut output_rx) = ai_runner
        .execute_streaming_with_retry(&command, None)
        .await?;

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
            AiOutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                full_stdout.push_str(&s);
                full_stdout.push('\n');
                output.on_stdout(&s);
            }
            AiOutputLine::Stderr(s) => {
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
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

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
            findings: Some(tail_findings.clone()),
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
        };
        agent.record_acceptance_attempt(&change.id, attempt);
        output.on_error(&error_msg);
        return Ok(AcceptanceResult::CommandFailed {
            error: error_msg,
            findings: tail_findings,
        });
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
        ParseResult::Continue => {
            info!("Acceptance requires continuation for: {}", change.id);
            let attempt = AcceptanceAttempt {
                attempt: agent.next_acceptance_attempt_number(&change.id),
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(vec!["Investigation incomplete - continue later".to_string()]),
                exit_code: status.code(),
                stdout_tail,
                stderr_tail,
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_info("Acceptance test requires continuation");
            Ok(AcceptanceResult::Continue)
        }
        ParseResult::Fail { .. } => {
            let findings_for_tasks = tail_findings.clone();
            info!(
                "Acceptance failed for: {} with {} findings",
                change.id,
                findings_for_tasks.len()
            );
            let attempt = AcceptanceAttempt {
                attempt: agent.next_acceptance_attempt_number(&change.id),
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(findings_for_tasks.clone()),
                exit_code: status.code(),
                stdout_tail,
                stderr_tail,
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_warn(&format!(
                "Acceptance test failed with {} findings",
                findings_for_tasks.len()
            ));
            for finding in &findings_for_tasks {
                output.on_warn(&format!("  - {}", finding));
            }
            Ok(AcceptanceResult::Fail {
                findings: findings_for_tasks,
            })
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

    // Format findings into a single follow-up task (always use bullet points)
    let findings_text = findings
        .iter()
        .map(|f| format!("  - {}", f))
        .collect::<Vec<_>>()
        .join("\n");

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
    fn test_build_acceptance_tail_findings_prefers_stdout() {
        let findings = build_acceptance_tail_findings(
            Some("stdout line 1\nstdout line 2".to_string()),
            Some("stderr line".to_string()),
        );

        assert_eq!(findings, vec!["stdout line 1", "stdout line 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_falls_back_to_stderr() {
        let findings =
            build_acceptance_tail_findings(Some("  ".to_string()), Some("stderr".to_string()));

        assert_eq!(findings, vec!["stderr"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_fallback_message() {
        let findings = build_acceptance_tail_findings(None, Some("\n\n".to_string()));

        assert_eq!(findings, vec!["No acceptance output captured"]);
    }

    #[test]
    fn test_acceptance_result_is_pass() {
        assert!(AcceptanceResult::Pass.is_pass());
        assert!(!AcceptanceResult::Fail {
            findings: vec!["error".to_string()]
        }
        .is_pass());
        assert!(!AcceptanceResult::CommandFailed {
            error: "test".to_string(),
            findings: vec!["failure".to_string()],
        }
        .is_pass());
        assert!(!AcceptanceResult::Cancelled.is_pass());
    }
}
