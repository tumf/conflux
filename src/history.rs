//! Apply and archive attempt history tracking module.
//!
//! This module provides in-memory tracking of apply and archive attempts per change,
//! allowing context injection for subsequent retry attempts.

use std::collections::HashMap;
use std::time::Duration;

/// Default number of tail lines to capture from stdout/stderr
const DEFAULT_TAIL_LINES: usize = 50;

/// Collects stdout/stderr output and captures the last N lines as a summary.
#[derive(Debug, Clone)]
pub struct OutputCollector {
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
    max_lines: usize,
}

impl OutputCollector {
    /// Create a new OutputCollector with default tail line count.
    pub fn new() -> Self {
        Self::with_max_lines(DEFAULT_TAIL_LINES)
    }

    /// Create a new OutputCollector with a specified maximum tail line count.
    pub fn with_max_lines(max_lines: usize) -> Self {
        Self {
            stdout_lines: Vec::new(),
            stderr_lines: Vec::new(),
            max_lines,
        }
    }

    /// Add a stdout line to the collector.
    pub fn add_stdout(&mut self, line: &str) {
        self.stdout_lines.push(line.to_string());
        // Keep only the last N lines to avoid unbounded memory growth
        if self.stdout_lines.len() > self.max_lines {
            self.stdout_lines.remove(0);
        }
    }

    /// Add a stderr line to the collector.
    pub fn add_stderr(&mut self, line: &str) {
        self.stderr_lines.push(line.to_string());
        // Keep only the last N lines to avoid unbounded memory growth
        if self.stderr_lines.len() > self.max_lines {
            self.stderr_lines.remove(0);
        }
    }

    /// Get the stdout tail summary as a single string.
    /// Returns None if no stdout was captured.
    pub fn stdout_tail(&self) -> Option<String> {
        if self.stdout_lines.is_empty() {
            None
        } else {
            Some(self.stdout_lines.join("\n"))
        }
    }

    /// Get the stderr tail summary as a single string.
    /// Returns None if no stderr was captured.
    pub fn stderr_tail(&self) -> Option<String> {
        if self.stderr_lines.is_empty() {
            None
        } else {
            Some(self.stderr_lines.join("\n"))
        }
    }
}

impl Default for OutputCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single apply attempt
#[derive(Debug, Clone)]
pub struct ApplyAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Whether the attempt succeeded
    pub success: bool,
    /// Duration of the attempt
    pub duration: Duration,
    /// Error message if failed (None if success)
    pub error: Option<String>,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Last N lines of stdout (tail summary)
    pub stdout_tail: Option<String>,
    /// Last N lines of stderr (tail summary)
    pub stderr_tail: Option<String>,
}

/// Tracks apply attempts per change
pub struct ApplyHistory {
    /// Map of change_id to list of attempts
    attempts: HashMap<String, Vec<ApplyAttempt>>,
}

impl ApplyHistory {
    /// Create a new empty ApplyHistory
    pub fn new() -> Self {
        Self {
            attempts: HashMap::new(),
        }
    }

    /// Record a new attempt for a change
    pub fn record(&mut self, change_id: &str, attempt: ApplyAttempt) {
        self.attempts
            .entry(change_id.to_string())
            .or_default()
            .push(attempt);
    }

    /// Get all attempts for a change
    #[allow(dead_code)]
    pub fn get(&self, change_id: &str) -> Option<&[ApplyAttempt]> {
        self.attempts.get(change_id).map(|v| v.as_slice())
    }

    /// Get the last attempt for a change
    #[allow(dead_code)]
    pub fn last(&self, change_id: &str) -> Option<&ApplyAttempt> {
        self.attempts.get(change_id).and_then(|v| v.last())
    }

