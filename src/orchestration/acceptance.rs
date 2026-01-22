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
) -> Result<(AcceptanceResult, u32)>
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
            // Note: For cancellation, we don't record an attempt, so return 0
            return Ok((AcceptanceResult::Cancelled, 0));
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
        ));
    }

    // Process parsed result
    match parse_result {
        ParseResult::Pass => {
            info!("Acceptance passed for: {}", change.id);
            let attempt_number = agent.next_acceptance_attempt_number(&change.id);
            let attempt = AcceptanceAttempt {
                attempt: attempt_number,
                passed: true,
                duration: start_time.elapsed(),
                findings: None,
                exit_code: status.code(),
                stdout_tail,
                stderr_tail: stderr_tail.clone(),
                commit_hash: commit_hash.clone(),
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_success("Acceptance test passed");
            Ok((AcceptanceResult::Pass, attempt_number))
        }
        ParseResult::Continue => {
            info!("Acceptance requires continuation for: {}", change.id);
            let attempt_number = agent.next_acceptance_attempt_number(&change.id);
            let attempt = AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(vec!["Investigation incomplete - continue later".to_string()]),
                exit_code: status.code(),
                stdout_tail,
                stderr_tail,
                commit_hash: commit_hash.clone(),
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_info("Acceptance test requires continuation");
            Ok((AcceptanceResult::Continue, attempt_number))
        }
        ParseResult::Fail { .. } => {
            let findings_for_tasks = tail_findings.clone();
            info!(
                "Acceptance failed for: {} with {} findings",
                change.id,
                findings_for_tasks.len()
            );
            let attempt_number = agent.next_acceptance_attempt_number(&change.id);
            let attempt = AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(findings_for_tasks.clone()),
                exit_code: status.code(),
                stdout_tail,
                stderr_tail,
                commit_hash: commit_hash.clone(),
            };
            agent.record_acceptance_attempt(&change.id, attempt);
            output.on_warn(&format!(
                "Acceptance test failed with {} findings",
                findings_for_tasks.len()
            ));
            for finding in &findings_for_tasks {
                output.on_warn(&format!("  - {}", finding));
            }
            Ok((
                AcceptanceResult::Fail {
                    findings: findings_for_tasks,
                },
                attempt_number,
            ))
        }
    }
}

/// Update tasks.md on acceptance failure.
///
/// Adds a follow-up section at the end of tasks.md with the acceptance failure findings.
/// Each finding is added as a separate unchecked task to signal that work needs to be done.
///
/// # Arguments
/// * `change_id` - The change ID
/// * `findings` - The acceptance failure findings
/// * `workspace_path` - Optional workspace path for parallel execution
/// * `attempt_number` - The acceptance attempt number (1-based)
///
/// # Returns
/// * `Ok(())` - Task file updated successfully
/// * `Err(e)` - Failed to update task file
pub async fn update_tasks_on_acceptance_failure(
    change_id: &str,
    findings: &[String],
    workspace_path: Option<&std::path::Path>,
    attempt_number: u32,
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

    // Format findings as individual tasks (no wrapper task)
    let findings_tasks = findings
        .iter()
        .map(|f| format!("- [ ] {}", f))
        .collect::<Vec<_>>()
        .join("\n");

    // Create follow-up section with attempt number
    let follow_up_section = format!(
        "\n\n## Acceptance #{} Failure Follow-up\n{}",
        attempt_number, findings_tasks
    );

    // Append to tasks.md
    let updated_content = format!("{}{}", content, follow_up_section);
    fs::write(&tasks_path, updated_content).await.map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to write tasks file {:?}: {}",
            tasks_path, e
        ))
    })?;

    info!(
        "Updated tasks.md for {} with {} acceptance findings (attempt #{})",
        change_id,
        findings.len(),
        attempt_number
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

    #[tokio::test]
    async fn test_update_tasks_on_acceptance_failure_format() {
        use tempfile::TempDir;
        use tokio::fs;

        // Create temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let change_id = "test-change";
        let change_dir = temp_dir.path().join("openspec/changes").join(change_id);
        fs::create_dir_all(&change_dir).await.unwrap();

        // Create initial tasks.md
        let tasks_path = change_dir.join("tasks.md");
        let initial_content = "## 1. タスク\n- [x] 完了済み\n- [ ] 未完了\n";
        fs::write(&tasks_path, initial_content).await.unwrap();

        // Call update_tasks_on_acceptance_failure with attempt #1
        let findings = vec![
            "Type error in auth.rs:42".to_string(),
            "Missing import statement".to_string(),
            "Unit test failed: test_login".to_string(),
        ];
        update_tasks_on_acceptance_failure(change_id, &findings, Some(temp_dir.path()), 1)
            .await
            .unwrap();

        // Read updated content
        let updated_content = fs::read_to_string(&tasks_path).await.unwrap();

        // Verify format
        assert!(
            updated_content.contains("## Acceptance #1 Failure Follow-up"),
            "Should contain numbered header"
        );
        assert!(
            updated_content.contains("- [ ] Type error in auth.rs:42"),
            "Should contain first finding as individual task"
        );
        assert!(
            updated_content.contains("- [ ] Missing import statement"),
            "Should contain second finding as individual task"
        );
        assert!(
            updated_content.contains("- [ ] Unit test failed: test_login"),
            "Should contain third finding as individual task"
        );
        assert!(
            !updated_content.contains("Address acceptance findings"),
            "Should not contain wrapper task"
        );
        assert!(
            !updated_content.contains("  - Type error"),
            "Should not have nested bullet points"
        );
    }

    #[tokio::test]
    async fn test_update_tasks_on_acceptance_failure_multiple_attempts() {
        use tempfile::TempDir;
        use tokio::fs;

        // Create temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let change_id = "test-change";
        let change_dir = temp_dir.path().join("openspec/changes").join(change_id);
        fs::create_dir_all(&change_dir).await.unwrap();

        // Create initial tasks.md
        let tasks_path = change_dir.join("tasks.md");
        let initial_content = "## 1. タスク\n- [x] 完了済み\n";
        fs::write(&tasks_path, initial_content).await.unwrap();

        // First failure (attempt #1)
        let findings1 = vec!["Error 1".to_string()];
        update_tasks_on_acceptance_failure(change_id, &findings1, Some(temp_dir.path()), 1)
            .await
            .unwrap();

        // Second failure (attempt #2)
        let findings2 = vec!["Error 2".to_string()];
        update_tasks_on_acceptance_failure(change_id, &findings2, Some(temp_dir.path()), 2)
            .await
            .unwrap();

        // Read updated content
        let updated_content = fs::read_to_string(&tasks_path).await.unwrap();

        // Verify both follow-up sections exist with correct numbering
        assert!(
            updated_content.contains("## Acceptance #1 Failure Follow-up"),
            "Should contain first follow-up"
        );
        assert!(
            updated_content.contains("## Acceptance #2 Failure Follow-up"),
            "Should contain second follow-up"
        );
        assert!(
            updated_content.contains("- [ ] Error 1"),
            "Should contain first error"
        );
        assert!(
            updated_content.contains("- [ ] Error 2"),
            "Should contain second error"
        );
    }
}
