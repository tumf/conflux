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
    /// Acceptance blocked due to implementation blocker
    Blocked,
}

/// Parse acceptance output text and determine pass/fail/continue/blocked status.
///
/// Expected format:
/// - PASS: `ACCEPTANCE: PASS` (with optional markdown decorations like `**ACCEPTANCE: PASS**`)
/// - FAIL: `ACCEPTANCE: FAIL` followed by `FINDINGS:` and items prefixed with `- `
/// - CONTINUE: `ACCEPTANCE: CONTINUE`
/// - BLOCKED: `ACCEPTANCE: BLOCKED`
///
/// # Examples
///
/// ```ignore
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
///
/// let blocked_output = "ACCEPTANCE: BLOCKED\n";
/// assert_eq!(parse_acceptance_output(blocked_output), AcceptanceResult::Blocked);
/// ```
pub fn parse_acceptance_output(output: &str) -> AcceptanceResult {
    let lines: Vec<&str> = output.lines().collect();

    // Look for ACCEPTANCE: PASS, ACCEPTANCE: FAIL, ACCEPTANCE: CONTINUE, or ACCEPTANCE: BLOCKED
    // Skip code blocks (delimited by ```) to avoid matching examples in prompts
    let mut acceptance_status = None;
    let mut in_code_block = false;

    for line in &lines {
        let trimmed = line.trim();

        // Toggle code block state on triple backticks
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        // Skip lines inside code blocks
        if in_code_block {
            continue;
        }

        // Strip markdown decorations (**, *, _, etc.) before matching
        let normalized = strip_markdown_decorations(trimmed);
        // Use starts_with instead of exact match to handle cases where
        // the AI agent output concatenates text after the status keyword
        // without a newline (e.g., "ACCEPTANCE: PASSAll criteria verified").
        if normalized.starts_with("ACCEPTANCE: PASS") {
            acceptance_status = Some("pass");
            break;
        } else if normalized.starts_with("ACCEPTANCE: FAIL") {
            acceptance_status = Some("fail");
            break;
        } else if normalized.starts_with("ACCEPTANCE: CONTINUE") {
            acceptance_status = Some("continue");
            break;
        } else if normalized.starts_with("ACCEPTANCE: BLOCKED") {
            acceptance_status = Some("blocked");
            break;
        }
    }

    match acceptance_status {
        Some("pass") => AcceptanceResult::Pass,
        Some("continue") => AcceptanceResult::Continue,
        Some("blocked") => AcceptanceResult::Blocked,
        Some("fail") => {
            // Parse findings
            let findings = parse_findings(output);
            AcceptanceResult::Fail { findings }
        }
        _ => {
            // Default to continue if no explicit status found
            // This allows the acceptance loop to retry and investigate further
            AcceptanceResult::Continue
        }
    }
}

