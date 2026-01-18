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
    /// Acceptance requires more investigation - continue later
    Continue,
}

/// Parse acceptance output text and determine pass/fail/continue status.
///
/// Expected format:
/// - PASS: `ACCEPTANCE: PASS` (with optional markdown decorations like `**ACCEPTANCE: PASS**`)
/// - FAIL: `ACCEPTANCE: FAIL` followed by `FINDINGS:` and items prefixed with `- `
/// - CONTINUE: `ACCEPTANCE: CONTINUE`
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
///
/// let continue_output = "ACCEPTANCE: CONTINUE\n";
/// assert_eq!(parse_acceptance_output(continue_output), AcceptanceResult::Continue);
/// ```
pub fn parse_acceptance_output(output: &str) -> AcceptanceResult {
    let lines: Vec<&str> = output.lines().collect();

    // Look for ACCEPTANCE: PASS, ACCEPTANCE: FAIL, or ACCEPTANCE: CONTINUE
    let mut acceptance_status = None;
    for line in &lines {
        let trimmed = line.trim();
        // Strip markdown decorations (**, *, _, etc.) before matching
        let normalized = strip_markdown_decorations(trimmed);
        if normalized == "ACCEPTANCE: PASS" {
            acceptance_status = Some("pass");
            break;
        } else if normalized == "ACCEPTANCE: FAIL" {
            acceptance_status = Some("fail");
            break;
        } else if normalized == "ACCEPTANCE: CONTINUE" {
            acceptance_status = Some("continue");
            break;
        }
    }

    match acceptance_status {
        Some("pass") => AcceptanceResult::Pass,
        Some("continue") => AcceptanceResult::Continue,
        Some("fail") => {
            // Parse findings
            let findings = parse_findings(output);
            AcceptanceResult::Fail { findings }
        }
        _ => {
            // Default to fail if no explicit status found
            AcceptanceResult::Fail {
                findings: vec![
                    "No explicit ACCEPTANCE: PASS, ACCEPTANCE: FAIL, or ACCEPTANCE: CONTINUE found in output".to_string(),
                ],
            }
        }
    }
}

/// Strip markdown decorations from a string.
/// Removes bold (**), italic (*), underline (_), and other common markdown formatting.
fn strip_markdown_decorations(text: &str) -> String {
    // Simple approach: remove all common markdown decoration characters
    text.replace("**", "")
        .replace(['*', '_'], "")
        .trim()
        .to_string()
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

    #[test]
    fn test_parse_continue() {
        let output = "ACCEPTANCE: CONTINUE\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_continue_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: CONTINUE\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_bold_decoration() {
        let output = "**ACCEPTANCE: PASS**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_bold_decoration_and_extra_output() {
        let output = "Some debug output\n**ACCEPTANCE: PASS**\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_bold_decoration() {
        let output = "**ACCEPTANCE: FAIL**\nFINDINGS:\n- Issue 1\n- Issue 2\n";
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
    fn test_parse_continue_with_bold_decoration() {
        let output = "**ACCEPTANCE: CONTINUE**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_italic_decoration() {
        let output = "*ACCEPTANCE: PASS*\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_mixed_decorations() {
        let output = "**_ACCEPTANCE: PASS_**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_strip_markdown_decorations() {
        assert_eq!(
            strip_markdown_decorations("**ACCEPTANCE: PASS**"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("*ACCEPTANCE: PASS*"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("_ACCEPTANCE: PASS_"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("**_ACCEPTANCE: PASS_**"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }
}
