//! Permission auto-reject detection module.
//!
//! This module provides utilities to detect permission auto-reject patterns
//! in agent output (stdout/stderr) and extract the denied path for user guidance.

use regex::Regex;
use std::sync::OnceLock;

/// Pattern to detect permission auto-reject in agent output.
///
/// This looks for combinations of "permission requested" followed by "auto-rejecting"
/// in the output tail. The pattern captures the denied path for guidance.
fn permission_reject_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Match patterns like:
        // "permission requested: bash ls" or "permission requested for: <path>"
        // followed by "auto-rejecting" or "auto-reject"
        // (?s) enables DOTALL mode (. matches newline)
        Regex::new(r"(?si)permission\s+requested[^:]*:\s*([^\n]+).*?auto-reject")
            .expect("Invalid regex")
    })
}

/// Result of permission auto-reject detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionReject {
    /// The path or command that was denied
    pub denied_path: String,
}

impl PermissionReject {
    /// Create a new PermissionReject
    pub fn new(denied_path: String) -> Self {
        Self { denied_path }
    }

    /// Format a user-friendly error message with configuration guidance
    pub fn format_error_message(&self) -> String {
        format!(
            "Permission auto-rejected for: {}\n\
            To resolve this, update the permission configuration in .cflx.jsonc:\n\
            1. Add permission for this specific operation, or\n\
            2. Set permission to 'allow' for this category\n\
            Example: {{\"permission\": {{\"bash\": \"allow\"}}}}",
            self.denied_path
        )
    }
}

/// Detect permission auto-reject pattern in output.
///
/// Searches for the pattern "permission requested" + "auto-rejecting" in the
/// combined stdout/stderr tail output.
///
/// # Arguments
///
/// * `stdout_tail` - Last N lines of stdout (optional)
/// * `stderr_tail` - Last N lines of stderr (optional)
///
/// # Returns
///
/// * `Some(PermissionReject)` - If auto-reject pattern is detected
/// * `None` - If no auto-reject pattern found
pub fn detect_permission_reject(
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> Option<PermissionReject> {
    // Combine stdout and stderr for pattern matching
    let combined = match (stdout_tail, stderr_tail) {
        (Some(out), Some(err)) => format!("{}\n{}", out, err),
        (Some(out), None) => out.to_string(),
        (None, Some(err)) => err.to_string(),
        (None, None) => return None,
    };

    // Search for the pattern
    let pattern = permission_reject_pattern();
    pattern.captures(&combined).map(|caps| {
        let denied_path = caps
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        PermissionReject::new(denied_path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_permission_reject_basic() {
        let stdout = "Some output\npermission requested: bash ls\nauto-rejecting\nMore output";
        let result = detect_permission_reject(Some(stdout), None);
        assert!(result.is_some());
        let reject = result.unwrap();
        assert_eq!(reject.denied_path, "bash ls");
    }

    #[test]
    fn test_detect_permission_reject_stderr() {
        let stderr = "Error: permission requested: /path/to/file\nauto-reject";
        let result = detect_permission_reject(None, Some(stderr));
        assert!(result.is_some());
        let reject = result.unwrap();
        assert_eq!(reject.denied_path, "/path/to/file");
    }

    #[test]
    fn test_detect_permission_reject_combined() {
        let stdout = "permission requested: git push";
        let stderr = "auto-rejecting request";
        let result = detect_permission_reject(Some(stdout), Some(stderr));
        assert!(result.is_some());
        let reject = result.unwrap();
        assert_eq!(reject.denied_path, "git push");
    }

    #[test]
    fn test_detect_permission_reject_case_insensitive() {
        let output = "Permission Requested: npm install\nAuto-Rejecting";
        let result = detect_permission_reject(Some(output), None);
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_permission_reject_no_match() {
        let output = "Normal output without permission issues";
        let result = detect_permission_reject(Some(output), None);
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_permission_reject_partial_match() {
        // Has "permission requested" but no "auto-reject"
        let output = "permission requested: bash echo";
        let result = detect_permission_reject(Some(output), None);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_error_message() {
        let reject = PermissionReject::new("bash rm -rf /".to_string());
        let message = reject.format_error_message();
        assert!(message.contains("bash rm -rf /"));
        assert!(message.contains("permission configuration"));
        assert!(message.contains(".cflx.jsonc"));
    }

    #[test]
    fn test_detect_permission_reject_multiline() {
        let output = "Line 1\nLine 2\npermission requested: write file.txt\nLine 3\nLine 4\nauto-rejecting\nLine 5";
        let result = detect_permission_reject(Some(output), None);
        assert!(result.is_some());
        let reject = result.unwrap();
        assert_eq!(reject.denied_path, "write file.txt");
    }

    #[test]
    fn test_detect_permission_reject_empty_input() {
        let result = detect_permission_reject(None, None);
        assert!(result.is_none());
    }
}
