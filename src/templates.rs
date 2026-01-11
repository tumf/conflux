//! Configuration templates for different AI agents

use crate::cli::Template;

/// Claude Code agent configuration template
pub const CLAUDE_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: Claude Code

  // Command to analyze dependencies and select next change
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '{prompt}'",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end
  //   TUI interaction: on_queue_add, on_queue_remove, on_approve, on_unapprove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}
  "hooks": {
    // "on_change_start": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Applied {change_id} (attempt {apply_count})'"
  }
}
"#;

/// OpenCode agent configuration template
pub const OPENCODE_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: OpenCode

  // Command to analyze dependencies and select next change
  "analyze_command": "opencode run --format json '{prompt}'",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "opencode run '/openspec-apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "opencode run '/openspec-archive {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "opencode run '{prompt}'",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end
  //   TUI interaction: on_queue_add, on_queue_remove, on_approve, on_unapprove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}
  "hooks": {
    // "on_change_start": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Applied {change_id} (attempt {apply_count})'"
  }
}
"#;

/// Codex agent configuration template
pub const CODEX_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: Codex

  // Command to analyze dependencies and select next change
  "analyze_command": "codex --json '{prompt}'",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "codex '/openspec:apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "codex '/openspec:archive {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "codex '{prompt}'",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end
  //   TUI interaction: on_queue_add, on_queue_remove, on_approve, on_unapprove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}
  "hooks": {
    // "on_change_start": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Applied {change_id} (attempt {apply_count})'"
  }
}
"#;

/// Get the configuration template content for the specified template type
pub fn get_template_content(template: Template) -> &'static str {
    match template {
        Template::Claude => CLAUDE_TEMPLATE,
        Template::Opencode => OPENCODE_TEMPLATE,
        Template::Codex => CODEX_TEMPLATE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;

    #[test]
    fn test_claude_template_is_valid_jsonc() {
        // JSONC allows comments and trailing commas, but we should at least
        // verify the template contains expected fields
        assert!(CLAUDE_TEMPLATE.contains("apply_command"));
        assert!(CLAUDE_TEMPLATE.contains("archive_command"));
        assert!(CLAUDE_TEMPLATE.contains("analyze_command"));
        assert!(CLAUDE_TEMPLATE.contains("claude --dangerously-skip-permissions"));
        assert!(CLAUDE_TEMPLATE.contains("--verbose --output-format stream-json"));
        // Should NOT contain nested agent structure
        assert!(!CLAUDE_TEMPLATE.contains("\"agent\":"));
    }

    #[test]
    fn test_opencode_template_is_valid_jsonc() {
        assert!(OPENCODE_TEMPLATE.contains("apply_command"));
        assert!(OPENCODE_TEMPLATE.contains("archive_command"));
        assert!(OPENCODE_TEMPLATE.contains("analyze_command"));
        assert!(OPENCODE_TEMPLATE.contains("opencode run"));
        // Should NOT contain nested agent structure
        assert!(!OPENCODE_TEMPLATE.contains("\"agent\":"));
    }

    #[test]
    fn test_codex_template_is_valid_jsonc() {
        assert!(CODEX_TEMPLATE.contains("apply_command"));
        assert!(CODEX_TEMPLATE.contains("archive_command"));
        assert!(CODEX_TEMPLATE.contains("analyze_command"));
        assert!(CODEX_TEMPLATE.contains("codex"));
        // Should NOT contain nested agent structure
        assert!(!CODEX_TEMPLATE.contains("\"agent\":"));
    }

    #[test]
    fn test_get_template_content() {
        assert_eq!(get_template_content(Template::Claude), CLAUDE_TEMPLATE);
        assert_eq!(get_template_content(Template::Opencode), OPENCODE_TEMPLATE);
        assert_eq!(get_template_content(Template::Codex), CODEX_TEMPLATE);
    }

    #[test]
    fn test_templates_contain_change_id_placeholder() {
        // All templates should contain {change_id} placeholder
        assert!(CLAUDE_TEMPLATE.contains("{change_id}"));
        assert!(OPENCODE_TEMPLATE.contains("{change_id}"));
        assert!(CODEX_TEMPLATE.contains("{change_id}"));
    }

    #[test]
    fn test_claude_template_parseable_by_config() {
        // Template should be parseable by OrchestratorConfig
        let config = OrchestratorConfig::parse_jsonc(CLAUDE_TEMPLATE)
            .expect("CLAUDE_TEMPLATE should be valid JSONC");
        assert!(config.apply_command.is_some());
        assert!(config.archive_command.is_some());
        assert!(config.analyze_command.is_some());
        assert!(config.apply_command.unwrap().contains("claude"));
    }

    #[test]
    fn test_opencode_template_parseable_by_config() {
        // Template should be parseable by OrchestratorConfig
        let config = OrchestratorConfig::parse_jsonc(OPENCODE_TEMPLATE)
            .expect("OPENCODE_TEMPLATE should be valid JSONC");
        assert!(config.apply_command.is_some());
        assert!(config.archive_command.is_some());
        assert!(config.analyze_command.is_some());
        assert!(config.apply_command.unwrap().contains("opencode"));
    }

    #[test]
    fn test_codex_template_parseable_by_config() {
        // Template should be parseable by OrchestratorConfig
        let config = OrchestratorConfig::parse_jsonc(CODEX_TEMPLATE)
            .expect("CODEX_TEMPLATE should be valid JSONC");
        assert!(config.apply_command.is_some());
        assert!(config.archive_command.is_some());
        assert!(config.analyze_command.is_some());
        assert!(config.apply_command.unwrap().contains("codex"));
    }
}
