//! Configuration templates for different AI agents

use crate::cli::Template;

/// Claude Code agent configuration template
pub const CLAUDE_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: Claude Code

  // Agent command configuration
  "agent": {
    // Command to run for applying changes
    "apply_command": "claude --dangerously-skip-permissions -p '/openspec:apply {change_id}'"
  },

  // Lifecycle hooks (optional)
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
"#;

/// OpenCode agent configuration template
pub const OPENCODE_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: OpenCode

  // Agent command configuration
  "agent": {
    // Command to run for applying changes
    "apply_command": "opencode run '/openspec-apply {change_id}'"
  },

  // Lifecycle hooks (optional)
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
  }
}
"#;

/// Codex agent configuration template
pub const CODEX_TEMPLATE: &str = r#"{
  // OpenSpec Orchestrator Configuration
  // Template: Codex

  // Agent command configuration
  "agent": {
    // Command to run for applying changes
    "apply_command": "codex '/openspec:apply {change_id}'"
  },

  // Lifecycle hooks (optional)
  "hooks": {
    // "pre_apply": "echo 'Starting {change_id}'",
    // "post_apply": "echo 'Completed {change_id}'"
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

    #[test]
    fn test_claude_template_is_valid_jsonc() {
        // JSONC allows comments and trailing commas, but we should at least
        // verify the template contains expected fields
        assert!(CLAUDE_TEMPLATE.contains("agent"));
        assert!(CLAUDE_TEMPLATE.contains("apply_command"));
        assert!(CLAUDE_TEMPLATE.contains("claude --dangerously-skip-permissions"));
    }

    #[test]
    fn test_opencode_template_is_valid_jsonc() {
        assert!(OPENCODE_TEMPLATE.contains("agent"));
        assert!(OPENCODE_TEMPLATE.contains("apply_command"));
        assert!(OPENCODE_TEMPLATE.contains("opencode run"));
    }

    #[test]
    fn test_codex_template_is_valid_jsonc() {
        assert!(CODEX_TEMPLATE.contains("agent"));
        assert!(CODEX_TEMPLATE.contains("apply_command"));
        assert!(CODEX_TEMPLATE.contains("codex"));
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
}
