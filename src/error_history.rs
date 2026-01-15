use regex::Regex;
use std::collections::VecDeque;

/// Configuration for the error circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub threshold: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 5,
        }
    }
}

/// Tracks error history for a single change to detect repeated failures
#[derive(Debug, Clone)]
pub struct ErrorHistory {
    /// Recent normalized error messages (FIFO queue)
    recent_errors: VecDeque<String>,
    /// Circuit breaker configuration
    config: CircuitBreakerConfig,
}

impl ErrorHistory {
    /// Create a new error history tracker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            recent_errors: VecDeque::with_capacity(config.threshold),
            config,
        }
    }

    /// Record an error message
    pub fn record_error(&mut self, error_msg: &str) {
        let normalized = normalize_error_message(error_msg);

        // Keep only the most recent errors (up to threshold)
        if self.recent_errors.len() >= self.config.threshold {
            self.recent_errors.pop_front();
        }
        self.recent_errors.push_back(normalized);
    }

    /// Check if the same error has occurred consecutively, triggering the circuit breaker
    pub fn detect_same_error(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Need at least threshold errors to detect
        if self.recent_errors.len() < self.config.threshold {
            return false;
        }

        // Check if all recent errors are the same
        if let Some(first) = self.recent_errors.front() {
            self.recent_errors.iter().all(|e| e == first)
        } else {
            false
        }
    }

    /// Clear the error history (useful after successful operations)
    pub fn clear(&mut self) {
        self.recent_errors.clear();
    }

    /// Get the most recent error message (if any)
    pub fn last_error(&self) -> Option<&str> {
        self.recent_errors.back().map(|s| s.as_str())
    }

    /// Get the count of recorded errors
    pub fn error_count(&self) -> usize {
        self.recent_errors.len()
    }
}

/// Normalize an error message by removing variable parts and JSON field names
///
/// This function performs two-stage filtering:
/// 1. Remove JSON field patterns like `"field_name": value`
/// 2. Normalize common variable patterns (paths, numbers, etc.)
fn normalize_error_message(msg: &str) -> String {
    // Stage 1: Remove JSON field patterns to avoid false positives
    // Match patterns like: "is_error": false, "field": "value", etc.
    let json_field_regex = Regex::new(r#""[^"]+"\s*:\s*(?:false|true|null|"[^"]*"|\d+)"#)
        .expect("Invalid JSON field regex");
    let without_json_fields = json_field_regex.replace_all(msg, "");

    // Stage 2: Normalize the error message
    let msg = without_json_fields.as_ref();

    // Remove file paths (absolute and relative)
    let path_regex = Regex::new(r"(/[a-zA-Z0-9_.\-/]+|[a-zA-Z]:\\[a-zA-Z0-9_.\-\\]+)")
        .expect("Invalid path regex");
    let msg = path_regex.replace_all(msg, "<PATH>");

    // Remove line/column numbers (match each :digit pattern individually)
    let line_regex = Regex::new(r":\d+")
        .expect("Invalid line regex");
    let msg = line_regex.replace_all(&msg, ":<NUM>");

    // Remove timestamps
    let timestamp_regex = Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}")
        .expect("Invalid timestamp regex");
    let msg = timestamp_regex.replace_all(&msg, "<TIMESTAMP>");

    // Remove generic numbers
    let number_regex = Regex::new(r"\b\d+\b")
        .expect("Invalid number regex");
    let msg = number_regex.replace_all(&msg, "<NUM>");

    // Normalize whitespace
    let whitespace_regex = Regex::new(r"\s+")
        .expect("Invalid whitespace regex");
    let normalized = whitespace_regex.replace_all(&msg, " ");

    normalized.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_removes_paths() {
        let msg = "File not found: /home/user/project/src/main.rs";
        let normalized = normalize_error_message(msg);
        assert_eq!(normalized, "File not found: <PATH>");
    }

    #[test]
    fn test_normalize_removes_line_numbers() {
        let msg = "Error at main.rs:42:15";
        let normalized = normalize_error_message(msg);
        assert_eq!(normalized, "Error at main.rs:<NUM>:<NUM>");
    }

    #[test]
    fn test_normalize_removes_json_fields() {
        let msg = r#"Error occurred: {"is_error": false, "status": "ok"}"#;
        let normalized = normalize_error_message(msg);
        // JSON fields should be removed
        assert!(!normalized.contains("is_error"));
        assert!(!normalized.contains("false"));
        assert!(normalized.contains("Error occurred"));
    }

    #[test]
    fn test_normalize_handles_complex_json() {
        let msg = r#"Response: {"is_error": false, "message": "OK", "count": 42}"#;
        let normalized = normalize_error_message(msg);
        assert!(!normalized.contains("is_error"));
        assert!(!normalized.contains("message"));
        assert!(!normalized.contains("count"));
    }

    #[test]
    fn test_detect_same_error_with_threshold() {
        let config = CircuitBreakerConfig {
            enabled: true,
            threshold: 3,
        };
        let mut history = ErrorHistory::new(config);

        // Not triggered with fewer errors
        history.record_error("File not found: /path/a");
        assert!(!history.detect_same_error());

        history.record_error("File not found: /path/b");
        assert!(!history.detect_same_error());

        // Triggered on threshold
        history.record_error("File not found: /path/c");
        assert!(history.detect_same_error());
    }

    #[test]
    fn test_detect_same_error_different_errors() {
        let config = CircuitBreakerConfig {
            enabled: true,
            threshold: 3,
        };
        let mut history = ErrorHistory::new(config);

        history.record_error("File not found: /path/a");
        history.record_error("Permission denied: /path/b");
        history.record_error("File not found: /path/c");

        // Should not trigger with different errors
        assert!(!history.detect_same_error());
    }

    #[test]
    fn test_circuit_breaker_disabled() {
        let config = CircuitBreakerConfig {
            enabled: false,
            threshold: 3,
        };
        let mut history = ErrorHistory::new(config);

        history.record_error("Same error 1");
        history.record_error("Same error 2");
        history.record_error("Same error 3");

        // Should not trigger when disabled
        assert!(!history.detect_same_error());
    }

    #[test]
    fn test_clear_history() {
        let config = CircuitBreakerConfig::default();
        let mut history = ErrorHistory::new(config);

        history.record_error("Error 1");
        history.record_error("Error 2");
        assert_eq!(history.error_count(), 2);

        history.clear();
        assert_eq!(history.error_count(), 0);
        assert!(!history.detect_same_error());
    }

    #[test]
    fn test_last_error() {
        let config = CircuitBreakerConfig::default();
        let mut history = ErrorHistory::new(config);

        assert!(history.last_error().is_none());

        history.record_error("Error 1");
        assert!(history.last_error().is_some());

        history.record_error("Error 2");
        let last = history.last_error().unwrap();
        assert!(last.contains("Error"));
    }
}
