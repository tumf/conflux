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

fn file_read_denied_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:read|open|access)\s+(?:permission\s+)?denied(?:\s+(?:for|to))?\s*:?\s*([^\n]*)")
            .expect("Invalid regex")
    })
}

fn tool_denied_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:tool|command)\s+(?:access\s+)?(?:denied|not\s+allowed|blocked|rejected)\s*:?\s*([^\n]*)")
            .expect("Invalid regex")
    })
}

fn harness_policy_denied_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)(?:permission|policy|sandbox|harness)[^\n]*(?:denied|not\s+allowed|blocked|rejected)\s*:?\s*([^\n]*)")
            .expect("Invalid regex")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDenialCategory {
    AutoReject,
    FileRead,
    ToolAccess,
    CommandPolicy,
}

impl PermissionDenialCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AutoReject => "auto_reject",
            Self::FileRead => "file_read",
            Self::ToolAccess => "tool_access",
            Self::CommandPolicy => "command_policy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenial {
    pub category: PermissionDenialCategory,
    pub denied_target: String,
    pub evidence: String,
}

impl PermissionDenial {
    pub fn signature(&self) -> PermissionDenialSignature {
        PermissionDenialSignature {
            category: self.category.clone(),
            denied_target: normalize_signature_target(&self.denied_target),
        }
    }

    pub fn format_guidance(&self) -> String {
        format!(
            "Repeated unresolved permission/tool policy denial detected for {}: {}. \
operator action required: update the local harness/tool permission policy or grant access, then resume the preserved workspace.",
            self.category.as_str(),
            self.denied_target
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenialSignature {
    pub category: PermissionDenialCategory,
    pub denied_target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenialTracker {
    last_signature: Option<PermissionDenialSignature>,
}

impl PermissionDenialTracker {
    pub fn new() -> Self {
        Self {
            last_signature: None,
        }
    }

    pub fn observe(
        &mut self,
        denial: &PermissionDenial,
        repository_visible_progress: bool,
    ) -> PermissionDenialObservation {
        let signature = denial.signature();
        let repeated = self.last_signature.as_ref() == Some(&signature);
        let stalled = repeated && !repository_visible_progress;

        if repository_visible_progress || !repeated {
            self.last_signature = Some(signature);
        }

        PermissionDenialObservation { stalled }
    }
}

impl Default for PermissionDenialTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionDenialObservation {
    pub stalled: bool,
}

/// Result of permission auto-reject detection
#[allow(dead_code)] // Legacy compatibility API; new flows use PermissionDenial directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionReject {
    /// The path or command that was denied
    pub denied_path: String,
}

#[allow(dead_code)] // Legacy compatibility API; exercised by tests and external callers.
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

fn combine_sources(sources: &[Option<&str>]) -> Option<String> {
    let combined = sources
        .iter()
        .filter_map(|source| source.map(str::trim).filter(|text| !text.is_empty()))
        .collect::<Vec<_>>()
        .join("\n");

    if combined.is_empty() {
        None
    } else {
        Some(combined)
    }
}

fn capture_target(caps: &regex::Captures<'_>) -> String {
    caps.get(1)
        .map(|m| m.as_str().trim())
        .filter(|text| !text.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn normalize_signature_target(target: &str) -> String {
    target
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub fn classify_permission_denial(sources: &[Option<&str>]) -> Option<PermissionDenial> {
    let combined = combine_sources(sources)?;

    if let Some(caps) = permission_reject_pattern().captures(&combined) {
        return Some(PermissionDenial {
            category: PermissionDenialCategory::AutoReject,
            denied_target: capture_target(&caps),
            evidence: caps
                .get(0)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| combined.clone()),
        });
    }

    if let Some(caps) = tool_denied_pattern().captures(&combined) {
        return Some(PermissionDenial {
            category: PermissionDenialCategory::ToolAccess,
            denied_target: capture_target(&caps),
            evidence: caps
                .get(0)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| combined.clone()),
        });
    }

    if let Some(caps) = file_read_denied_pattern().captures(&combined) {
        return Some(PermissionDenial {
            category: PermissionDenialCategory::FileRead,
            denied_target: capture_target(&caps),
            evidence: caps
                .get(0)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| combined.clone()),
        });
    }

    if let Some(caps) = harness_policy_denied_pattern().captures(&combined) {
        return Some(PermissionDenial {
            category: PermissionDenialCategory::CommandPolicy,
            denied_target: capture_target(&caps),
            evidence: caps
                .get(0)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or(combined),
        });
    }

    None
}

/// Detect permission auto-reject pattern in output.
///
/// Searches for the pattern "permission requested" + "auto-rejecting" in the
/// combined stdout/stderr tail output.
#[allow(dead_code)] // Legacy compatibility API; new flows use classify_permission_denial.
pub fn detect_permission_reject(
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> Option<PermissionReject> {
    classify_permission_denial(&[stdout_tail, stderr_tail]).and_then(|denial| {
        (denial.category == PermissionDenialCategory::AutoReject)
            .then(|| PermissionReject::new(denial.denied_target))
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

    #[test]
    fn test_classifies_file_read_permission_denied() {
        let result = classify_permission_denial(&[Some(
            "Error: Read permission denied for /private/config.json",
        )])
        .expect("file read denial should match");

        assert_eq!(result.category, PermissionDenialCategory::FileRead);
        assert_eq!(result.denied_target, "/private/config.json");
    }

    #[test]
    fn test_classifies_tool_access_denied() {
        let result = classify_permission_denial(&[Some("Tool access denied: Bash")])
            .expect("tool denial should match");

        assert_eq!(result.category, PermissionDenialCategory::ToolAccess);
        assert_eq!(result.denied_target, "Bash");
    }

    #[test]
    fn test_classifies_command_level_harness_policy_rejection() {
        let result = classify_permission_denial(&[Some(
            "harness policy rejected command: git push origin main",
        )])
        .expect("command policy denial should match");

        assert_eq!(result.category, PermissionDenialCategory::CommandPolicy);
        assert_eq!(result.denied_target, "command: git push origin main");
    }

    #[test]
    fn test_non_permission_failure_does_not_match() {
        let result = classify_permission_denial(&[Some(
            "cargo test failed: assertion failed in parser unit test",
        )]);

        assert!(result.is_none());
    }

    #[test]
    fn test_permission_denial_tracker_first_changed_and_progressing_are_not_stalled() {
        let mut tracker = PermissionDenialTracker::new();
        let first = classify_permission_denial(&[Some("Tool access denied: Bash")]).unwrap();
        let changed = classify_permission_denial(&[Some("Tool access denied: Read")]).unwrap();

        assert!(!tracker.observe(&first, false).stalled);
        assert!(!tracker.observe(&changed, false).stalled);
        assert!(!tracker.observe(&changed, true).stalled);
    }

    #[test]
    fn test_permission_denial_tracker_repeated_without_progress_stalls() {
        let mut tracker = PermissionDenialTracker::new();
        let denial = classify_permission_denial(&[Some("Tool access denied: Bash")]).unwrap();

        assert!(!tracker.observe(&denial, false).stalled);
        assert!(tracker.observe(&denial, false).stalled);
    }
}
