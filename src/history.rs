//! Apply and archive attempt history tracking module.
//!
//! This module provides in-memory tracking of apply and archive attempts per change,
//! allowing context injection for subsequent retry attempts.

use std::collections::HashMap;
use std::time::Duration;

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

    /// Clear history for a change (call on archive)
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

                format!(
                    "<last_apply attempt=\"{}\">\nstatus: {}\nduration: {}s{}{}\n</last_apply>",
                    a.attempt, status, duration_secs, error_line, exit_code_line
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

                format!(
                    "<last_archive attempt=\"{}\">\nstatus: {}\nduration: {}s{}{}{}\n</last_archive>",
                    a.attempt, status, duration_secs, error_line, verification_line, exit_code_line
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
        });

        context.record(ResolveAttempt {
            attempt: 2,
            command_success: true,
            verification_success: false,
            duration: Duration::from_secs(40),
            continuation_reason: Some("MERGE_HEAD still exists".to_string()),
            exit_code: Some(0),
        });

        let formatted = context.format_continuation_context();

        assert!(formatted.contains("This is attempt 3 of 5"));
        assert!(formatted.contains("Previous attempt (1):"));
        assert!(formatted.contains("Previous attempt (2):"));
        assert!(formatted.contains("Conflict markers remain"));
        assert!(formatted.contains("MERGE_HEAD still exists"));
    }
}
