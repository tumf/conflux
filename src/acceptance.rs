//! Acceptance test output parsing module.
//!
//! This module provides functions to parse acceptance test output
//! and determine pass/fail status with findings.
//!
//! Verdict contract (post `adopt-json-acceptance-verdict`):
//!
//! - **Primary**: a strict JSON verdict object of the form
//!   `{"acceptance":"pass|fail|continue|gated","findings":[...]}` emitted as
//!   the final machine-readable verdict payload. JSON verdicts may appear
//!   directly as a line on stdout, or wrapped inside a supported JSONL event
//!   text payload. In either case the runtime unwraps the payload and evaluates
//!   the JSON verdict.
//! - **Fallback**: legacy plain-text standalone verdict markers of the form
//!   `ACCEPTANCE: PASS|FAIL|CONTINUE|GATED` remain supported for backward
//!   compatibility, but JSON takes priority whenever both are present.

/// Result of parsing acceptance output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed
    Pass,
    /// Acceptance failed with findings
    Fail { findings: Vec<String> },
    /// Acceptance requires more investigation - continue later
    Continue,
    /// Acceptance gated due to implementation blocker
    Gated,
}

/// Canonical plain-text verdict variants. These remain supported as a
/// backward-compatible fallback when no JSON verdict object is present.
///
/// The runtime requires an unwrapped standalone line that exactly equals one
/// of these markers (after stripping tolerated markdown decorations).
/// Trailing-text concatenation such as `ACCEPTANCE: PASSAll ...` or
/// `ACCEPTANCE: PASS## ...` does NOT satisfy the fallback contract and falls
/// through to the CONTINUE default.
pub(crate) const CANONICAL_VERDICTS: &[(&str, &str)] = &[
    ("ACCEPTANCE: PASS", "pass"),
    ("ACCEPTANCE: FAIL", "fail"),
    ("ACCEPTANCE: CONTINUE", "continue"),
    ("ACCEPTANCE: GATED", "gated"),
    // Legacy backward-compatibility marker during migration.
    ("ACCEPTANCE: BLOCKED", "gated"),
];

/// Attempt to parse a single line/text payload as a strict JSON acceptance
/// verdict object. Returns `Some((kind, findings))` when the trimmed content is
/// a JSON object with an `acceptance` field equal to one of
/// `pass`/`fail`/`continue`/`gated` (case-insensitive).
/// Legacy `blocked` input is accepted as backward-compatible alias.
///
/// Accepted shapes:
///
/// ```json
/// {"acceptance":"pass"}
/// {"acceptance":"fail","findings":["src/foo.rs:10 issue"]}
/// ```
pub(crate) fn parse_json_verdict(text: &str) -> Option<(&'static str, Vec<String>)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let obj = value.as_object()?;
    let raw_kind = obj.get("acceptance")?.as_str()?;
    let kind = match raw_kind.trim().to_ascii_lowercase().as_str() {
        "pass" => "pass",
        "fail" => "fail",
        "continue" => "continue",
        "gated" => "gated",
        "blocked" => "gated",
        _ => return None,
    };
    let findings = obj
        .get("findings")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((kind, findings))
}

/// Detect a canonical verdict kind in a single output line.
///
/// Preference order:
///
/// 1. Strict JSON verdict object on the line itself (primary contract).
/// 2. Strict JSON verdict object inside an unwrapped supported agent JSONL
///    event text payload.
/// 3. Legacy plain-text standalone canonical marker on the line itself, or
///    inside the unwrapped event payload (backward-compatible fallback).
///
/// Returns `None` when no verdict is recognized. This helper is used both for
/// streaming verdict detection (grace-period trigger) and for full-output
/// parsing fallback.
pub fn detect_verdict_in_line(line: &str) -> Option<&'static str> {
    if let Some((kind, _)) = parse_json_verdict(line) {
        return Some(kind);
    }
    if let Some(text) = crate::stream_json_textifier::extract_text_from_stream_json(line) {
        for inner in text.lines() {
            if let Some((kind, _)) = parse_json_verdict(inner) {
                return Some(kind);
            }
        }
        for inner in text.lines() {
            if let Some(kind) = canonical_verdict_kind(inner) {
                return Some(kind);
            }
        }
        return None;
    }
    canonical_verdict_kind(line)
}

/// Returns the canonical verdict kind for a single output line, or `None` when
/// the line is not a standalone canonical verdict.
///
/// Matching is strict: the line must equal one of [`CANONICAL_VERDICTS`] markers
/// exactly after trimming whitespace and stripping defensively-tolerated markdown
/// decorations (bold/italic/underline + leading heading/blockquote/bullet
/// prefixes). Trailing-text concatenation onto the marker is rejected.
pub(crate) fn canonical_verdict_kind(line: &str) -> Option<&'static str> {
    let normalized = strip_markdown_decorations(line.trim());
    CANONICAL_VERDICTS
        .iter()
        .find(|(marker, _)| normalized == *marker)
        .map(|(_, kind)| *kind)
}

