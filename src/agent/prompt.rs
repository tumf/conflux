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
#[allow(dead_code)]
pub fn build_apply_prompt(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    acceptance_tail_context: &str,
) -> String {
    build_apply_prompt_with_skill(
        crate::config::defaults::DEFAULT_APPLY_SKILL,
        change_id,
        user_prompt,
        history_context,
        acceptance_tail_context,
    )
}

pub fn build_apply_prompt_with_skill(
    apply_skill: &str,
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    acceptance_tail_context: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("load skills: {}", apply_skill));
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
#[allow(dead_code)]
pub fn build_archive_prompt(change_id: &str, user_prompt: &str, history_context: &str) -> String {
    build_archive_prompt_with_skill(
        crate::config::defaults::DEFAULT_ARCHIVE_SKILL,
        change_id,
        user_prompt,
        history_context,
    )
}

pub fn build_archive_prompt_with_skill(
    archive_skill: &str,
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("load skills: {}", archive_skill));
    parts.push(format!("Archive change id: {}", change_id));

    if !user_prompt.is_empty() {
        parts.push(user_prompt.to_string());
    }

    if !history_context.is_empty() {
        parts.push(history_context.to_string());
    }

    parts.join("\n\n")
}

/// Build cleanup-review prompt for post-apply dirty worktree handoff.
///
/// The cleanup-review operation is a strict, handoff-only operation:
/// - It must clean only the apply-generated dirty state.
/// - It must not perform blind staging (e.g. `git add -A`).
/// - On success it must emit exactly one marker: `CLEANUP_REVIEW: CLEAN`.
#[allow(dead_code)]
pub fn build_cleanup_review_prompt(change_id: &str) -> String {
    build_cleanup_review_prompt_with_skill(
        crate::config::defaults::DEFAULT_CLEANUP_REVIEW_SKILL,
        change_id,
    )
}

pub fn build_cleanup_review_prompt_with_skill(
    cleanup_review_skill: &str,
    change_id: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("load skills: {}", cleanup_review_skill));
    parts.push(format!("Cleanup-review change id: {}", change_id));
    parts.push(format!(
        "change_id: {}\nproposal_path: openspec/changes/{}/proposal.md\ntasks_path: openspec/changes/{}/tasks.md\nworkspace_path: .",
        change_id, change_id, change_id
    ));
    parts.push(
        "Rules:\n- Perform only post-apply handoff cleanup for this managed worktree\n- NEVER use blind staging such as `git add -A` or `git add .`\n- Stage and commit only intentional cleanup files needed for clean handoff\n- Do not ask for human input; finish autonomously\n- On success, output exactly one final marker line: CLEANUP_REVIEW: CLEAN"
            .to_string(),
    );

    parts.join("\n\n")
}

/// Parse cleanup-review output and validate the final verdict marker.
///
/// Returns true only when output contains exactly one standalone
/// `CLEANUP_REVIEW: CLEAN` line outside markdown code fences.
pub fn parse_cleanup_review_output(output: &str) -> bool {
    let mut in_code_block = false;
    let mut marker_count = 0_u32;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if trimmed == "CLEANUP_REVIEW: CLEAN" {
            marker_count += 1;
        }
    }

    marker_count == 1
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
#[allow(dead_code)]
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

pub fn build_acceptance_prompt_with_skill(
    accept_skill: &str,
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    last_output_context: &str,
    diff_context: &str,
) -> String {
    build_acceptance_prompt_context_only_with_skill(
        accept_skill,
        change_id,
        user_prompt,
        history_context,
        last_output_context,
        diff_context,
    )
}

/// Repository-agnostic archive-commitability checks that acceptance must
/// evaluate before allowing archive to start.
const ARCHIVE_READINESS_CONTEXT: &str = "<archive_readiness_context>\n\
Before returning ACCEPTANCE: PASS, verify this workspace is ready for the real final archive commit on this repository's actual commit path.\n\
Focus only on blockers that would actually prevent the archive commit from succeeding.\n\
Do not assume that tests, linters, formatters, or pre-commit hooks exist unless they are part of the real commit path for this repository.\n\
If a normal commit in this repository runs hooks or other verification that would block the archive commit, treat that commit-path failure as relevant.\n\
If archive commitability is blocked, return a non-pass verdict and include actionable findings with:\n\
1) the blocking commit-path step or hook,\n\
2) the failing command or commit attempt when available,\n\
3) relevant file/path context when available.\n\
Do not defer commit-path blockers to archive.\n\
</archive_readiness_context>";

