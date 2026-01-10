//! Apply attempt history tracking module.
//!
//! This module provides in-memory tracking of apply attempts per change,
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
}
