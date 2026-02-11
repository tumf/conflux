//! Configuration templates for different AI agents

use crate::cli::Template;

/// Claude Code agent configuration template
pub const CLAUDE_TEMPLATE: &str = r#"{
  // Conflux Configuration
  // Template: Claude Code

  // Command to analyze dependencies and select next change
  "analyze_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/openspec:archive {change_id} {prompt}'",

  // Command to run acceptance tests after apply (supports {change_id} and {prompt} placeholders)
  "acceptance_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p '/cflx-accept {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "claude --dangerously-skip-permissions --verbose --output-format stream-json -p {prompt}",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // System prompt for acceptance command (injected into {prompt} placeholder)
  // Note: A hardcoded acceptance prompt is always prepended before this value
  "acceptance_prompt": "",

  // Controls how the acceptance `{prompt}` is constructed.
  // - full: include hardcoded acceptance system prompt + diff/history context
  // - context_only: only include change metadata + diff/history context
  // Use context_only when the fixed acceptance instructions live in a separate command template.
  "acceptance_prompt_mode": "context_only",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Logging configuration for TUI debug output
  // "logging": {
  //   "suppress_repetitive_debug": true,
  //   "summary_interval_secs": 60
  // },

  // Command to create a worktree for proposals from TUI (+ key)
  // Supports {workspace_dir} and {repo_root} placeholders
  // "worktree_command": "codex '/openspec:proposal --worktree {workspace_dir}'",

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end, on_merged
  //   TUI interaction: on_queue_add, on_queue_remove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}, {completed_tasks}, {total_tasks}
  "hooks": {
    // Run lifecycle
    // "on_start": "echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'",
    // "on_finish": "echo '[on_finish] status={status} processed={changes_processed}/{total_changes}'",
    // "on_error": "echo '[on_error] change={change_id} error={error}'",

    // Change lifecycle
    // "on_change_start": "echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",
    // "pre_apply": "echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "post_apply": "echo '[post_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "on_change_complete": "echo '[on_change_complete] change={change_id} tasks={total_tasks}/{total_tasks}'",
    // "pre_archive": "echo '[pre_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "post_archive": "echo '[post_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "on_change_end": "echo '[on_change_end] change={change_id} progress={changes_processed}/{total_changes} remaining={remaining_changes}'",
    // "on_merged": "echo '[on_merged] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",

    // TUI interaction
    // "on_queue_add": "echo '[on_queue_add] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_queue_remove": "echo '[on_queue_remove] change={change_id} tasks={completed_tasks}/{total_tasks}'"
  }
}
"#;

/// OpenCode agent configuration template
pub const OPENCODE_TEMPLATE: &str = r#"{
  // Conflux Configuration
  // Template: OpenCode

  // Command to analyze dependencies and select next change
  "analyze_command": "opencode run --format json {prompt}",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "opencode run '/openspec-apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "opencode run '/conflux:archive {change_id} {prompt}'",

  // Command to run acceptance tests after apply (supports {change_id} and {prompt} placeholders)
  "acceptance_command": "opencode run '/cflx-accept {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "opencode run {prompt}",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // System prompt for acceptance command (injected into {prompt} placeholder)
  // Note: A hardcoded acceptance prompt is always prepended before this value
  "acceptance_prompt": "",

  // Controls how the acceptance `{prompt}` is constructed.
  // - full: include hardcoded acceptance system prompt + diff/history context
  // - context_only: only include change metadata + diff/history context
  // Use context_only when the fixed acceptance instructions live in the OpenCode command template.
  "acceptance_prompt_mode": "context_only",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Logging configuration for TUI debug output
  // "logging": {
  //   "suppress_repetitive_debug": true,
  //   "summary_interval_secs": 60
  // },

  // Command to create a worktree for proposals from TUI (+ key)
  // Supports {workspace_dir} and {repo_root} placeholders
  // "worktree_command": "claude run '/openspec:proposal --worktree {workspace_dir}'",

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end, on_merged
  //   TUI interaction: on_queue_add, on_queue_remove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}, {completed_tasks}, {total_tasks}
  "hooks": {
    // Run lifecycle
    // "on_start": "echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'",
    // "on_finish": "echo '[on_finish] status={status} processed={changes_processed}/{total_changes}'",
    // "on_error": "echo '[on_error] change={change_id} error={error}'",

    // Change lifecycle
    // "on_change_start": "echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",
    // "pre_apply": "echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "post_apply": "echo '[post_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "on_change_complete": "echo '[on_change_complete] change={change_id} tasks={total_tasks}/{total_tasks}'",
    // "pre_archive": "echo '[pre_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "post_archive": "echo '[post_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "on_change_end": "echo '[on_change_end] change={change_id} progress={changes_processed}/{total_changes} remaining={remaining_changes}'",
    // "on_merged": "echo '[on_merged] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",

    // TUI interaction
    // "on_queue_add": "echo '[on_queue_add] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_queue_remove": "echo '[on_queue_remove] change={change_id} tasks={completed_tasks}/{total_tasks}'"
  }
}
"#;