/// Build acceptance prompt context without the hardcoded system prompt.
///
/// Use this when the fixed acceptance instructions live in the OpenCode command template
/// and the orchestrator should only inject variable context via `{prompt}`.
#[allow(dead_code)]
pub fn build_acceptance_prompt_context_only(
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    last_output_context: &str,
    diff_context: &str,
) -> String {
    build_acceptance_prompt_context_only_with_skill(
        crate::config::defaults::DEFAULT_ACCEPT_SKILL,
        change_id,
        user_prompt,
        history_context,
        last_output_context,
        diff_context,
    )
}

pub fn build_acceptance_prompt_context_only_with_skill(
    accept_skill: &str,
    change_id: &str,
    user_prompt: &str,
    history_context: &str,
    last_output_context: &str,
    diff_context: &str,
) -> String {
    let mut parts = Vec::new();

    parts.push(format!("load skills: {}", accept_skill));
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
pub(crate) mod tests {
    use super::*;

    /// Expose ARCHIVE_READINESS_CONTEXT for cross-module drift tests.
    pub(crate) fn get_archive_readiness_context() -> &'static str {
        ARCHIVE_READINESS_CONTEXT
    }

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
            .find("load skills: cflx-accept")
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
        assert!(result.contains("load skills: cflx-accept"));
        assert!(result.contains("Acceptance id:test-change"));
        assert!(result.contains("change_id: test-change"));
        assert!(result.contains("proposal_path: openspec/changes/test-change/proposal.md"));
        assert!(result.contains("USER_PROMPT"));

        // Should NOT contain diff context section with actual content
        assert!(!result.contains("Files changed since last acceptance check:"));
        assert!(!result.contains("Previous acceptance findings:"));
    }

    #[test]
    fn test_operation_prompt_builders_use_custom_skill_preludes() {
        let apply =
            build_apply_prompt_with_skill("team-apply", "change-123", "user", "history", "");
        assert!(apply.contains("load skills: team-apply"));
        assert!(!apply.contains("load skills: cflx-apply"));

        let archive =
            build_archive_prompt_with_skill("team-archive", "change-123", "user", "history");
        assert!(archive.contains("load skills: team-archive"));
        assert!(!archive.contains("load skills: cflx-archive"));

        let cleanup = build_cleanup_review_prompt_with_skill("team-cleanup-review", "change-123");
        assert!(cleanup.contains("load skills: team-cleanup-review"));
        assert!(!cleanup.contains("load skills: cflx-cleanup-review"));

        let acceptance = build_acceptance_prompt_context_only_with_skill(
            "cflx-accept-with-speca",
            "change-123",
            "user",
            "history",
            "last",
            "diff",
        );
        assert!(acceptance.contains("load skills: cflx-accept-with-speca"));
        assert!(!acceptance.contains("load skills: cflx-accept\n"));
        assert!(acceptance.contains("change_id: change-123"));
    }

    #[test]
    fn test_build_cleanup_review_prompt_contains_required_context() {
        let prompt = build_cleanup_review_prompt("change-123");

        assert!(prompt.contains("load skills: cflx-cleanup-review"));
        assert!(prompt.contains("Cleanup-review change id: change-123"));
        assert!(prompt.contains("change_id: change-123"));
        assert!(prompt.contains("proposal_path: openspec/changes/change-123/proposal.md"));
        assert!(prompt.contains("tasks_path: openspec/changes/change-123/tasks.md"));
        assert!(prompt.contains("workspace_path: ."));
        assert!(prompt.contains("NEVER use blind staging"));
        assert!(prompt.contains("CLEANUP_REVIEW: CLEAN"));
    }

    #[test]
    fn test_parse_cleanup_review_output_accepts_single_marker() {
        let output = "log line\nCLEANUP_REVIEW: CLEAN\nmore logs";
        assert!(parse_cleanup_review_output(output));
    }

    #[test]
    fn test_parse_cleanup_review_output_rejects_multiple_markers() {
        let output = "CLEANUP_REVIEW: CLEAN\nCLEANUP_REVIEW: CLEAN\n";
        assert!(!parse_cleanup_review_output(output));
    }

    #[test]
    fn test_parse_cleanup_review_output_ignores_code_fence_markers() {
        let output = "```\nCLEANUP_REVIEW: CLEAN\n```\n";
        assert!(!parse_cleanup_review_output(output));
    }
}