/// Strip markdown decorations from a string.
/// Removes bold (**), italic (*), underline (_), heading prefixes (#),
/// blockquote prefixes (>), and bullet prefixes (-) that LLMs commonly
/// add around verdict markers.
///
/// This is a defensive measure — the canonical contract forbids these
/// wrappers, but the parser tolerates them to prevent silent CONTINUE
/// fallback when an agent drifts.
pub(crate) fn strip_markdown_decorations(text: &str) -> String {
    let mut s = text.to_string();
    // Remove bold pairs first to avoid leaving stray * characters
    s = s.replace("**", "");
    // Remove remaining italic/underline characters
    s = s.replace(['*', '_'], "");
    // Trim whitespace, then strip leading markdown block-level prefixes
    let trimmed = s.trim();
    // Strip leading heading markers (e.g., "## ", "### ")
    let trimmed = trimmed.trim_start_matches('#');
    // Strip leading blockquote markers (e.g., "> ")
    let trimmed = trimmed.trim_start_matches('>');
    // Strip leading bullet markers (e.g., "- ")
    let trimmed = trimmed.trim_start_matches('-');
    trimmed.trim().to_string()
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
        // When no explicit marker is present, default to CONTINUE
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_no_marker_defaults_to_continue() {
        // Empty output defaults to CONTINUE
        assert_eq!(parse_acceptance_output(""), AcceptanceResult::Continue);

        // Output with no acceptance marker defaults to CONTINUE
        let output = "Some debug output\nNo marker here\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);

        // Output with findings but no marker defaults to CONTINUE
        let output = "FINDINGS:\n- Issue 1\n- Issue 2\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
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

    #[test]
    fn test_parse_ignores_acceptance_in_code_blocks() {
        // Code block with FAIL should be ignored, actual PASS should be detected
        let output = r#"
Example output:
```
ACCEPTANCE: FAIL
FINDINGS:
- Issue 1
```

Actual result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_ignores_multiple_code_blocks() {
        // Multiple code blocks with different statuses should be ignored
        let output = r#"
First example:
```
ACCEPTANCE: FAIL
```

Second example:
```
ACCEPTANCE: CONTINUE
```

Actual result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_code_block_with_language_specifier() {
        // Code blocks with language specifier should also be ignored
        let output = r#"
Example:
```bash
ACCEPTANCE: FAIL
```

Result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_unclosed_code_block() {
        // If code block is not closed, everything after opening should be skipped
        let output = r#"
Example:
```
ACCEPTANCE: FAIL
ACCEPTANCE: PASS
"#;
        // Both are inside unclosed code block, default to Continue
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_trailing_text() {
        // Regression: AI agent output may concatenate text after the status
        // keyword without a newline, e.g. "ACCEPTANCE: PASSAll acceptance criteria verified:"
        let output = "ACCEPTANCE: PASSAll acceptance criteria verified:\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_trailing_text_in_context() {
        // Full context from real log output
        let output = r#"Some prior output
ACCEPTANCE: PASSAll acceptance criteria verified:

1. Git working tree is clean
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_trailing_text() {
        let output = "ACCEPTANCE: FAILSome additional context\nFINDINGS:\n- Issue 1\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
                assert_eq!(findings[0], "Issue 1");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_continue_with_trailing_text() {
        let output = "ACCEPTANCE: CONTINUENeeds further investigation\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_blocked_with_trailing_text() {
        let output = "ACCEPTANCE: BLOCKEDWaiting for dependency\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Blocked);
    }

    #[test]
    fn test_parse_blocked() {
        let output = "ACCEPTANCE: BLOCKED\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Blocked);
    }

    #[test]
    fn test_parse_blocked_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: BLOCKED\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Blocked);
    }

    #[test]
    fn test_parse_blocked_with_bold_decoration() {
        let output = "**ACCEPTANCE: BLOCKED**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Blocked);
    }

    // Characterization tests: document the exact contract that
    // src/orchestration/acceptance.rs relies on after the refactor.

    #[test]
    fn test_parse_fail_findings_excludes_preamble() {
        // parse_acceptance_output for FAIL extracts only items from the
        // FINDINGS section — preamble lines before the marker are NOT included.
        let output =
            "preamble line\nACCEPTANCE: FAIL\nFINDINGS:\n- Finding 1\n- Finding 2\npostamble";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["Finding 1", "Finding 2"]);
                assert!(!findings.iter().any(|f| f.contains("preamble")));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_findings_from_findings_section_only() {
        // Findings come exclusively from lines prefixed with "- " after
        // "FINDINGS:". This is the single authoritative source used in
        // AcceptanceResult::Fail after the refactor.
        let output =
            "ACCEPTANCE: FAIL\nFINDINGS:\n- src/foo.rs:10 missing test\n- src/bar.rs:5 dead code\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "src/foo.rs:10 missing test");
                assert_eq!(findings[1], "src/bar.rs:5 dead code");
            }
            _ => panic!("Expected Fail"),
        }
    }

    // --- Markdown drift tolerance tests ---
    // These tests verify that the parser defensively handles common LLM
    // formatting drift (headings, blockquotes, bullets) even though the
    // canonical contract forbids these wrappers.

    #[test]
    fn test_parse_pass_with_heading_prefix() {
        let output = "## ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_heading_h3_prefix() {
        let output = "### ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_heading_prefix() {
        let output = "## ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
                assert_eq!(findings[0], "Issue 1");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_pass_with_blockquote_prefix() {
        let output = "> ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_blockquote_prefix() {
        let output = "> ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_pass_with_bullet_prefix() {
        let output = "- ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_continue_with_heading_prefix() {
        let output = "## ACCEPTANCE: CONTINUE\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_blocked_with_heading_prefix() {
        let output = "## ACCEPTANCE: BLOCKED\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Blocked);
    }

    #[test]
    fn test_parse_pass_with_heading_and_bold() {
        // Combined drift: heading + bold
        let output = "## **ACCEPTANCE: PASS**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_strip_markdown_decorations_heading() {
        assert_eq!(
            strip_markdown_decorations("## ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_blockquote() {
        assert_eq!(
            strip_markdown_decorations("> ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_bullet() {
        assert_eq!(
            strip_markdown_decorations("- ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_heading_and_bold() {
        assert_eq!(
            strip_markdown_decorations("## **ACCEPTANCE: PASS**"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_parse_fail_empty_findings_when_no_section() {
        // When ACCEPTANCE: FAIL appears without a FINDINGS section,
        // parse_acceptance_output returns an empty findings vec.
        let output = "ACCEPTANCE: FAIL\nSome explanation without a FINDINGS: header\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert!(findings.is_empty());
            }
            _ => panic!("Expected Fail"),
        }
    }

    // --- Marker contract consistency tests ---
    // These tests verify that the marker detection used in orchestration/acceptance.rs
    // streaming code is consistent with the parser contract. The streaming code uses
    // strip_markdown_decorations + starts_with to detect markers for the grace period,
    // and the parser uses the same function. Both must agree.

    #[test]
    fn test_marker_detection_consistency_with_parser() {
        // The orchestration streaming code (src/orchestration/acceptance.rs) detects
        // markers using strip_markdown_decorations + starts_with for the grace period.
        // The parser (parse_acceptance_output) uses the same approach.
        // This test ensures both detect markers in all documented drift cases.
        let drift_cases: &[(&str, &str)] = &[
            ("ACCEPTANCE: PASS", "pass"),
            ("**ACCEPTANCE: PASS**", "pass"),
            ("## ACCEPTANCE: PASS", "pass"),
            ("> ACCEPTANCE: PASS", "pass"),
            ("- ACCEPTANCE: PASS", "pass"),
            ("### **ACCEPTANCE: FAIL**", "fail"),
            ("ACCEPTANCE: CONTINUE", "continue"),
            ("ACCEPTANCE: BLOCKED", "blocked"),
            ("## ACCEPTANCE: BLOCKED", "blocked"),
            ("> ACCEPTANCE: FAIL", "fail"),
        ];

        for (case, expected_kind) in drift_cases {
            // 1. Verify strip_markdown_decorations + starts_with detects the marker
            let normalized = strip_markdown_decorations(case.trim());
            let is_marker = normalized.starts_with("ACCEPTANCE: PASS")
                || normalized.starts_with("ACCEPTANCE: FAIL")
                || normalized.starts_with("ACCEPTANCE: CONTINUE")
                || normalized.starts_with("ACCEPTANCE: BLOCKED");
            assert!(
                is_marker,
                "strip_markdown_decorations + starts_with must detect marker in '{}' (normalized: '{}')",
                case, normalized
            );

            // 2. Verify the full parser returns the expected result kind (not a spurious fallback)
            let full_output = format!("{}\n", case);
            let result = parse_acceptance_output(&full_output);
            let result_kind = match &result {
                AcceptanceResult::Pass => "pass",
                AcceptanceResult::Fail { .. } => "fail",
                AcceptanceResult::Continue => "continue",
                AcceptanceResult::Blocked => "blocked",
            };
            assert_eq!(
                result_kind, *expected_kind,
                "parse_acceptance_output returned '{}' but expected '{}' for input '{}'",
                result_kind, expected_kind, case
            );
        }
    }

    #[test]
    fn test_code_fence_markers_rejected_by_both_parser_and_detection() {
        // Markers inside code fences must NOT be detected by either the parser
        // or the streaming marker detection (which operates line-by-line and
        // does not track code fence state, but the parser does).
        let output = "```\nACCEPTANCE: PASS\n```\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::Continue,
            "Parser must not match markers inside code fences"
        );
    }
}
