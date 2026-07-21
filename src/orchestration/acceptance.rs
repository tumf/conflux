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
pub const MAX_ACCEPTANCE_RETRY_CYCLES: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFinding {
    pub identity: String,
    pub external: bool,
}

pub fn normalize_findings(findings: &[String]) -> Vec<NormalizedFinding> {
    let mut normalized = findings
        .iter()
        .filter_map(|finding| {
            let normalized = finding.split_whitespace().collect::<Vec<_>>().join(" ");
            (!normalized.is_empty()).then(|| {
                let lower = normalized.to_ascii_lowercase();
                let path_token = lower
                    .split_whitespace()
                    .find(|word| {
                        word.contains('/') || word.ends_with(".rs") || word.ends_with(".md")
                    })
                    .unwrap_or("");
                // Coordinates are unstable finding context, not identity. Remove the
                // complete token so `src/lib.rs:10:2` cannot leave `:10:2` behind.
                let path = path_token
                    .trim_matches(|character: char| matches!(character, '`' | '(' | ')' | ','))
                    .split(':')
                    .next()
                    .unwrap_or("");
                // An explicit non-mockable prerequisite is external only when the
                // finding has no repository target or requested repository repair.
                let external = path.is_empty()
                    && !lower.contains("fix ")
                    && !lower.contains("repair ")
                    && [
                        "external non-mockable",
                        "non-mockable external",
                        "external prerequisite",
                        "external service outage",
                        "missing non-mockable external credential",
                    ]
                    .iter()
                    .any(|needle| lower.contains(needle));
                let message = lower
                    .replace(path_token, "")
                    .split_whitespace()
                    .filter(|word| !word.chars().all(|character| character.is_ascii_digit()))
                    .collect::<Vec<_>>()
                    .join(" ");
                NormalizedFinding {
                    identity: format!(
                        "{}|{}|{}",
                        if external { "external" } else { "repository" },
                        path,
                        message
                    ),
                    external,
                }
            })
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.identity.cmp(&right.identity));
    normalized.dedup_by(|left, right| left.identity == right.identity);
    normalized
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceRetryDecision {
    Retry {
        reason: &'static str,
    },
    Stall {
        reason: &'static str,
        external_blockers: Vec<String>,
    },
}

pub fn repository_findings(findings: &[String]) -> Vec<String> {
    findings
        .iter()
        .filter(|finding| {
            normalize_findings(std::slice::from_ref(*finding))
                .first()
                .is_some_and(|normalized| !normalized.external)
        })
        .cloned()
        .collect()
}

pub fn semantic_progress_fingerprint(workspace: &std::path::Path) -> std::io::Result<String> {
    fn include(path: &str) -> bool {
        !path.starts_with(".git/")
            && !path.starts_with(".cflx/")
            && !path.contains("/APPLY_BLOCKED/")
            && !path.starts_with("logs/")
            && !path.starts_with("history/")
            && (path.starts_with("src/")
                || path.starts_with("tests/")
                || path.starts_with("config/")
                || path.starts_with("openspec/specs/")
                || path.contains("/specs/")
                || path == ".cflx.jsonc"
                || path.ends_with("/.cflx.jsonc")
                || path.ends_with("Cargo.toml")
                || path.ends_with("tasks.md"))
    }
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        output: &mut Vec<(String, Vec<u8>)>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, output)?;
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if include(&relative) {
                let mut contents = std::fs::read(path)?;
                if relative.ends_with("tasks.md") {
                    let text = String::from_utf8_lossy(&contents);
                    contents = text
                        .split("\n## Acceptance #")
                        .next()
                        .unwrap_or(&text)
                        .as_bytes()
                        .to_vec();
                }
                output.push((relative, contents));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(workspace, workspace, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let hash = files
        .into_iter()
        .flat_map(|(path, bytes)| path.into_bytes().into_iter().chain(bytes))
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        });
    Ok(format!("{hash:016x}"))
}

pub fn decide_acceptance_retry(
    previous_identities: &[String],
    previous_fingerprint: Option<&str>,
    findings: &[NormalizedFinding],
    semantic_fingerprint: &str,
    cycle_count: u32,
) -> AcceptanceRetryDecision {
    let identities = findings
        .iter()
        .map(|finding| finding.identity.clone())
        .collect::<Vec<_>>();
    let external_blockers = findings
        .iter()
        .filter(|finding| finding.external)
        .map(|finding| finding.identity.clone())
        .collect();
    if cycle_count >= MAX_ACCEPTANCE_RETRY_CYCLES {
        return AcceptanceRetryDecision::Stall {
            reason: "acceptance_cycle_limit_exhausted",
            external_blockers,
        };
    }
    if !findings.is_empty() && findings.iter().all(|finding| finding.external) {
        return AcceptanceRetryDecision::Stall {
            reason: "external_acceptance_blocker",
            external_blockers,
        };
    }
    if previous_identities.is_empty() {
        return AcceptanceRetryDecision::Retry {
            reason: "first_acceptance_failure",
        };
    }
    if previous_identities != identities || previous_fingerprint != Some(semantic_fingerprint) {
        return AcceptanceRetryDecision::Retry {
            reason: "finding_or_semantic_progress_changed",
        };
    }
    AcceptanceRetryDecision::Stall {
        reason: "repeated_acceptance_findings",
        external_blockers,
    }
}

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
        .filter(|line| {
            let trimmed = line.trim();
            // Filter out empty lines, ACCEPTANCE: markers, and FINDINGS: lines
            !trimmed.is_empty()
                && !trimmed.starts_with("ACCEPTANCE:")
                && !trimmed.starts_with("FINDINGS:")
        })
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
    /// Acceptance gated due to implementation blocker - stop apply loop.
    Gated,
    /// Acceptance command execution failed (non-zero exit).
    CommandFailed {
        error: String,
        findings: Vec<String>,
    },
    /// Acceptance detected a repeated unresolved permission/policy blocker.
    PermissionStalled {
        blocker: crate::events::StalledBlocker,
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
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
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

    // Execute acceptance command with streaming via AiCommandRunner (real process handle)
    let (mut child, mut output_rx, start_time, command) = agent
        .run_acceptance_streaming_with_runner(&change.id, ai_runner, None, base_branch.as_deref())
        .await?;

    // Log acceptance started with command
    output.on_info(&format!("Acceptance started: {}", change.id));
    output.on_info(&format!("  Command: {}", command));

    // Create output collector for history and parsing
    let mut output_collector = OutputCollector::new();
    let mut full_stdout = String::new();

    // Grace period after detecting an acceptance marker before terminating the process.
    // This handles the case where the agent process (or its child processes) does not exit
    // promptly after emitting ACCEPTANCE: PASS/FAIL/etc., for example because
    // child processes (MCP servers) keep stdout/stderr pipes open.
    const MARKER_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(30);

    // Stream output until channel closes or acceptance marker detected + grace period
    let mut marker_detected = false;
    let mut verdict_stream_detector = crate::acceptance::VerdictStreamDetector::default();
    let mut marker_deadline: Option<tokio::time::Instant> = None;
    let mut early_terminated = false;

    loop {
        let recv_future = output_rx.recv();

        let line = if let Some(deadline) = marker_deadline {
            // After marker detection, apply a timeout for remaining output
            match tokio::time::timeout_at(deadline, recv_future).await {
                Ok(Some(line)) => line,
                Ok(None) => break, // Channel closed normally
                Err(_) => {
                    // Grace period expired — terminate the process
                    warn!(
                        "Acceptance marker grace period expired for {}, terminating process",
                        change.id
                    );
                    let _ = child.terminate();
                    early_terminated = true;
                    break;
                }
            }
        } else {
            match tokio::time::timeout(std::time::Duration::from_millis(50), recv_future).await {
                Ok(Some(line)) => line,
                Ok(None) => break, // Channel closed normally
                Err(_) => {
                    if cancel_check() {
                        warn!("Acceptance test cancelled while waiting for output");
                        output.on_warn("Acceptance test cancelled");
                        let _ = child.terminate();
                        return Ok((AcceptanceResult::Cancelled, 0, command));
                    }
                    continue;
                }
            }
        };

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

                // Detect a canonical verdict in stdout to start the grace
                // period. This prevents indefinite blocking when the agent
                // process does not exit after emitting the verdict. The
                // detector recognises the primary strict JSON verdict (as a
                // standalone line or wrapped in a supported agent JSONL event)
                // and, as fallback, the legacy standalone plain-text
                // marker. Malformed markers with trailing text (for example
                // "ACCEPTANCE: PASSAll ...") do NOT trigger early completion.
                if !marker_detected && verdict_stream_detector.detect(&s).is_some() {
                    marker_detected = true;
                    marker_deadline = Some(tokio::time::Instant::now() + MARKER_GRACE_PERIOD);
                    info!(
                        "Acceptance canonical verdict detected for {}, starting {}s grace period",
                        change.id,
                        MARKER_GRACE_PERIOD.as_secs()
                    );
                }
            }
            OutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);
                output.on_agent_stderr(&s);
            }
        }
    }

    // Child has exited, wait for status. Keep this cancellation-aware because
    // the output channel may close before the process status is reaped.
    let status = loop {
        if cancel_check() {
            warn!(
                "Acceptance test cancelled while waiting for child status for: {}",
                change.id
            );
            output.on_warn("Acceptance test cancelled");
            let _ = child.terminate();
            return Ok((AcceptanceResult::Cancelled, 0, command));
        }

        match tokio::time::timeout(std::time::Duration::from_millis(50), child.wait()).await {
            Ok(status) => {
                break status.map_err(|e| {
                    OrchestratorError::AgentCommand(format!(
                        "Failed to wait for acceptance command for change '{}': {}",
                        change.id, e
                    ))
                })?;
            }
            Err(_) => continue,
        }
    };

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Build tail findings for history recording (last N lines, used in AcceptanceAttempt).
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // A verdict-finalized run is one we terminated after observing the
    // canonical standalone verdict. The non-zero exit from termination is
    // expected — the verdict drives the final result.
    let verdict_finalized_run = early_terminated && marker_detected;

    // Check if command failed (skip when verdict already finalized).
    if !status.success() && !verdict_finalized_run {
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
        crate::acceptance::AcceptanceResult::Fail {
            findings: parsed_findings,
        } => {
            info!("Acceptance test failed for: {}", change.id);
            output.on_warn("Acceptance test: FAIL");
            let findings = if parsed_findings.is_empty() {
                vec!["Investigate acceptance failure and apply the required fix".to_string()]
            } else {
                parsed_findings
            };
            (AcceptanceResult::Fail { findings }, false)
        }
        crate::acceptance::AcceptanceResult::Continue => {
            info!("Acceptance requires continuation for: {}", change.id);
            output.on_info("Acceptance test: CONTINUE");
            (AcceptanceResult::Continue, false)
        }
        crate::acceptance::AcceptanceResult::Gated => {
            info!("Acceptance gated for: {}", change.id);
            output.on_warn("Acceptance test: GATED");
            (AcceptanceResult::Gated, false)
        }
    };

    let history_findings = match &result {
        AcceptanceResult::Fail { findings } => Some(findings.clone()),
        AcceptanceResult::Continue => {
            Some(vec!["Investigation incomplete - continue later".to_string()])
        }
        AcceptanceResult::Gated => Some(vec!["Implementation blocker detected".to_string()]),
        AcceptanceResult::Pass => None,
        AcceptanceResult::CommandFailed { .. }
        | AcceptanceResult::PermissionStalled { .. }
        | AcceptanceResult::Cancelled => Some(tail_findings.clone()),
    };
    let attempt_number = agent.next_acceptance_attempt_number(&change.id);
    let attempt = AcceptanceAttempt {
        attempt: attempt_number,
        passed,
        duration: start_time.elapsed(),
        findings: history_findings,
        exit_code: status.code(),
        stdout_tail,
        stderr_tail,
        commit_hash: commit_hash.clone(),
    };
    agent.record_acceptance_attempt(&change.id, attempt);
    match &result {
        AcceptanceResult::Fail { findings } => {
            let repository_findings = repository_findings(findings);
            if !repository_findings.is_empty() {
                agent.record_acceptance_follow_up(&change.id, attempt_number, repository_findings);
            }
        }
        AcceptanceResult::Pass => agent.clear_acceptance_follow_up(&change.id),
        _ => {}
    }
    Ok((result, attempt_number, command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_fingerprint_excludes_runtime_bookkeeping() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/lib.rs"), "one").unwrap();
        let before = semantic_progress_fingerprint(temp.path()).unwrap();
        std::fs::create_dir_all(temp.path().join(".cflx")).unwrap();
        std::fs::write(temp.path().join(".cflx/acceptance-state.json"), "runtime").unwrap();
        assert_eq!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(temp.path().join("src/lib.rs"), "two").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
    }

    #[test]
    fn retry_decision_normalizes_order_whitespace_duplicates_and_stalls_repeats() {
        let findings = normalize_findings(&[
            " src/lib.rs:10   missing  test ".to_string(),
            "src/lib.rs:11 missing test".to_string(),
        ]);
        assert_eq!(findings.len(), 1);
        let decision = decide_acceptance_retry(
            &[findings[0].identity.clone()],
            Some("unchanged"),
            &findings,
            "unchanged",
            2,
        );
        assert!(matches!(
            decision,
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ..
            }
        ));
    }

    #[test]
    fn retry_decision_stalls_external_only_and_allows_progress_changed() {
        let findings = normalize_findings(&["external service outage".to_string()]);
        assert!(findings[0].external);
        assert!(matches!(
            decide_acceptance_retry(&[], None, &findings, "one", 1),
            AcceptanceRetryDecision::Stall {
                reason: "external_acceptance_blocker",
                ..
            }
        ));
        assert!(
            matches!(decide_acceptance_retry(&[], None, &findings, "one", MAX_ACCEPTANCE_RETRY_CYCLES), AcceptanceRetryDecision::Stall { reason: "acceptance_cycle_limit_exhausted", external_blockers } if external_blockers.len() == 1)
        );
    }

    #[test]
    fn semantic_fingerprint_tracks_change_specs_and_jsonc_but_excludes_runtime_follow_up() {
        let temp = tempfile::TempDir::new().unwrap();
        let tasks = temp.path().join("openspec/changes/example/tasks.md");
        let spec = temp
            .path()
            .join("openspec/changes/example/specs/runtime/spec.md");
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        std::fs::write(&tasks, "## Implementation Tasks\n- [x] work\n").unwrap();
        std::fs::write(&spec, "requirement one").unwrap();
        std::fs::write(temp.path().join(".cflx.jsonc"), "{ \"mode\": 1 }").unwrap();
        let before = semantic_progress_fingerprint(temp.path()).unwrap();
        std::fs::write(&tasks, "## Implementation Tasks\n- [x] work\n\n## Acceptance #2 Failure Follow-up\n- [ ] runtime finding\n").unwrap();
        assert_eq!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(&spec, "requirement two").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(temp.path().join(".cflx.jsonc"), "{ \"mode\": 2 }").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
    }

    #[test]
    fn generic_credential_and_unavailable_errors_remain_repository_fixable() {
        let findings = normalize_findings(&[
            "missing API key in test fixture".to_string(),
            "src/client.rs: rate limit retry missing".to_string(),
            "network unreachable: fix retry handling".to_string(),
            "dns resolution failed while repairing src/client.rs".to_string(),
            "missing non-mockable external credential".to_string(),
        ]);
        assert_eq!(
            findings.iter().filter(|finding| finding.external).count(),
            1
        );
        assert!(findings[0].identity.starts_with("external|"));
        assert_eq!(
            repository_findings(&[
                "missing API key in test fixture".to_string(),
                "src/client.rs: rate limit retry missing".to_string(),
                "network unreachable: fix retry handling".to_string(),
                "dns resolution failed while repairing src/client.rs".to_string(),
                "missing non-mockable external credential".to_string(),
            ])
            .len(),
            4
        );
    }

    #[test]
    fn alternating_continue_and_fail_keeps_fail_retry_history_deterministic() {
        let findings = normalize_findings(&["src/lib.rs:10 missing regression coverage".into()]);
        let identities = findings
            .iter()
            .map(|finding| finding.identity.clone())
            .collect::<Vec<_>>();

        // CONTINUE never enters the FAIL retry decision; the first later FAIL
        // remains the repair opportunity and the repeated FAIL then stalls.
        assert!(matches!(
            decide_acceptance_retry(&[], None, &findings, "unchanged", 1),
            AcceptanceRetryDecision::Retry {
                reason: "first_acceptance_failure"
            }
        ));
        assert!(matches!(
            decide_acceptance_retry(&identities, Some("unchanged"), &findings, "unchanged", 2),
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ..
            }
        ));
    }

    #[test]
    fn serial_and_parallel_same_inputs_have_retry_outcome_parity() {
        let findings = normalize_findings(&[
            "src/lib.rs:10 missing regression coverage".into(),
            "external non-mockable prerequisite unavailable".into(),
        ]);
        let previous = findings
            .iter()
            .map(|finding| finding.identity.clone())
            .collect::<Vec<_>>();

        // Both execution modes call this shared pure decision with checkpoint
        // state. Keep an explicit parity fixture for their common boundary.
        let serial = decide_acceptance_retry(&previous, Some("same"), &findings, "same", 2);
        let parallel = decide_acceptance_retry(&previous, Some("same"), &findings, "same", 2);
        assert_eq!(serial, parallel);
        assert!(matches!(
            serial,
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ref external_blockers
            } if external_blockers.len() == 1
        ));
    }

    #[test]
    fn retry_decision_handles_mixed_and_findingless_failures() {
        let mixed = normalize_findings(&[
            "src/lib.rs:1 fix test".into(),
            "external service outage".into(),
        ]);
        assert!(matches!(
            decide_acceptance_retry(&[], None, &mixed, "one", 1),
            AcceptanceRetryDecision::Retry { .. }
        ));
        assert!(matches!(
            decide_acceptance_retry(&[], None, &[], "one", 1),
            AcceptanceRetryDecision::Retry { .. }
        ));
        assert_eq!(
            repository_findings(&[
                "src/lib.rs:1 fix test".into(),
                "external service outage".into(),
            ]),
            vec!["src/lib.rs:1 fix test"]
        );
    }

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
        assert!(!AcceptanceResult::PermissionStalled {
            blocker: crate::events::StalledBlocker::acceptance_infrastructure("permission denied"),
        }
        .is_pass());
        assert!(!AcceptanceResult::Cancelled.is_pass());
        assert!(!AcceptanceResult::Gated.is_pass());
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_acceptance_marker() {
        let findings = build_acceptance_tail_findings(
            Some("line 1\nACCEPTANCE: FAIL\nline 2".to_string()),
            None,
        );

        assert_eq!(findings, vec!["line 1", "line 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_findings_line() {
        let findings = build_acceptance_tail_findings(
            Some("error 1\nFINDINGS:\n- item 1\n- item 2".to_string()),
            None,
        );

        assert_eq!(findings, vec!["error 1", "- item 1", "- item 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_both_markers() {
        let findings = build_acceptance_tail_findings(
            Some("ACCEPTANCE: FAIL\nFINDINGS:\nactual error\nanother line".to_string()),
            None,
        );

        assert_eq!(findings, vec!["actual error", "another line"]);
    }

    // Characterization tests: document the difference between tail_findings
    // and parse_acceptance_output findings so the refactor is clearly motivated.

    #[test]
    fn test_tail_findings_includes_preamble_parse_does_not() {
        // tail_findings includes all non-marker lines (preamble, postamble, items).
        // parse_acceptance_output findings includes only FINDINGS section items.
        // The refactor unifies the FAIL path to use parse_acceptance_output findings.
        let stdout = "preamble\nACCEPTANCE: FAIL\nFINDINGS:\n- Finding 1\n- Finding 2\npostamble"
            .to_string();

        let tail = build_acceptance_tail_findings(Some(stdout.clone()), None);
        // tail includes preamble, finding items, postamble
        assert!(tail.iter().any(|l| l.contains("preamble")));
        assert!(tail.iter().any(|l| l.contains("postamble")));
        assert!(tail.iter().any(|l| l.contains("Finding 1")));

        // parse_acceptance_output returns only the FINDINGS section items
        match crate::acceptance::parse_acceptance_output(&stdout) {
            crate::acceptance::AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["Finding 1", "Finding 2"]);
                assert!(!findings.iter().any(|f| f.contains("preamble")));
                assert!(!findings.iter().any(|f| f.contains("postamble")));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_findings_is_preferred_source_for_fail_result() {
        // After the refactor: for AcceptanceResult::Fail, findings come from
        // parse_acceptance_output (FINDINGS section), not from build_acceptance_tail_findings.
        let stdout =
            "ACCEPTANCE: FAIL\nFINDINGS:\n- src/foo.rs:10 issue A\n- src/bar.rs:5 issue B\n"
                .to_string();

        match crate::acceptance::parse_acceptance_output(&stdout) {
            crate::acceptance::AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "src/foo.rs:10 issue A");
                assert_eq!(findings[1], "src/bar.rs:5 issue B");
            }
            _ => panic!("Expected Fail"),
        }
    }
}
