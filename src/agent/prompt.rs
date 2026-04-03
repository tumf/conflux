//! Prompt building functions for agent commands.

/// Legacy hardcoded system prompt for apply commands.
/// Kept only for compatibility in tests; actual prompt is sourced from OpenCode command files.
pub const APPLY_SYSTEM_PROMPT: &str = "";

/// Build apply prompt from change metadata, user prompt, history context, and acceptance tail
/// Format: fixed prelude + user_prompt + APPLY_SYSTEM_PROMPT + acceptance_tail_context + history_context
///
/// # Arguments
///
/// * `change_id` - Change identifier
/// * `user_prompt` - User-customizable apply prompt
/// * `history_context` - Previous apply attempts context
/// * `acceptance_tail_context` - Acceptance output tail context (optional)
///
/// # Note
///
/// The acceptance_tail_context should be built using `build_last_acceptance_output_context`
/// and should only be provided for the first apply attempt after acceptance failure.
pub fn build_apply_prompt(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    acceptance_tail_context: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push("load skills: cflx-workflow".to_string());
    parts.push(format!("Apply change id: {}", change_id));

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    parts.push(APPLY_SYSTEM_PROMPT.to_string());

    if !acceptance_tail_context.is_empty() {
        parts.push(acceptance_tail_context.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build archive prompt from change metadata, user prompt, and history context
/// Format: fixed prelude + user_prompt + history_context
pub fn build_archive_prompt(change_id: &str, user_prompt: &str, history_context: &str) -> String {
    let mut parts = Vec::new();

    parts.push("load skills: cflx-workflow".to_string());
    parts.push(format!("Archive change id: {}", change_id));

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
/// Now unified with context_only mode - no embedded system prompt.
/// All fixed instructions must come from the command template.
///
/// The prompt is constructed as:
/// 1. change metadata (change_id and paths)
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
    // Delegate to context_only implementation - "full" mode is now deprecated
    build_acceptance_prompt_context_only(
        change_id,
        user_prompt,
        history_context,
        last_output_context,
        diff_context,
    )
}

/// Repository-standard archive-readiness checks that acceptance must evaluate
/// before allowing archive to start.
const ARCHIVE_READINESS_CONTEXT: &str = "<archive_readiness_context>\n\
Before returning ACCEPTANCE: PASS, verify this workspace is ready for a real final archive commit under repository quality gates.\n\
Run and evaluate these gates (or documented equivalents if this repo differs):\n\
- pre-commit hook behavior for a normal commit (no --no-verify)\n\
- cargo fmt --check\n\
- cargo clippy -- -D warnings\n\
- cargo test\n\
If any gate fails, return a non-pass verdict and include actionable findings with:\n\
1) blocking gate name (hook/fmt/clippy/test),\n\
2) failing command,\n\
3) relevant file/path context when available.\n\
Do not defer these failures to archive.\n\
</archive_readiness_context>";

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

    parts.push("load skills: cflx-workflow".to_string());
    parts.push(format!("Acceptance id:{}", change_id));

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

    parts.push(ARCHIVE_READINESS_CONTEXT.to_string());

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
        // 1. change metadata (change_id, paths)
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
        let skill_pos = result
            .find("load skills: cflx-workflow")
            .expect("Skill prelude should be present");
        let acceptance_id_pos = result
            .find("Acceptance id:test-change")
            .expect("Acceptance id prelude should be present");
        let metadata_pos = result
            .find("change_id: test-change")
            .expect("Change metadata should be present");
        let diff_pos = result
            .find("DIFF_CONTEXT_MARKER")
            .expect("Diff context should be present");
        let readiness_pos = result
            .find("<archive_readiness_context>")
            .expect("Archive readiness context should be present");
        let last_output_pos = result
            .find("LAST_OUTPUT_MARKER")
            .expect("Last output context should be present");
        let user_pos = result
            .find("USER_PROMPT_MARKER")
            .expect("User prompt should be present");
        let history_pos = result
            .find("HISTORY_CONTEXT_MARKER")
            .expect("History context should be present");

        // Verify order: prelude < metadata < diff < readiness < last_output < user < history
        assert!(
            skill_pos < acceptance_id_pos,
            "Skill prelude should come before acceptance id"
        );
        assert!(
            acceptance_id_pos < metadata_pos,
            "Acceptance id should come before change metadata"
        );
        assert!(
            metadata_pos < diff_pos,
            "Change metadata should come before diff context"
        );
        assert!(
            diff_pos < readiness_pos,
            "Diff context should come before archive readiness context"
        );
        assert!(
            readiness_pos < last_output_pos,
            "Archive readiness context should come before last output context"
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

        // Should contain prelude, change metadata and user prompt
        assert!(result.contains("load skills: cflx-workflow"));
        assert!(result.contains("Acceptance id:test-change"));
        assert!(result.contains("change_id: test-change"));
        assert!(result.contains("proposal_path: openspec/changes/test-change/proposal.md"));
        assert!(result.contains("USER_PROMPT"));

        // Should NOT contain diff context section with actual content
        assert!(!result.contains("Files changed since last acceptance check:"));
        assert!(!result.contains("Previous acceptance findings:"));
    }
}
