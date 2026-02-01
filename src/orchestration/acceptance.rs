//! Shared acceptance operations for CLI and TUI modes.
//!
//! Provides acceptance test execution after apply and before archive.

#![allow(dead_code)]

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
/// * `Ok((AcceptanceResult::Pass, attempt_number))` - Acceptance passed
/// * `Ok((AcceptanceResult::Fail { findings }, attempt_number))` - Acceptance failed with findings
/// * `Ok((AcceptanceResult::CommandFailed { error, findings }, attempt_number))` - Command execution failed
/// * `Ok((AcceptanceResult::Cancelled, attempt_number))` - Operation was cancelled
/// * `Err(e)` - An error occurred
///
/// The attempt_number is the number of the acceptance attempt that was just recorded.
pub async fn acceptance_test_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    _ai_runner: &crate::ai_command_runner::AiCommandRunner,
    _config: &crate::config::OrchestratorConfig,
    output: &O,
    cancel_check: F,
) -> Result<(AcceptanceResult, u32, String)>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::agent::OutputLine;

    info!("Running acceptance test for: {}", change.id);
    output.on_info(&format!("Acceptance test: {}", change.id));

    // Capture current commit hash for diff tracking
    let commit_hash = crate::vcs::git::commands::get_current_commit(".")
        .await
        .ok(); // Allow to fail silently (non-git repos)

    // Get current branch for diff context (first acceptance needs base branch)
    let base_branch = crate::vcs::git::commands::get_current_branch(".")
        .await
        .ok()
        .flatten(); // None if in detached HEAD or non-git repo

    // Execute acceptance command with streaming
    let (mut child, mut output_rx, start_time, command) = agent
        .run_acceptance_streaming(&change.id, None, base_branch.as_deref())
        .await?;

    // Log acceptance started with command
    output.on_info(&format!("Acceptance started: {}", change.id));
    output.on_info(&format!("  Command: {}", command));

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
            // Note: For cancellation, we don't record an attempt, so return 0
            return Ok((AcceptanceResult::Cancelled, 0, command));
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

    // TODO: Use actual command output (tail_findings) instead of parsing
    // Build findings from last N lines of output
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // Check if command failed
    if !status.success() {
        let error_msg = format!(
            "Acceptance command failed with exit code: {:?}",
            status.code()
        );
        let attempt_number = agent.next_acceptance_attempt_number(&change.id);
        let attempt = AcceptanceAttempt {
            attempt: attempt_number,
            passed: false,
            duration: start_time.elapsed(),
            findings: Some(tail_findings.clone()),
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            commit_hash: commit_hash.clone(),
        };
        agent.record_acceptance_attempt(&change.id, attempt);
        output.on_error(&error_msg);
        return Ok((
            AcceptanceResult::CommandFailed {
                error: error_msg,
                findings: tail_findings,
            },
            attempt_number,
            command,
        ));
    }

    // Parse acceptance output to determine result
    let parsed_result = crate::acceptance::parse_acceptance_output(&full_stdout);

    let (result, passed) = match parsed_result {
        crate::acceptance::AcceptanceResult::Pass => {
            info!("Acceptance test passed for: {}", change.id);
            output.on_info("Acceptance test: PASS");
            (AcceptanceResult::Pass, true)
        }
        crate::acceptance::AcceptanceResult::Fail { findings } => {
            info!("Acceptance test failed for: {}", change.id);
            output.on_warn("Acceptance test: FAIL");
            (AcceptanceResult::Fail { findings }, false)
        }
        crate::acceptance::AcceptanceResult::Continue => {
            info!("Acceptance requires continuation for: {}", change.id);
            output.on_info("Acceptance test: CONTINUE");
            (AcceptanceResult::Continue, false)
        }
    };

    let attempt_number = agent.next_acceptance_attempt_number(&change.id);
    let attempt = AcceptanceAttempt {
        attempt: attempt_number,
        passed,
        duration: start_time.elapsed(),
        findings: Some(tail_findings.clone()),
        exit_code: status.code(),
        stdout_tail,
        stderr_tail,
        commit_hash: commit_hash.clone(),
    };
    agent.record_acceptance_attempt(&change.id, attempt);
    Ok((result, attempt_number, command))
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
