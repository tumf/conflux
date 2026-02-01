//! Prompt building functions for agent commands.

use crate::config::defaults::ACCEPTANCE_SYSTEM_PROMPT;

/// Legacy hardcoded system prompt for apply commands.
/// Kept only for compatibility in tests; actual prompt is sourced from OpenCode command files.
pub const APPLY_SYSTEM_PROMPT: &str = "";

/// Build apply prompt from user prompt, history context, and acceptance tail
/// Format: user_prompt + APPLY_SYSTEM_PROMPT + acceptance_tail_context + history_context
///
/// # Arguments
///
/// * `user_prompt` - User-customizable apply prompt
/// * `history_context` - Previous apply attempts context
/// * `acceptance_tail_context` - Acceptance output tail context (optional)
///
/// # Note
///
/// The acceptance_tail_context should be built using `build_last_acceptance_output_context`
/// and should only be provided for the first apply attempt after acceptance failure.
pub fn build_apply_prompt(
    user_prompt: &str,
    history_context: &str,
    acceptance_tail_context: &str,
) -> String {
    let mut parts = Vec::new();

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    // System prompt is always included
    parts.push(APPLY_SYSTEM_PROMPT.to_string());

    // Acceptance tail context (if acceptance failed and this is the first apply retry)
    if !acceptance_tail_context.is_empty() {
        parts.push(acceptance_tail_context.to_string());
    }

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
/// 2. diff_context (if not empty) - changed files context for all acceptance attempts
/// 3. last_output_context (if not empty) - previous acceptance stdout/stderr tail for 2nd+ attempts
/// 4. user_prompt (if not empty)
/// 5. history_context (if not empty)
pub fn build_acceptance_prompt(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    last_output_context: &str,
    diff_context: &str,
) -> String {
    let mut parts = Vec::new();

    // System prompt is always included first, with change_id expanded
    let system_prompt = ACCEPTANCE_SYSTEM_PROMPT.replace("{change_id}", change_id);
    parts.push(system_prompt);

    // Diff context comes right after system prompt
    if !diff_context.is_empty() {
        parts.push(diff_context.to_string());
    }

    if !last_output_context.is_empty() {
        parts.push(last_output_context.to_string());
    }

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build acceptance prompt context without the hardcoded system prompt.
///
/// Use this when the fixed acceptance instructions live in the OpenCode command template
/// and the orchestrator should only inject variable context via `{prompt}`.
pub fn build_acceptance_prompt_context_only(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    last_output_context: &str,
    diff_context: &str,
) -> String {
    let mut parts = Vec::new();

    // Change metadata first so downstream templates can reference it.
    parts.push(format!("change_id: {}", change_id));
    parts.push(format!(
        "proposal_path: openspec/changes/{}/proposal.md\n\
tasks_path: openspec/changes/{}/tasks.md\n\
spec_deltas_path: openspec/changes/{}/specs/",
        change_id, change_id, change_id
    ));

    if !diff_context.is_empty() {
        parts.push(diff_context.to_string());
    }

    if !last_output_context.is_empty() {
        parts.push(last_output_context.to_string());
    }

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build diff context for acceptance attempts.
///
/// Returns formatted context with changed files and previous findings.
/// Used for all acceptance attempts (1st shows base→current, 2nd+ shows last→current).
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

/// Build last acceptance output context for 2nd+ acceptance attempts.
///
/// Returns formatted context with stdout/stderr tail from the previous acceptance attempt.
/// This allows the agent to see what was investigated in the previous acceptance run.
pub fn build_last_acceptance_output_context(
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> String {
    // If both are empty, return empty string
    if stdout_tail.is_none() && stderr_tail.is_none() {
        return String::new();
    }

    let mut lines = vec!["<last_acceptance_output>".to_string()];
    lines.push(
        "Previous acceptance investigation output (for context - avoid repeating the same checks):"
            .to_string(),
    );
    lines.push(String::new());

    if let Some(stdout) = stdout_tail {
        if !stdout.trim().is_empty() {
            lines.push("stdout:".to_string());
            lines.push(stdout.to_string());
            lines.push(String::new());
        }
    }

    if let Some(stderr) = stderr_tail {
        if !stderr.trim().is_empty() {
            lines.push("stderr:".to_string());
            lines.push(stderr.to_string());
            lines.push(String::new());
        }
    }

    lines.push("</last_acceptance_output>".to_string());

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

    #[test]
    fn test_build_acceptance_prompt_insertion_order() {
        // Test that the prompt components are inserted in the correct order:
        // 1. system_prompt
        // 2. diff_context
        // 3. last_output_context
        // 4. user_prompt
        // 5. history_context

        let change_id = "test-change";
        let user_prompt = "USER_PROMPT_MARKER";
        let history_context = "HISTORY_CONTEXT_MARKER";
        let last_output_context =
            "<last_acceptance_output>\nLAST_OUTPUT_MARKER\n</last_acceptance_output>";
        let diff_context =
            "<acceptance_diff_context>\nDIFF_CONTEXT_MARKER\n</acceptance_diff_context>";

        let result = build_acceptance_prompt(
            change_id,
            user_prompt,
            history_context,
            last_output_context,
            diff_context,
        );

        // Find positions of each marker
        let system_pos = result
            .find("You are reviewing the implementation")
            .expect("System prompt should be present");
        let diff_pos = result
            .find("DIFF_CONTEXT_MARKER")
            .expect("Diff context should be present");
        let last_output_pos = result
            .find("LAST_OUTPUT_MARKER")
            .expect("Last output context should be present");
        let user_pos = result
            .find("USER_PROMPT_MARKER")
            .expect("User prompt should be present");
        let history_pos = result
            .find("HISTORY_CONTEXT_MARKER")
            .expect("History context should be present");

        // Verify order: system < diff < last_output < user < history
        assert!(
            system_pos < diff_pos,
            "System prompt should come before diff context"
        );
        assert!(
            diff_pos < last_output_pos,
            "Diff context should come before last output context"
        );
        assert!(
            last_output_pos < user_pos,
            "Last output context should come before user prompt"
        );
        assert!(
            user_pos < history_pos,
            "User prompt should come before history context"
        );
    }

    #[test]
    fn test_build_acceptance_prompt_empty_diff_context() {
        // Test that empty diff context is correctly omitted
        let change_id = "test-change";
        let user_prompt = "USER_PROMPT";
        let history_context = "";
        let last_output_context = "";
        let diff_context = ""; // Empty diff context

        let result = build_acceptance_prompt(
            change_id,
            user_prompt,
            history_context,
            last_output_context,
            diff_context,
        );

        // Should contain system prompt and user prompt
        assert!(result.contains("You are reviewing the implementation"));
        assert!(result.contains("USER_PROMPT"));

        // Should NOT contain diff context section with actual content
        // (Note: The ACCEPTANCE_SYSTEM_PROMPT mentions <acceptance_diff_context> in instructions,
        // but we should not have a separate block with actual file listings)
        assert!(!result.contains("Files changed since last acceptance check:"));
        assert!(!result.contains("Previous acceptance findings:"));
    }
}