/// Codex agent configuration template
pub const CODEX_TEMPLATE: &str = r#"{
  // Conflux Configuration
  // Template: Codex

  // Command to analyze dependencies and select next change
  "analyze_command": "codex --json {prompt}",

  // Command to apply a change (supports {change_id} and {prompt} placeholders)
  "apply_command": "codex '/openspec:apply {change_id} {prompt}'",

  // Command to archive a completed change (supports {change_id} and {prompt} placeholders)
  "archive_command": "codex '/openspec:archive {change_id} {prompt}'",

  // Command to run acceptance tests after apply (supports {change_id} and {prompt} placeholders)
  "acceptance_command": "codex '/cflx-accept {change_id} {prompt}'",

  // Command to resolve conflicts (supports {prompt} placeholder)
  "resolve_command": "codex {prompt}",

  // System prompt for apply command (user-customizable, injected into {prompt} placeholder)
  // Note: A hardcoded system prompt is always appended after this value
  "apply_prompt": "",

  // System prompt for archive command (injected into {prompt} placeholder)
  "archive_prompt": "",

  // System prompt for acceptance command (injected into {prompt} placeholder)
  // Note: A hardcoded acceptance prompt is always prepended before this value
  "acceptance_prompt": "",

  // Controls how the acceptance `{prompt}` is constructed.
  // - full: include hardcoded acceptance system prompt + diff/history context
  // - context_only: only include change metadata + diff/history context
  // Use context_only when the fixed acceptance instructions live in a separate command template.
  "acceptance_prompt_mode": "context_only",

  // Maximum iterations for the orchestration loop (default: 50, 0 = no limit)
  // "max_iterations": 50,

  // Logging configuration for TUI debug output
  // "logging": {
  //   "suppress_repetitive_debug": true,
  //   "summary_interval_secs": 60
  // },

  // Command to create a worktree for proposals from TUI (+ key)
  // Supports {workspace_dir} and {repo_root} placeholders
  // "worktree_command": "opencode run '/openspec:proposal --worktree {workspace_dir}'",

  // Lifecycle hooks (optional)
  // Available hooks:
  //   Run lifecycle: on_start, on_finish, on_error
  //   Change lifecycle: on_change_start, pre_apply, post_apply, on_change_complete, pre_archive, post_archive, on_change_end, on_merged
  //   TUI interaction: on_queue_add, on_queue_remove
  // Available placeholders: {change_id}, {changes_processed}, {total_changes}, {remaining_changes}, {apply_count}, {completed_tasks}, {total_tasks}
  "hooks": {
    // Run lifecycle
    // "on_start": "echo '[on_start] changes_processed={changes_processed} total={total_changes} remaining={remaining_changes}'",
    // "on_finish": "echo '[on_finish] status={status} processed={changes_processed}/{total_changes}'",
    // "on_error": "echo '[on_error] change={change_id} error={error}'",

    // Change lifecycle
    // "on_change_start": "echo '[on_change_start] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",
    // "pre_apply": "echo '[pre_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "post_apply": "echo '[post_apply] change={change_id} apply_count={apply_count} tasks={completed_tasks}/{total_tasks}'",
    // "on_change_complete": "echo '[on_change_complete] change={change_id} tasks={total_tasks}/{total_tasks}'",
    // "pre_archive": "echo '[pre_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "post_archive": "echo '[post_archive] change={change_id} progress={changes_processed}/{total_changes}'",
    // "on_change_end": "echo '[on_change_end] change={change_id} progress={changes_processed}/{total_changes} remaining={remaining_changes}'",
    // "on_merged": "echo '[on_merged] change={change_id} tasks={completed_tasks}/{total_tasks} progress={changes_processed}/{total_changes}'",

    // TUI interaction
    // "on_queue_add": "echo '[on_queue_add] change={change_id} tasks={completed_tasks}/{total_tasks}'",
    // "on_queue_remove": "echo '[on_queue_remove] change={change_id} tasks={completed_tasks}/{total_tasks}'"
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