    /// Get attempt count for a change
    pub fn count(&self, change_id: &str) -> u32 {
        self.attempts
            .get(change_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }

    /// Clear history for a change (call on successful archive)
    #[allow(dead_code)]
    pub fn clear(&mut self, change_id: &str) {
        self.attempts.remove(change_id);
    }

    /// Format history as context string for prompt injection.
    /// Returns an empty string if there are no previous attempts.
    pub fn format_context(&self, change_id: &str) -> String {
        let Some(attempts) = self.attempts.get(change_id) else {
            return String::new();
        };

        if attempts.is_empty() {
            return String::new();
        }

        attempts
            .iter()
            .map(|a| {
                let status = if a.success { "success" } else { "failed" };
                let duration_secs = a.duration.as_secs();
                let error_line = match &a.error {
                    Some(e) => format!("\nerror: {}", e),
                    None => String::new(),
                };
                let exit_code_line = match a.exit_code {
                    Some(code) => format!("\nexit_code: {}", code),
                    None => String::new(),
                };
                let stdout_line = match &a.stdout_tail {
                    Some(s) if !s.is_empty() => format!("\nstdout_tail:\n{}", s),
                    _ => String::new(),
                };
                let stderr_line = match &a.stderr_tail {
                    Some(s) if !s.is_empty() => format!("\nstderr_tail:\n{}", s),
                    _ => String::new(),
                };

                format!(
                    "<last_apply attempt=\"{}\">\nstatus: {}\nduration: {}s{}{}{}{}\n</last_apply>",
                    a.attempt,
                    status,
                    duration_secs,
                    error_line,
                    exit_code_line,
                    stdout_line,
                    stderr_line
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Default for ApplyHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single archive attempt
#[derive(Debug, Clone)]
pub struct ArchiveAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Whether the attempt succeeded
    pub success: bool,
    /// Duration of the attempt
    pub duration: Duration,
    /// Error message if failed (None if success)
    pub error: Option<String>,
    /// Verification result (e.g., reason why NotArchived)
    pub verification_result: Option<String>,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Last N lines of stdout (tail summary)
    pub stdout_tail: Option<String>,
    /// Last N lines of stderr (tail summary)
    pub stderr_tail: Option<String>,
}

/// Tracks archive attempts per change
pub struct ArchiveHistory {
    /// Map of change_id to list of attempts
    attempts: HashMap<String, Vec<ArchiveAttempt>>,
}

impl ArchiveHistory {
    /// Create a new empty ArchiveHistory
    pub fn new() -> Self {
        Self {
            attempts: HashMap::new(),
        }
    }

    /// Record a new attempt for a change
    pub fn record(&mut self, change_id: &str, attempt: ArchiveAttempt) {
        self.attempts
            .entry(change_id.to_string())
            .or_default()
            .push(attempt);
    }

    /// Get all attempts for a change
    #[allow(dead_code)]
    pub fn get(&self, change_id: &str) -> Option<&[ArchiveAttempt]> {
        self.attempts.get(change_id).map(|v| v.as_slice())
    }

    /// Get attempt count for a change
    pub fn count(&self, change_id: &str) -> u32 {
        self.attempts
            .get(change_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }

    /// Clear history for a change (call on successful archive)
    pub fn clear(&mut self, change_id: &str) {
        self.attempts.remove(change_id);
    }

    /// Format history as context string for prompt injection.
    /// Returns an empty string if there are no previous attempts.
    pub fn format_context(&self, change_id: &str) -> String {
        let Some(attempts) = self.attempts.get(change_id) else {
            return String::new();
        };

        if attempts.is_empty() {
            return String::new();
        }

        attempts
            .iter()
            .map(|a| {
                let status = if a.success { "success" } else { "failed" };
                let duration_secs = a.duration.as_secs();
                let error_line = match &a.error {
                    Some(e) => format!("\nerror: {}", e),
                    None => String::new(),
                };
                let verification_line = match &a.verification_result {
                    Some(v) => format!("\nverification_result: {}", v),
                    None => String::new(),
                };
                let exit_code_line = match a.exit_code {
                    Some(code) => format!("\nexit_code: {}", code),
                    None => String::new(),
                };
                let stdout_line = match &a.stdout_tail {
                    Some(s) if !s.is_empty() => format!("\nstdout_tail:\n{}", s),
                    _ => String::new(),
                };
                let stderr_line = match &a.stderr_tail {
                    Some(s) if !s.is_empty() => format!("\nstderr_tail:\n{}", s),
                    _ => String::new(),
                };

                format!(
                    "<last_archive attempt=\"{}\">\nstatus: {}\nduration: {}s{}{}{}{}{}\n</last_archive>",
                    a.attempt, status, duration_secs, error_line, verification_line, exit_code_line, stdout_line, stderr_line
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Default for ArchiveHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single acceptance attempt
#[derive(Debug, Clone)]
pub struct AcceptanceAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Whether the acceptance passed
    pub passed: bool,
    /// Duration of the attempt
    pub duration: Duration,
    /// Findings if failed (None if passed)
    pub findings: Option<Vec<String>>,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Last N lines of stdout (tail summary)
    pub stdout_tail: Option<String>,
    /// Last N lines of stderr (tail summary)
    pub stderr_tail: Option<String>,
    /// Commit hash at the time of this acceptance check (for diff calculation)
    pub commit_hash: Option<String>,
}

/// Tracks acceptance attempts per change
pub struct AcceptanceHistory {
    /// Map of change_id to list of attempts
    attempts: HashMap<String, Vec<AcceptanceAttempt>>,
}

impl AcceptanceHistory {
    /// Create a new empty AcceptanceHistory
    pub fn new() -> Self {
        Self {
            attempts: HashMap::new(),
        }
    }

    /// Record a new attempt for a change
    pub fn record(&mut self, change_id: &str, attempt: AcceptanceAttempt) {
        self.attempts
            .entry(change_id.to_string())
            .or_default()
            .push(attempt);
    }

    /// Get all attempts for a change
    #[allow(dead_code)]
    pub fn get(&self, change_id: &str) -> Option<&[AcceptanceAttempt]> {
        self.attempts.get(change_id).map(|v| v.as_slice())
    }

    /// Get attempt count for a change
    pub fn count(&self, change_id: &str) -> u32 {
        self.attempts
            .get(change_id)
            .map(|v| v.len() as u32)
            .unwrap_or(0)
    }

    /// Clear history for a change (call on successful archive)
    pub fn clear(&mut self, change_id: &str) {
        self.attempts.remove(change_id);
    }

    /// Count consecutive CONTINUE attempts from the end of the history.
    /// A CONTINUE attempt is detected by checking if findings contain "Investigation incomplete - continue later".
    pub fn count_consecutive_continues(&self, change_id: &str) -> u32 {
        let Some(attempts) = self.attempts.get(change_id) else {
            return 0;
        };

        attempts
            .iter()
            .rev()
            .take_while(|a| {
                a.findings
                    .as_ref()
                    .and_then(|f| f.first())
                    .map(|s| s.contains("Investigation incomplete - continue later"))
                    .unwrap_or(false)
            })
            .count() as u32
    }

    /// Get the last commit hash from the most recent acceptance attempt.
    /// Returns None if there are no previous attempts or the last attempt has no commit hash.
    pub fn last_commit_hash(&self, change_id: &str) -> Option<String> {
        self.attempts
            .get(change_id)
            .and_then(|v| v.last())
            .and_then(|a| a.commit_hash.clone())
    }

    /// Get the last findings from the most recent acceptance attempt.
    /// Returns None if there are no previous attempts or the last attempt has no findings.
    pub fn last_findings(&self, change_id: &str) -> Option<Vec<String>> {
        self.attempts
            .get(change_id)
            .and_then(|v| v.last())
            .and_then(|a| a.findings.clone())
    }

    /// Format history as context string for prompt injection.
    /// Returns an empty string if there are no previous attempts.
    pub fn format_context(&self, change_id: &str) -> String {
        let Some(attempts) = self.attempts.get(change_id) else {
            return String::new();
        };

        if attempts.is_empty() {
            return String::new();
        }

        attempts
            .iter()
            .map(|a| {
                let status = if a.passed { "passed" } else { "failed" };
                let duration_secs = a.duration.as_secs();
                let findings_line = match &a.findings {
                    Some(f) if !f.is_empty() => {
                        let findings_text = f
                            .iter()
                            .map(|finding| format!("  - {}", finding))
                            .collect::<Vec<_>>()
                            .join("\n");
                        format!("\nfindings:\n{}", findings_text)
                    }
                    _ => String::new(),
                };
                let exit_code_line = match a.exit_code {
                    Some(code) => format!("\nexit_code: {}", code),
                    None => String::new(),
                };
                let stdout_line = match &a.stdout_tail {
                    Some(s) if !s.is_empty() => format!("\nstdout_tail:\n{}", s),
                    _ => String::new(),
                };
                let stderr_line = match &a.stderr_tail {
                    Some(s) if !s.is_empty() => format!("\nstderr_tail:\n{}", s),
                    _ => String::new(),
                };

                format!(
                    "<last_acceptance attempt=\"{}\">\nstatus: {}\nduration: {}s{}{}{}{}\n</last_acceptance>",
                    a.attempt, status, duration_secs, findings_line, exit_code_line, stdout_line, stderr_line
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Default for AcceptanceHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of a single resolve attempt
#[derive(Debug, Clone)]
pub struct ResolveAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Whether the command exited successfully
    pub command_success: bool,
    /// Whether verification passed
    pub verification_success: bool,
    /// Duration of the attempt
    pub duration: Duration,
    /// Reason why the resolve needs to continue (verification failure reason)
    pub continuation_reason: Option<String>,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Last N lines of stdout (tail summary)
    pub stdout_tail: Option<String>,
    /// Last N lines of stderr (tail summary)
    pub stderr_tail: Option<String>,
}

/// Tracks resolve attempts within a single retry session
pub struct ResolveContext {
    /// Attempts in the current session
    attempts: Vec<ResolveAttempt>,
    /// Maximum number of retries
    max_retries: u32,
}

impl ResolveContext {
    /// Create a new resolve context for a retry session
    pub fn new(max_retries: u32) -> Self {
        Self {
            attempts: Vec::new(),
            max_retries,
        }
    }

    /// Record a new attempt
    pub fn record(&mut self, attempt: ResolveAttempt) {
        self.attempts.push(attempt);
    }

    /// Get the current attempt number (1-based)
    pub fn current_attempt(&self) -> u32 {
        (self.attempts.len() as u32) + 1
    }

    /// Format continuation context for prompt injection.
    /// Returns an empty string if there are no previous attempts.
    pub fn format_continuation_context(&self) -> String {
        if self.attempts.is_empty() {
            return String::new();
        }

        let mut lines = vec![
            format!(
                "This is attempt {} of {} for conflict resolution.",
                self.current_attempt(),
                self.max_retries
            ),
            String::new(),
        ];

        for attempt in &self.attempts {
            let command_exit = if attempt.command_success {
                format!("success (code: {})", attempt.exit_code.unwrap_or(0))
            } else {
                format!("failed (code: {})", attempt.exit_code.unwrap_or(-1))
            };
            let verification = if attempt.verification_success {
                "passed"
            } else {
                "failed"
            };
            let duration_secs = attempt.duration.as_secs();

            lines.push(format!("Previous attempt ({}):", attempt.attempt));
            lines.push(format!("- Command exit: {}", command_exit));
            lines.push(format!("- Verification: {}", verification));
            if let Some(reason) = &attempt.continuation_reason {
                lines.push(format!("- Reason: {}", reason));
            }
            lines.push(format!("- Duration: {}s", duration_secs));
            if let Some(stdout) = &attempt.stdout_tail {
                if !stdout.is_empty() {
                    lines.push("- Stdout tail:".to_string());
                    lines.push(format!("  {}", stdout.replace('\n', "\n  ")));
                }
            }
            if let Some(stderr) = &attempt.stderr_tail {
                if !stderr.is_empty() {
                    lines.push("- Stderr tail:".to_string());
                    lines.push(format!("  {}", stderr.replace('\n', "\n  ")));
                }
            }
            lines.push(String::new());
        }

        if let Some(last) = self.attempts.last() {
            if let Some(reason) = &last.continuation_reason {
                lines.push(format!("Continue resolving the conflicts. {}", reason));
            } else {
                lines.push("Continue resolving the conflicts.".to_string());
            }
        }

        format!(
            "<resolve_context>\n{}\n</resolve_context>",
            lines.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_attempt(attempt: u32, success: bool, duration_secs: u64) -> ApplyAttempt {
        ApplyAttempt {
            attempt,
            success,
            duration: Duration::from_secs(duration_secs),
            error: if success {
                None
            } else {
                Some("Test error".to_string())
            },
            exit_code: if success { Some(0) } else { Some(1) },
            stdout_tail: None,
            stderr_tail: None,
        }
    }

    #[test]
    fn test_new_history_is_empty() {
        let history = ApplyHistory::new();
        assert!(history.get("any-change").is_none());
        assert_eq!(history.count("any-change"), 0);
    }

    #[test]
    fn test_record_and_retrieve() {
        let mut history = ApplyHistory::new();
        let attempt = create_test_attempt(1, false, 30);

        history.record("change-a", attempt);

        assert_eq!(history.count("change-a"), 1);
        let attempts = history.get("change-a").unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].attempt, 1);
        assert!(!attempts[0].success);
    }

    #[test]
    fn test_multiple_attempts_accumulation() {
        let mut history = ApplyHistory::new();

        history.record("change-a", create_test_attempt(1, false, 30));
        history.record("change-a", create_test_attempt(2, false, 45));
        history.record("change-a", create_test_attempt(3, true, 60));

        assert_eq!(history.count("change-a"), 3);

        let attempts = history.get("change-a").unwrap();
        assert_eq!(attempts[0].attempt, 1);
        assert_eq!(attempts[1].attempt, 2);
        assert_eq!(attempts[2].attempt, 3);

        let last = history.last("change-a").unwrap();
        assert_eq!(last.attempt, 3);
        assert!(last.success);
    }

    #[test]
    fn test_separate_changes_tracked_independently() {
        let mut history = ApplyHistory::new();

        history.record("change-a", create_test_attempt(1, false, 30));
        history.record("change-b", create_test_attempt(1, true, 20));
        history.record("change-a", create_test_attempt(2, true, 40));

        assert_eq!(history.count("change-a"), 2);
        assert_eq!(history.count("change-b"), 1);
    }

    #[test]
    fn test_clear_functionality() {
        let mut history = ApplyHistory::new();

        history.record("change-a", create_test_attempt(1, false, 30));
        history.record("change-a", create_test_attempt(2, true, 45));
        history.record("change-b", create_test_attempt(1, true, 20));

        assert_eq!(history.count("change-a"), 2);

        history.clear("change-a");

        assert_eq!(history.count("change-a"), 0);
        assert!(history.get("change-a").is_none());
        // change-b should be unaffected
        assert_eq!(history.count("change-b"), 1);
    }

    #[test]
    fn test_format_context_empty_history() {
        let history = ApplyHistory::new();
        let context = history.format_context("change-a");
        assert!(context.is_empty());
    }

    #[test]
    fn test_format_context_single_failed_attempt() {
        let mut history = ApplyHistory::new();
        history.record(
            "change-a",
            ApplyAttempt {
                attempt: 1,
                success: false,
                duration: Duration::from_secs(45),
                error: Some("Type error in auth.rs:42".to_string()),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
            },
        );

        let context = history.format_context("change-a");

        assert!(context.contains("<last_apply attempt=\"1\">"));
        assert!(context.contains("status: failed"));
        assert!(context.contains("duration: 45s"));
        assert!(context.contains("error: Type error in auth.rs:42"));
        assert!(context.contains("exit_code: 1"));
        assert!(context.contains("</last_apply>"));
    }

    #[test]
    fn test_format_context_successful_attempt() {
        let mut history = ApplyHistory::new();
        history.record(
            "change-a",
            ApplyAttempt {
                attempt: 1,
                success: true,
                duration: Duration::from_secs(30),
                error: None,
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
            },
        );

        let context = history.format_context("change-a");

        assert!(context.contains("status: success"));
        assert!(!context.contains("error:"));
        assert!(context.contains("exit_code: 0"));
    }

    #[test]
    fn test_format_context_multiple_attempts() {
        let mut history = ApplyHistory::new();
        history.record(
            "change-a",
            ApplyAttempt {
                attempt: 1,
                success: false,
                duration: Duration::from_secs(30),
                error: Some("Missing dependency".to_string()),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
            },
        );
        history.record(
            "change-a",
            ApplyAttempt {
                attempt: 2,
                success: false,
                duration: Duration::from_secs(45),
                error: Some("Type error".to_string()),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
            },
        );

        let context = history.format_context("change-a");

        // Should contain both attempts
        assert!(context.contains("<last_apply attempt=\"1\">"));
        assert!(context.contains("<last_apply attempt=\"2\">"));
        assert!(context.contains("Missing dependency"));
        assert!(context.contains("Type error"));
    }

    #[test]
    fn test_last_returns_none_for_unknown_change() {
        let history = ApplyHistory::new();
        assert!(history.last("unknown").is_none());
    }

    #[test]
    fn test_default_impl() {
        let history = ApplyHistory::default();
        assert_eq!(history.count("any"), 0);
    }

    // ArchiveHistory tests
    fn create_test_archive_attempt(
        attempt: u32,
        success: bool,
        duration_secs: u64,
        verification_result: Option<String>,
    ) -> ArchiveAttempt {
        ArchiveAttempt {
            attempt,
            success,
            duration: Duration::from_secs(duration_secs),
            error: if success {
                None
            } else {
                Some("Archive verification failed".to_string())
            },
            verification_result,
            exit_code: if success { Some(0) } else { Some(1) },
            stdout_tail: None,
            stderr_tail: None,
        }
    }

    #[test]
    fn test_archive_history_new() {
        let history = ArchiveHistory::new();
        assert!(history.get("any-change").is_none());
        assert_eq!(history.count("any-change"), 0);
    }

    #[test]
    fn test_archive_history_record_and_retrieve() {
        let mut history = ArchiveHistory::new();
        let attempt = create_test_archive_attempt(
            1,
            false,
            5,
            Some("Change still exists at openspec/changes/my-change".to_string()),
        );

        history.record("change-a", attempt);

        assert_eq!(history.count("change-a"), 1);
        let attempts = history.get("change-a").unwrap();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].attempt, 1);
        assert!(!attempts[0].success);
    }

    #[test]
    fn test_archive_history_multiple_attempts() {
        let mut history = ArchiveHistory::new();

        history.record(
            "change-a",
            create_test_archive_attempt(1, false, 5, Some("Change not archived".to_string())),
        );
        history.record(
            "change-a",
            create_test_archive_attempt(2, false, 6, Some("Change not archived".to_string())),
        );
        history.record("change-a", create_test_archive_attempt(3, true, 7, None));

        assert_eq!(history.count("change-a"), 3);
    }

    #[test]
    fn test_archive_history_clear() {
        let mut history = ArchiveHistory::new();

        history.record(
            "change-a",
            create_test_archive_attempt(1, false, 5, Some("Not archived".to_string())),
        );
        history.record("change-b", create_test_archive_attempt(1, true, 5, None));

        assert_eq!(history.count("change-a"), 1);

        history.clear("change-a");

        assert_eq!(history.count("change-a"), 0);
        assert!(history.get("change-a").is_none());
        // change-b should be unaffected
        assert_eq!(history.count("change-b"), 1);
    }

    #[test]
    fn test_archive_history_format_context_empty() {
        let history = ArchiveHistory::new();
        let context = history.format_context("change-a");
        assert!(context.is_empty());
    }

    #[test]
    fn test_archive_history_format_context_single_attempt() {
        let mut history = ArchiveHistory::new();
        history.record(
            "change-a",
            ArchiveAttempt {
                attempt: 1,
                success: false,
                duration: Duration::from_secs(5),
                error: Some("Archive command succeeded but verification failed".to_string()),
                verification_result: Some(
                    "Change still exists at openspec/changes/my-change".to_string(),
                ),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
            },
        );

        let context = history.format_context("change-a");

        assert!(context.contains("<last_archive attempt=\"1\">"));
        assert!(context.contains("status: failed"));
        assert!(context.contains("duration: 5s"));
        assert!(context.contains("error: Archive command succeeded but verification failed"));
        assert!(context.contains("verification_result: Change still exists"));
        assert!(context.contains("exit_code: 0"));
        assert!(context.contains("</last_archive>"));
    }

    #[test]
    fn test_archive_history_format_context_multiple_attempts() {
        let mut history = ArchiveHistory::new();
        history.record(
            "change-a",
            ArchiveAttempt {
                attempt: 1,
                success: false,
                duration: Duration::from_secs(5),
                error: Some("Verification failed".to_string()),
                verification_result: Some("Change not moved".to_string()),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
            },
        );
        history.record(
            "change-a",
            ArchiveAttempt {
                attempt: 2,
                success: false,
                duration: Duration::from_secs(6),
                error: Some("Still not archived".to_string()),
                verification_result: Some("Change still exists".to_string()),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
            },
        );

        let context = history.format_context("change-a");

        // Should contain both attempts
        assert!(context.contains("<last_archive attempt=\"1\">"));
        assert!(context.contains("<last_archive attempt=\"2\">"));
        assert!(context.contains("Change not moved"));
        assert!(context.contains("Change still exists"));
    }

    #[test]
    fn test_archive_history_default() {
        let history = ArchiveHistory::default();
        assert_eq!(history.count("any"), 0);
    }

    // ResolveContext tests
    #[test]
    fn test_resolve_context_new() {
        let context = ResolveContext::new(3);
        assert_eq!(context.current_attempt(), 1);
        assert!(context.format_continuation_context().is_empty());
    }

    #[test]
    fn test_resolve_context_record() {
        let mut context = ResolveContext::new(3);

        context.record(ResolveAttempt {
            attempt: 1,
            command_success: true,
            verification_success: false,
            duration: Duration::from_secs(45),
            continuation_reason: Some(
                "Conflicts still present after resolution attempt: src/main.rs".to_string(),
            ),
            exit_code: Some(0),
            stdout_tail: None,
            stderr_tail: None,
        });

        assert_eq!(context.current_attempt(), 2);
    }

    #[test]
    fn test_resolve_context_format_continuation() {
        let mut context = ResolveContext::new(3);

        context.record(ResolveAttempt {
            attempt: 1,
            command_success: true,
            verification_success: false,
            duration: Duration::from_secs(45),
            continuation_reason: Some(
                "Conflicts still present after resolution attempt: src/main.rs, src/lib.rs"
                    .to_string(),
            ),
            exit_code: Some(0),
            stdout_tail: None,
            stderr_tail: None,
        });

        let formatted = context.format_continuation_context();

        assert!(formatted.contains("<resolve_context>"));
        assert!(formatted.contains("This is attempt 2 of 3 for conflict resolution"));
        assert!(formatted.contains("Previous attempt (1):"));
        assert!(formatted.contains("Command exit: success (code: 0)"));
        assert!(formatted.contains("Verification: failed"));
        assert!(formatted.contains("Reason: Conflicts still present"));
        assert!(formatted.contains("Duration: 45s"));
        assert!(formatted.contains("Continue resolving the conflicts"));
        assert!(formatted.contains("</resolve_context>"));
    }

    #[test]
    fn test_resolve_context_multiple_attempts() {
        let mut context = ResolveContext::new(5);

        context.record(ResolveAttempt {
            attempt: 1,
            command_success: true,
            verification_success: false,
            duration: Duration::from_secs(30),
            continuation_reason: Some("Conflict markers remain".to_string()),
            exit_code: Some(0),
            stdout_tail: None,
            stderr_tail: None,
        });

        context.record(ResolveAttempt {
            attempt: 2,
            command_success: true,
            verification_success: false,
            duration: Duration::from_secs(40),
            continuation_reason: Some("MERGE_HEAD still exists".to_string()),
            exit_code: Some(0),
            stdout_tail: None,
            stderr_tail: None,
        });

        let formatted = context.format_continuation_context();

        assert!(formatted.contains("This is attempt 3 of 5"));
        assert!(formatted.contains("Previous attempt (1):"));
        assert!(formatted.contains("Previous attempt (2):"));
        assert!(formatted.contains("Conflict markers remain"));
        assert!(formatted.contains("MERGE_HEAD still exists"));
    }

    // OutputCollector tests
    #[test]
    fn test_output_collector_new() {
        let collector = OutputCollector::new();
        assert!(collector.stdout_tail().is_none());
        assert!(collector.stderr_tail().is_none());
    }

    #[test]
    fn test_output_collector_add_stdout() {
        let mut collector = OutputCollector::new();
        collector.add_stdout("line 1");
        collector.add_stdout("line 2");

        let stdout = collector.stdout_tail().unwrap();
        assert_eq!(stdout, "line 1\nline 2");
    }

    #[test]
    fn test_output_collector_add_stderr() {
        let mut collector = OutputCollector::new();
        collector.add_stderr("error 1");
        collector.add_stderr("error 2");

        let stderr = collector.stderr_tail().unwrap();
        assert_eq!(stderr, "error 1\nerror 2");
    }

    #[test]
    fn test_output_collector_max_lines() {
        let mut collector = OutputCollector::with_max_lines(3);
        collector.add_stdout("line 1");
        collector.add_stdout("line 2");
        collector.add_stdout("line 3");
        collector.add_stdout("line 4");
        collector.add_stdout("line 5");

        let stdout = collector.stdout_tail().unwrap();
        assert_eq!(stdout, "line 3\nline 4\nline 5");
        assert!(!stdout.contains("line 1"));
        assert!(!stdout.contains("line 2"));
    }

    #[test]
    fn test_output_collector_default() {
        let collector = OutputCollector::default();
        assert!(collector.stdout_tail().is_none());
        assert!(collector.stderr_tail().is_none());
    }

    // AcceptanceHistory tests
    #[test]
    fn test_acceptance_history_last_commit_hash() {
        let mut history = AcceptanceHistory::new();

        // No history - should return None
        assert!(history.last_commit_hash("change-a").is_none());

        // Add attempt with commit hash
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 1,
                passed: false,
                duration: Duration::from_secs(30),
                findings: Some(vec!["Issue 1".to_string()]),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: Some("abc123".to_string()),
            },
        );

        // Should return the commit hash
        assert_eq!(
            history.last_commit_hash("change-a"),
            Some("abc123".to_string())
        );

        // Add another attempt with different commit hash
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 2,
                passed: true,
                duration: Duration::from_secs(45),
                findings: None,
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: Some("def456".to_string()),
            },
        );

        // Should return the last commit hash
        assert_eq!(
            history.last_commit_hash("change-a"),
            Some("def456".to_string())
        );
    }

    #[test]
    fn test_acceptance_history_last_commit_hash_none() {
        let mut history = AcceptanceHistory::new();

        // Add attempt without commit hash
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 1,
                passed: false,
                duration: Duration::from_secs(30),
                findings: Some(vec!["Issue 1".to_string()]),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: None,
            },
        );

        // Should return None
        assert!(history.last_commit_hash("change-a").is_none());
    }

    #[test]
    fn test_acceptance_history_last_findings() {
        let mut history = AcceptanceHistory::new();

        // No history - should return None
        assert!(history.last_findings("change-a").is_none());

        // Add attempt with findings
        let findings1 = vec!["Issue 1".to_string(), "Issue 2".to_string()];
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 1,
                passed: false,
                duration: Duration::from_secs(30),
                findings: Some(findings1.clone()),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: Some("abc123".to_string()),
            },
        );

        // Should return the findings
        assert_eq!(history.last_findings("change-a"), Some(findings1));

        // Add another attempt with different findings
        let findings2 = vec!["Fixed issue 1".to_string()];
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 2,
                passed: false,
                duration: Duration::from_secs(45),
                findings: Some(findings2.clone()),
                exit_code: Some(1),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: Some("def456".to_string()),
            },
        );

        // Should return the last findings
        assert_eq!(history.last_findings("change-a"), Some(findings2));

        // Add passed attempt with no findings
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 3,
                passed: true,
                duration: Duration::from_secs(50),
                findings: None,
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: Some("ghi789".to_string()),
            },
        );

        // Should return None (last attempt has no findings)
        assert!(history.last_findings("change-a").is_none());
    }
}
