//! Acceptance test output parsing module.
//!
//! This module provides functions to parse acceptance test output
//! and determine pass/fail status with findings.

/// Result of parsing acceptance output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed
    Pass,
    /// Acceptance failed with findings
    Fail { findings: Vec<String> },
}

/// Parse acceptance output text and determine pass/fail status.
///
/// Expected format:
/// - PASS: `ACCEPTANCE: PASS`
/// - FAIL: `ACCEPTANCE: FAIL` followed by `FINDINGS:` and items prefixed with `- `
///
/// # Examples
///
/// ```
/// use conflux::acceptance::{parse_acceptance_output, AcceptanceResult};
///
/// let pass_output = "ACCEPTANCE: PASS\n";
/// assert_eq!(parse_acceptance_output(pass_output), AcceptanceResult::Pass);
///
/// let fail_output = "ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n- Issue 2\n";
/// match parse_acceptance_output(fail_output) {
///     AcceptanceResult::Fail { findings } => {
///         assert_eq!(findings.len(), 2);
///         assert_eq!(findings[0], "Issue 1");
///         assert_eq!(findings[1], "Issue 2");
///     }
///     _ => panic!("Expected Fail"),
/// }
/// ```
pub fn parse_acceptance_output(output: &str) -> AcceptanceResult {
    let lines: Vec<&str> = output.lines().collect();

    // Look for ACCEPTANCE: PASS or ACCEPTANCE: FAIL
    let mut acceptance_status = None;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "ACCEPTANCE: PASS" {
            acceptance_status = Some(true);
            break;
        } else if trimmed == "ACCEPTANCE: FAIL" {
            acceptance_status = Some(false);
            break;
        }
    }

    match acceptance_status {
        Some(true) => AcceptanceResult::Pass,
        Some(false) => {
            // Parse findings
            let findings = parse_findings(output);
            AcceptanceResult::Fail { findings }
        }
        None => {
            // Default to fail if no explicit status found
            AcceptanceResult::Fail {
                findings: vec![
                    "No explicit ACCEPTANCE: PASS or ACCEPTANCE: FAIL found in output".to_string(),
                ],
            }
        }
    }
}

/// Parse findings from acceptance output.
/// Looks for lines starting with `- ` after a `FINDINGS:` header.
fn parse_findings(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let mut findings = Vec::new();
    let mut in_findings_section = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "FINDINGS:" {
            in_findings_section = true;
            continue;
        }

        if in_findings_section {
            if let Some(finding) = trimmed.strip_prefix("- ") {
                findings.push(finding.to_string());
            } else if !trimmed.is_empty() && !trimmed.starts_with('-') {
                // End of findings section if we encounter a non-finding line
                break;
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pass() {
        let output = "ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: PASS\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_findings() {
        let output = "ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n- Issue 2\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "Issue 1");
                assert_eq!(findings[1], "Issue 2");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_with_no_findings() {
        let output = "ACCEPTANCE: FAIL\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 0);
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_with_multiline_findings() {
        let output = r#"ACCEPTANCE: FAIL
FINDINGS:
- Task 1.3 is not completed
- Missing unit tests for new feature
- Code does not handle error case X
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 3);
                assert_eq!(findings[0], "Task 1.3 is not completed");
                assert_eq!(findings[1], "Missing unit tests for new feature");
                assert_eq!(findings[2], "Code does not handle error case X");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_no_status() {
        let output = "Some random output\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
                assert!(findings[0].contains("No explicit ACCEPTANCE"));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_findings_with_trailing_content() {
        let output = r#"ACCEPTANCE: FAIL
FINDINGS:
- Issue 1
- Issue 2

Additional output here
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "Issue 1");
                assert_eq!(findings[1], "Issue 2");
            }
            _ => panic!("Expected Fail"),
        }
    }
}