/// Parse acceptance output text and determine pass/fail/continue/gated status.
///
/// Contract (JSON-primary, text-fallback):
///
/// - Primary: a strict JSON verdict object
///   `{"acceptance":"pass|fail|continue|gated","findings":[...]}` emitted
///   either directly as a line, or wrapped inside a supported JSONL event text
///   payload unwrapped by the runtime.
///   The first JSON verdict encountered wins, regardless of any earlier text
///   marker.
/// - Fallback: a standalone legacy line equal to one of `ACCEPTANCE: PASS`,
///   `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, `ACCEPTANCE: GATED`,
///   or (legacy-compatibility) `ACCEPTANCE: BLOCKED`.
///   Bold/italic/underline wrappers and leading heading/blockquote/bullet
///   prefixes are tolerated defensively, but trailing-text concatenation onto
///   the marker (for example `ACCEPTANCE: PASSAll ...` or
///   `ACCEPTANCE: PASS## ...`) is NOT a canonical marker. When no JSON verdict
///   is found, the first canonical fallback line wins.
///
/// If neither is observed, the result defaults to [`AcceptanceResult::Continue`]
/// so the acceptance loop can retry.
pub fn parse_acceptance_output(output: &str) -> AcceptanceResult {
    let mut fallback_kind: Option<&'static str> = None;
    let mut in_code_block = false;

    for raw_line in output.lines() {
        let trimmed = raw_line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            continue;
        }

        // Collect scan candidates: the raw line, plus unwrapped event text when
        // the line is a supported JSONL event wrapper from an agent runtime.
        let mut candidates: Vec<String> = vec![trimmed.to_string()];
        if let Some(text) = crate::stream_json_textifier::extract_text_from_stream_json(trimmed) {
            for inner in text.lines() {
                candidates.push(inner.to_string());
            }
        }

        for candidate in &candidates {
            let cand = candidate.trim();
            if let Some((kind, findings)) = parse_json_verdict(cand) {
                return match kind {
                    "pass" => AcceptanceResult::Pass,
                    "fail" => AcceptanceResult::Fail { findings },
                    "continue" => AcceptanceResult::Continue,
                    "gated" | "blocked" => AcceptanceResult::Gated,
                    _ => AcceptanceResult::Continue,
                };
            }
            if fallback_kind.is_none() {
                if let Some(kind) = canonical_verdict_kind(cand) {
                    fallback_kind = Some(kind);
                }
            }
        }
    }

    match fallback_kind {
        Some("pass") => AcceptanceResult::Pass,
        Some("continue") => AcceptanceResult::Continue,
        Some("gated") => AcceptanceResult::Gated,
        Some("fail") => {
            let findings = parse_findings(output);
            AcceptanceResult::Fail { findings }
        }
        _ => AcceptanceResult::Continue,
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

    // Trailing-text concatenation is NOT canonical. The runtime parser
    // rejects malformed verdicts and falls through to CONTINUE so the
    // acceptance loop can retry instead of locking in a bogus PASS.

    #[test]
    fn test_parse_pass_with_trailing_text_is_not_canonical() {
        // Regression: real log produced "ACCEPTANCE: PASSAll acceptance criteria verified:"
        // which previously satisfied PASS via starts_with. Strict canonical
        // matching rejects it.
        let output = "ACCEPTANCE: PASSAll acceptance criteria verified:\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_trailing_heading_is_not_canonical() {
        // Real log: "ACCEPTANCE: PASS## Acceptance Review Summary"
        let output = "ACCEPTANCE: PASS## Acceptance Review Summary\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_trailing_text_in_context_is_not_canonical() {
        let output = r#"Some prior output
ACCEPTANCE: PASSAll acceptance criteria verified:

1. Git working tree is clean
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_trailing_text_falls_through_to_canonical_pass() {
        // If a malformed verdict appears first but a canonical standalone
        // verdict follows on a later line, the canonical line wins.
        let output = "ACCEPTANCE: PASSAll bad form\nACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: FAILSome additional context\nFINDINGS:\n- Issue 1\n";
        // Trailing-text FAIL is malformed; falls through to CONTINUE.
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_continue_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: CONTINUENeeds further investigation\n";
        // Malformed verdict — falls through to CONTINUE default (same kind by
        // coincidence, but still via fallback rather than canonical match).
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_blocked_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: BLOCKEDWaiting for dependency\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_passed_word_boundary_is_not_canonical() {
        // "ACCEPTANCE: PASSED" must not match canonical PASS.
        let output = "ACCEPTANCE: PASSED\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_canonical_verdict_kind_strict_match() {
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(canonical_verdict_kind("**ACCEPTANCE: PASS**"), Some("pass"));
        assert_eq!(canonical_verdict_kind("## ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(canonical_verdict_kind("> ACCEPTANCE: FAIL"), Some("fail"));
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASSAll bad"), None);
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASS## heading"), None);
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASSED"), None);
        assert_eq!(canonical_verdict_kind("not a verdict"), None);
    }

    #[test]
    fn test_parse_blocked() {
        let output = "ACCEPTANCE: BLOCKED\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Gated);
    }

    #[test]
    fn test_parse_blocked_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: BLOCKED\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Gated);
    }

    #[test]
    fn test_parse_blocked_with_bold_decoration() {
        let output = "**ACCEPTANCE: BLOCKED**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Gated);
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
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Gated);
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
        // Streaming verdict detection (src/orchestration/acceptance.rs and
        // src/parallel/executor.rs) and the parser MUST agree on the canonical
        // contract: standalone exact match after stripping tolerated markdown
        // decorations. This test pins both surfaces to canonical_verdict_kind.
        let drift_cases: &[(&str, &str)] = &[
            ("ACCEPTANCE: PASS", "pass"),
            ("**ACCEPTANCE: PASS**", "pass"),
            ("## ACCEPTANCE: PASS", "pass"),
            ("> ACCEPTANCE: PASS", "pass"),
            ("- ACCEPTANCE: PASS", "pass"),
            ("### **ACCEPTANCE: FAIL**", "fail"),
            ("ACCEPTANCE: CONTINUE", "continue"),
            ("ACCEPTANCE: GATED", "gated"),
            // Legacy compatibility during migration.
            ("ACCEPTANCE: BLOCKED", "gated"),
            ("## ACCEPTANCE: BLOCKED", "gated"),
            ("> ACCEPTANCE: FAIL", "fail"),
        ];

        for (case, expected_kind) in drift_cases {
            assert_eq!(
                canonical_verdict_kind(case),
                Some(*expected_kind),
                "canonical_verdict_kind must detect '{}' as kind '{}'",
                case,
                expected_kind
            );

            let full_output = format!("{}\n", case);
            let result = parse_acceptance_output(&full_output);
            let result_kind = match &result {
                AcceptanceResult::Pass => "pass",
                AcceptanceResult::Fail { .. } => "fail",
                AcceptanceResult::Continue => "continue",
                AcceptanceResult::Gated => "gated",
            };
            assert_eq!(
                result_kind, *expected_kind,
                "parse_acceptance_output returned '{}' but expected '{}' for input '{}'",
                result_kind, expected_kind, case
            );
        }
    }

    #[test]
    fn test_marker_detection_rejects_trailing_text_uniformly() {
        // Both surfaces (parser + streaming detection) must reject trailing-text
        // verdicts so they cannot accidentally finalize an acceptance run early.
        let malformed: &[&str] = &[
            "ACCEPTANCE: PASSAll checks completed",
            "ACCEPTANCE: PASS## Acceptance Review Summary",
            "ACCEPTANCE: PASSED",
            "ACCEPTANCE: FAILSome explanation",
            "ACCEPTANCE: CONTINUEMore work",
            "ACCEPTANCE: BLOCKEDWaiting",
        ];

        for case in malformed {
            assert!(
                canonical_verdict_kind(case).is_none(),
                "canonical_verdict_kind must NOT detect malformed verdict '{}'",
                case
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

    // --- JSON-primary verdict contract tests ---

    #[test]
    fn test_parse_json_verdict_pass() {
        let line = r#"{"acceptance":"pass"}"#;
        assert_eq!(
            parse_json_verdict(line),
            Some(("pass", Vec::<String>::new()))
        );
    }

    #[test]
    fn test_parse_json_verdict_fail_with_findings() {
        let line = r#"{"acceptance":"fail","findings":["src/a.rs:1 bad","src/b.rs:2 worse"]}"#;
        let (kind, findings) = parse_json_verdict(line).expect("strict JSON verdict");
        assert_eq!(kind, "fail");
        assert_eq!(findings, vec!["src/a.rs:1 bad", "src/b.rs:2 worse"]);
    }

    #[test]
    fn test_parse_json_verdict_continue_gated_and_legacy_blocked() {
        assert_eq!(
            parse_json_verdict(r#"{"acceptance":"continue"}"#),
            Some(("continue", Vec::<String>::new()))
        );
        assert_eq!(
            parse_json_verdict(r#"{"acceptance":"gated"}"#),
            Some(("gated", Vec::<String>::new()))
        );
        assert_eq!(
            parse_json_verdict(r#"{"acceptance":"blocked"}"#),
            Some(("gated", Vec::<String>::new()))
        );
    }

    #[test]
    fn test_parse_json_verdict_case_insensitive_value() {
        assert_eq!(
            parse_json_verdict(r#"{"acceptance":"PASS"}"#),
            Some(("pass", Vec::<String>::new()))
        );
        assert_eq!(
            parse_json_verdict(r#"{"acceptance":"Fail"}"#),
            Some(("fail", Vec::<String>::new()))
        );
    }

    #[test]
    fn test_parse_json_verdict_rejects_non_object_and_unknown_kind() {
        assert_eq!(parse_json_verdict("pass"), None);
        assert_eq!(parse_json_verdict(r#"["pass"]"#), None);
        assert_eq!(parse_json_verdict(r#"{"acceptance":"maybe"}"#), None);
        assert_eq!(parse_json_verdict(r#"{"other":"pass"}"#), None);
        assert_eq!(parse_json_verdict("not json"), None);
        assert_eq!(parse_json_verdict(""), None);
    }

    #[test]
    fn test_parse_acceptance_output_json_pass_single_line() {
        // Primary contract: the final machine-readable verdict is a strict JSON
        // object emitted as its own stdout line.
        let output = r#"{"acceptance":"pass"}
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_json_fail_findings_preferred_over_text_section() {
        // JSON findings array is the source of truth when JSON verdict wins.
        let output = r#"preamble
{"acceptance":"fail","findings":["x","y"]}
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["x", "y"]);
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_acceptance_output_json_beats_text_fallback_regardless_of_order() {
        // JSON-primary: when both a text marker and a JSON verdict appear, the
        // JSON verdict wins even if the text marker came first.
        let output = "ACCEPTANCE: CONTINUE\n{\"acceptance\":\"pass\"}\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_text_fallback_when_no_json() {
        // Backward compatibility: absent a JSON verdict, the legacy standalone
        // text marker remains accepted as canonical.
        let output = "ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_agent_assistant_event() {
        // Some agent JSONL formats wrap final text in an assistant event.
        // The parser must unwrap the text payload and still find the JSON verdict.
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"{\"acceptance\":\"pass\"}"}]}}"#;
        let output = format!("{}\n", event);
        assert_eq!(parse_acceptance_output(&output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_agent_result_event() {
        // Some agent JSONL formats carry the assistant's final text in `result`.
        // Parser must unwrap and accept the JSON verdict.
        let event = r#"{"type":"result","subtype":"success","result":"{\"acceptance\":\"fail\",\"findings\":[\"a\"]}","is_error":false}"#;
        let output = format!("{}\n", event);
        match parse_acceptance_output(&output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["a".to_string()]);
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_codex_item_completed_event() {
        let event = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"{\"acceptance\":\"pass\"}"}}"#;
        let output = format!("{}\n", event);
        assert_eq!(parse_acceptance_output(&output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_detect_verdict_in_line_json_direct() {
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"pass"}"#),
            Some("pass")
        );
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"gated"}"#),
            Some("gated")
        );
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"blocked"}"#),
            Some("gated")
        );
    }

    #[test]
    fn test_detect_verdict_in_line_json_inside_assistant_event() {
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"{\"acceptance\":\"pass\"}"}]}}"#;
        assert_eq!(detect_verdict_in_line(event), Some("pass"));
    }

    #[test]
    fn test_detect_verdict_in_line_text_fallback() {
        assert_eq!(detect_verdict_in_line("ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(detect_verdict_in_line("**ACCEPTANCE: FAIL**"), Some("fail"));
        assert_eq!(
            detect_verdict_in_line("ACCEPTANCE: PASSAll checks done"),
            None,
            "trailing-text PASS must not satisfy even the text fallback"
        );
    }

    #[test]
    fn test_detect_verdict_in_line_text_inside_assistant_event() {
        // When the agent produces the legacy text marker but wrapped in a
        // stream-json assistant event, the detector MUST still recognize the
        // fallback so acceptance does not stall waiting for JSON.
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ACCEPTANCE: PASS"}]}}"#;
        assert_eq!(detect_verdict_in_line(event), Some("pass"));
    }

    #[test]
    fn test_detect_verdict_in_line_unrelated_events_return_none() {
        assert_eq!(
            detect_verdict_in_line(r#"{"type":"system","subtype":"init"}"#),
            None
        );
        assert_eq!(detect_verdict_in_line("plain log line"), None);
        assert_eq!(detect_verdict_in_line(""), None);
    }

    /// Regression for the malformed trailing-text real-log case
    /// (`ACCEPTANCE: PASS# ...`): with the legacy text-only contract, the
    /// parser returned CONTINUE and the acceptance loop retried. Under the new
    /// JSON-primary contract the agent emits a strict JSON verdict on a later
    /// line, and that JSON verdict MUST win regardless of the earlier drift.
    #[test]
    fn test_regression_malformed_text_then_json_pass() {
        let output = "ACCEPTANCE: PASS# Acceptance Review Summary\n{\"acceptance\":\"pass\"}\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }
}
