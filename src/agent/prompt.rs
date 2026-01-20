//! Prompt building functions for agent commands.

use crate::config::defaults::ACCEPTANCE_SYSTEM_PROMPT;

/// Legacy hardcoded system prompt for apply commands.
/// Kept only for compatibility in tests; actual prompt is sourced from OpenCode command files.
pub const APPLY_SYSTEM_PROMPT: &str = "";

/// Build apply prompt from user prompt and history context
/// Format: user_prompt + APPLY_SYSTEM_PROMPT + history_context
pub fn build_apply_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    // System prompt is always included
    parts.push(APPLY_SYSTEM_PROMPT.to_string());

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build archive prompt from user prompt and history context
/// Format: user_prompt + history_context
pub fn build_archive_prompt(user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build acceptance prompt from user prompt and history context
///
/// The prompt is constructed as:
/// 1. ACCEPTANCE_SYSTEM_PROMPT with {change_id} expanded (always included)
/// 2. diff_context (if not empty) - for 2nd+ acceptance attempts
/// 3. user_prompt (if not empty)
/// 4. history_context (if not empty)
pub fn build_acceptance_prompt(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
) -> String {
    let mut parts = Vec::new();

    // System prompt is always included first, with change_id expanded
    let system_prompt = ACCEPTANCE_SYSTEM_PROMPT.replace("{change_id}", change_id);
    parts.push(system_prompt);

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build diff context for 2nd+ acceptance attempts.
///
/// Returns formatted context with changed files and previous findings.
pub fn build_acceptance_diff_context(
    changed_files: &[String],
    previous_findings: Option<&[String]>,
) -> String {
    let mut lines = vec!["<acceptance_diff_context>".to_string()];

    if !changed_files.is_empty() {
        lines.push("Files changed since last acceptance check:".to_string());
        for file in changed_files {
            lines.push(format!("- {}", file));
        }
        lines.push(String::new());
    }

    if let Some(findings) = previous_findings {
        if !findings.is_empty() {
            lines.push("Previous acceptance findings:".to_string());
            for finding in findings {
                lines.push(format!("- {}", finding));
            }
            lines.push(String::new());
        }
    }

    lines.push("Focus your verification on:".to_string());
    lines.push("1. Whether the changed files address the previous findings".to_string());
    lines.push("2. Whether the changes introduce new issues".to_string());
    lines.push("3. Read relevant files if needed to confirm the fixes".to_string());
    lines.push("</acceptance_diff_context>".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_acceptance_diff_context_with_files_and_findings() {
        let changed_files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
        let findings = vec![
            "Task 1.1 not completed".to_string(),
            "Missing integration test".to_string(),
        ];

        let context = build_acceptance_diff_context(&changed_files, Some(&findings));

        assert!(context.contains("<acceptance_diff_context>"));
        assert!(context.contains("Files changed since last acceptance check:"));
        assert!(context.contains("- src/main.rs"));
        assert!(context.contains("- src/lib.rs"));
        assert!(context.contains("Previous acceptance findings:"));
        assert!(context.contains("- Task 1.1 not completed"));
        assert!(context.contains("- Missing integration test"));
        assert!(context.contains("Focus your verification on:"));
        assert!(context.contains("</acceptance_diff_context>"));
    }

    #[test]
    fn test_build_acceptance_diff_context_only_files() {
        let changed_files = vec!["src/config.rs".to_string()];

        let context = build_acceptance_diff_context(&changed_files, None);

        assert!(context.contains("<acceptance_diff_context>"));
        assert!(context.contains("Files changed since last acceptance check:"));
        assert!(context.contains("- src/config.rs"));
        assert!(!context.contains("Previous acceptance findings:"));
        assert!(context.contains("Focus your verification on:"));
        assert!(context.contains("</acceptance_diff_context>"));
    }

    #[test]
    fn test_build_acceptance_diff_context_only_findings() {
        let findings = vec!["Fix missing imports".to_string()];

        let context = build_acceptance_diff_context(&[], Some(&findings));

        assert!(context.contains("<acceptance_diff_context>"));
        assert!(!context.contains("Files changed since last acceptance check:"));
        assert!(context.contains("Previous acceptance findings:"));
        assert!(context.contains("- Fix missing imports"));
        assert!(context.contains("Focus your verification on:"));
        assert!(context.contains("</acceptance_diff_context>"));
    }

    #[test]
    fn test_build_acceptance_diff_context_empty() {
        let context = build_acceptance_diff_context(&[], None);

        // Even with empty input, should still have the structure
        assert!(context.contains("<acceptance_diff_context>"));
        assert!(!context.contains("Files changed since last acceptance check:"));
        assert!(!context.contains("Previous acceptance findings:"));
        assert!(context.contains("Focus your verification on:"));
        assert!(context.contains("</acceptance_diff_context>"));
    }
}
