//! Configuration module for Conflux.
//!
//! Supports JSONC format (JSON with Comments) for configuration files.
//! Configuration is loaded with the following priority:
//! 1. Custom config path (if provided)
//! 2. Project config: `.cflx.jsonc`
//! 3. Global config (XDG): `$XDG_CONFIG_HOME/cflx/config.jsonc` (if XDG_CONFIG_HOME is set)
//! 4. Global config (XDG default): `~/.config/cflx/config.jsonc`
//! 5. Global config (platform default): `dirs::config_dir()/cflx/config.jsonc`
//! 6. Default values (OpenCode-based commands)
//!
//! # Module Structure
//!
//! - `defaults` - Default values and path constants
//! - `expand` - Placeholder expansion utilities
//! - `jsonc` - JSONC parser (reusable by other modules)
//! - `types`  - All configuration struct/enum definitions and business-logic impls
//! - `load`   - File I/O: loading and parsing configuration files

pub mod defaults;
pub mod expand;
pub mod jsonc;
mod load;
mod types;

pub use types::*;

use std::path::PathBuf;

/// Get the XDG config path from environment variable ($XDG_CONFIG_HOME)
///
/// Returns `$XDG_CONFIG_HOME/cflx/config.jsonc` if XDG_CONFIG_HOME is set and non-empty.
fn get_xdg_env_config_path() -> Option<PathBuf> {
    if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg_config_home.is_empty() {
            return Some(
                PathBuf::from(xdg_config_home)
                    .join(GLOBAL_CONFIG_DIR)
                    .join(GLOBAL_CONFIG_FILE),
            );
        }
    }
    None
}

/// Get the XDG config path from default location (~/.config)
///
/// Returns `~/.config/cflx/config.jsonc` if home directory is available.
fn get_xdg_default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".config")
            .join(GLOBAL_CONFIG_DIR)
            .join(GLOBAL_CONFIG_FILE)
    })
}

/// Get the XDG config path, checking $XDG_CONFIG_HOME first, then falling back to ~/.config
///
/// Deprecated: Use get_xdg_env_config_path() and get_xdg_default_config_path() for explicit priority.
/// Returns:
/// - `$XDG_CONFIG_HOME/cflx/config.jsonc` if XDG_CONFIG_HOME is set
/// - `~/.config/cflx/config.jsonc` otherwise
#[deprecated(
    since = "0.1.0",
    note = "Use get_xdg_env_config_path() and get_xdg_default_config_path() for explicit priority"
)]
#[allow(dead_code)]
fn get_xdg_config_path() -> Option<PathBuf> {
    get_xdg_env_config_path().or_else(get_xdg_default_config_path)
}

/// Get the path to the global configuration file (platform default)
///
/// Returns platform-specific config directory + `cflx/config.jsonc`
/// - macOS: `~/Library/Application Support/cflx/config.jsonc`
/// - Linux: `~/.config/cflx/config.jsonc`
/// - Windows: `%APPDATA%/cflx/config.jsonc`
fn get_platform_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|config_dir| config_dir.join(GLOBAL_CONFIG_DIR).join(GLOBAL_CONFIG_FILE))
}

/// Get the path to the global configuration file
///
/// Deprecated: Use get_xdg_env_config_path() and get_xdg_default_config_path() for explicit priority.
/// This function now returns the XDG path for backward compatibility.
#[deprecated(
    since = "0.1.0",
    note = "Use get_xdg_env_config_path() and get_xdg_default_config_path() for explicit priority"
)]
#[allow(dead_code)]
#[allow(deprecated)]
pub fn get_global_config_path() -> Option<PathBuf> {
    get_xdg_config_path()
}

// Re-export commonly used items for convenience; also brings them into scope for path helpers.
#[allow(unused_imports)]
pub use defaults::{
    DEFAULT_ACCEPTANCE_MAX_CONTINUES, DEFAULT_APPLY_PROMPT, DEFAULT_ARCHIVE_PROMPT,
    DEFAULT_MAX_CONCURRENT_WORKSPACES, DEFAULT_MAX_ITERATIONS, GLOBAL_CONFIG_DIR,
    GLOBAL_CONFIG_FILE, PROJECT_CONFIG_FILE,
};

// Re-export external types referenced in tests and public API
#[allow(unused_imports)]
pub use crate::vcs::VcsBackend;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrchestratorConfig::default();
        assert!(config.apply_command.is_none());
        assert!(config.archive_command.is_none());
        assert!(config.analyze_command.is_none());
    }

    #[test]
    fn test_default_logging_config() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_logging(), LoggingConfig::default());
    }

    #[test]
    fn test_parse_logging_config() {
        let jsonc = r#"{
            "logging": {
                "suppress_repetitive_debug": false,
                "summary_interval_secs": 15
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let logging = config.get_logging();
        assert!(!logging.suppress_repetitive_debug);
        assert_eq!(logging.summary_interval_secs, 15);
    }

    #[test]
    fn test_stall_detection_defaults() {
        let config = OrchestratorConfig::default();
        assert_eq!(
            config.get_stall_detection(),
            StallDetectionConfig::default()
        );
    }

    #[test]
    fn test_parse_stall_detection_config() {
        let jsonc = r#"{
            "stall_detection": {
                "enabled": false,
                "threshold": 5
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let stall = config.get_stall_detection();
        assert!(!stall.enabled);
        assert_eq!(stall.threshold, 5);
    }

    #[test]
    fn test_get_commands_missing_returns_error() {
        let config = OrchestratorConfig::default();
        assert!(config.get_apply_command().is_err());
        assert!(config.get_archive_command().is_err());
        assert!(config.get_analyze_command().is_err());
        assert!(config.get_acceptance_command().is_err());
        assert!(config.get_resolve_command().is_err());
    }

    #[test]
    fn test_get_commands_with_custom_values() {
        let config = OrchestratorConfig {
            apply_command: Some("custom apply {change_id}".to_string()),
            archive_command: Some("custom archive {change_id}".to_string()),
            analyze_command: Some("custom analyze '{prompt}'".to_string()),
            acceptance_command: Some("custom acceptance {change_id}".to_string()),
            resolve_command: Some("custom resolve".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_apply_command().unwrap(),
            "custom apply {change_id}"
        );
        assert_eq!(
            config.get_archive_command().unwrap(),
            "custom archive {change_id}"
        );
        assert_eq!(
            config.get_analyze_command().unwrap(),
            "custom analyze '{prompt}'"
        );
        assert_eq!(
            config.get_acceptance_command().unwrap(),
            "custom acceptance {change_id}"
        );
        assert_eq!(config.get_resolve_command().unwrap(), "custom resolve");
    }

    #[test]
    fn test_expand_change_id() {
        let template = "agent run --apply {change_id}";
        let result = OrchestratorConfig::expand_change_id(template, "update-auth");
        assert_eq!(result, "agent run --apply update-auth");
    }

    #[test]
    fn test_expand_change_id_multiple() {
        let template = "agent --id {change_id} --name {change_id}";
        let result = OrchestratorConfig::expand_change_id(template, "fix-bug");
        assert_eq!(result, "agent --id fix-bug --name fix-bug");
    }

    #[test]
    fn test_expand_prompt() {
        let template = "claude '{prompt}'";
        let result = OrchestratorConfig::expand_prompt(template, "Select the next change");
        assert_eq!(result, "claude 'Select the next change'");
    }

    #[test]
    fn test_parse_simple_json() {
        let json = r#"{
            "apply_command": "test apply {change_id}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(json).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_single_line_comments() {
        let jsonc = r#"{
            // This is a comment
            "apply_command": "test apply {change_id}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_multi_line_comments() {
        let jsonc = r#"{
            /* This is a
               multi-line comment */
            "apply_command": "test apply {change_id}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_with_trailing_comma() {
        let jsonc = r#"{
            "apply_command": "test apply {change_id}",
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("test apply {change_id}".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_full_example() {
        let jsonc = r#"{
            // Apply command configuration
            "apply_command": "codex run 'openspec-apply {change_id}'",

            /* Archive command - used after change completion */
            "archive_command": "codex run 'conflux:archive {change_id}'",

            // Dependency analysis command
            "analyze_command": "claude '{prompt}'",
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("codex run 'openspec-apply {change_id}'".to_string())
        );
        assert_eq!(
            config.archive_command,
            Some("codex run 'conflux:archive {change_id}'".to_string())
        );
        assert_eq!(
            config.analyze_command,
            Some("claude '{prompt}'".to_string())
        );
    }

    #[test]
    fn test_parse_jsonc_preserves_strings_with_slashes() {
        let jsonc = r#"{
            "apply_command": "opencode run '/openspec-apply {change_id}'"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.apply_command,
            Some("opencode run '/openspec-apply {change_id}'".to_string())
        );
    }

    #[test]
    fn test_partial_config_requires_all_commands() {
        let jsonc = r#"{
            "apply_command": "custom apply {change_id}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        // Custom value should be used
        assert_eq!(
            config.get_apply_command().unwrap(),
            "custom apply {change_id}"
        );

        // Missing commands should return errors (no fallback to defaults)
        assert!(config.get_archive_command().is_err());
        assert!(config.get_analyze_command().is_err());
    }

    #[test]
    fn test_empty_config_requires_commands() {
        let jsonc = "{}";
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        // All commands should return errors when missing
        assert!(config.get_apply_command().is_err());
        assert!(config.get_archive_command().is_err());
        assert!(config.get_analyze_command().is_err());
    }

    #[test]
    fn test_load_from_custom_path() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary config file with all required commands
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{
                "apply_command": "custom-agent apply {{change_id}}",
                "archive_command": "custom-agent archive {{change_id}}",
                "analyze_command": "custom-agent analyze",
                "acceptance_command": "custom-agent acceptance",
                "resolve_command": "custom-agent resolve"
            }}"#
        )
        .unwrap();

        // Load from custom path
        let config = OrchestratorConfig::load(Some(temp_file.path())).unwrap();

        assert_eq!(
            config.get_apply_command().unwrap(),
            "custom-agent apply {change_id}"
        );
    }

    #[test]
    #[ignore] // Requires isolated environment (may load global config)
    fn test_load_returns_error_when_no_config_exists() {
        use std::env;
        use tempfile::TempDir;

        // Create a temporary directory with no config files
        let temp_dir = TempDir::new().unwrap();

        // Save current directory and environment
        let original_dir = env::current_dir().unwrap();
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_home = env::var("HOME").ok();

        // Point XDG_CONFIG_HOME and HOME to temp directory (where no configs exist)
        env::set_var("XDG_CONFIG_HOME", temp_dir.path().join("config"));
        env::set_var("HOME", temp_dir.path());
        env::set_current_dir(temp_dir.path()).unwrap();

        // Load config - should fail validation due to missing required commands
        let result = OrchestratorConfig::load(None);

        // Restore original directory and environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match original_home {
            Some(val) => env::set_var("HOME", val),
            None => env::remove_var("HOME"),
        }

        // Should return error when no config exists (no fallback to defaults)
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing required config"));
    }

    #[test]
    fn test_load_project_config_takes_priority() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();

        // Create project config file with all required commands
        let project_config_path = temp_dir.path().join(PROJECT_CONFIG_FILE);
        fs::write(
            &project_config_path,
            r#"{
                "apply_command": "project-agent apply {change_id}",
                "archive_command": "project-agent archive {change_id}",
                "analyze_command": "project-agent analyze",
                "acceptance_command": "project-agent acceptance",
                "resolve_command": "project-agent resolve"
            }"#,
        )
        .unwrap();

        // Save current directory and environment
        let original_dir = env::current_dir().unwrap();
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_home = env::var("HOME").ok();

        // Point XDG_CONFIG_HOME and HOME to temp directory (where no global configs exist)
        env::set_var("XDG_CONFIG_HOME", temp_dir.path().join("config"));
        env::set_var("HOME", temp_dir.path());
        env::set_current_dir(temp_dir.path()).unwrap();

        // Load config (should use project config)
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore original directory and environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match original_home {
            Some(val) => env::set_var("HOME", val),
            None => env::remove_var("HOME"),
        }

        // Project config should be used
        assert_eq!(
            config.get_apply_command().unwrap(),
            "project-agent apply {change_id}"
        );
    }

    #[test]
    fn test_get_apply_prompt_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_apply_prompt(), DEFAULT_APPLY_PROMPT);
    }

    #[test]
    fn test_get_archive_prompt_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_archive_prompt(), DEFAULT_ARCHIVE_PROMPT);
    }

    #[test]
    fn test_get_prompts_with_custom_values() {
        let config = OrchestratorConfig {
            apply_prompt: Some("Custom apply prompt".to_string()),
            archive_prompt: Some("Custom archive prompt".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_apply_prompt(), "Custom apply prompt");
        assert_eq!(config.get_archive_prompt(), "Custom archive prompt");
    }

    #[test]
    fn test_parse_jsonc_with_prompts() {
        let jsonc = r#"{
            "apply_command": "claude -p '/openspec:apply {change_id} {prompt}'",
            "archive_command": "claude -p '/openspec:archive {change_id} {prompt}'",
            "apply_prompt": "Test apply prompt",
            "archive_prompt": "Test archive prompt"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.apply_prompt, Some("Test apply prompt".to_string()));
        assert_eq!(
            config.archive_prompt,
            Some("Test archive prompt".to_string())
        );
        assert_eq!(config.get_apply_prompt(), "Test apply prompt");
        assert_eq!(config.get_archive_prompt(), "Test archive prompt");
    }

    #[test]
    fn test_expand_prompt_in_apply_command() {
        let template = "claude -p '/openspec:apply {change_id} {prompt}'";
        let command = OrchestratorConfig::expand_change_id(template, "fix-bug");
        let command = OrchestratorConfig::expand_prompt(&command, "Custom instructions");
        assert_eq!(
            command,
            "claude -p '/openspec:apply fix-bug Custom instructions'"
        );
    }

    #[test]
    fn test_expand_prompt_with_empty_string() {
        let template = "claude -p '/openspec:archive {change_id} {prompt}'";
        let command = OrchestratorConfig::expand_change_id(template, "add-feature");
        let command = OrchestratorConfig::expand_prompt(&command, "");
        assert_eq!(command, "claude -p '/openspec:archive add-feature '");
    }

    #[test]
    fn test_backward_compatible_no_prompt_placeholder() {
        // Commands without {prompt} placeholder should continue to work
        let template = "claude -p '/openspec:apply {change_id}'";
        let command = OrchestratorConfig::expand_change_id(template, "fix-bug");
        let command = OrchestratorConfig::expand_prompt(&command, "Ignored prompt");
        // The {prompt} replacement does nothing since placeholder doesn't exist
        assert_eq!(command, "claude -p '/openspec:apply fix-bug'");
    }

    #[test]
    fn test_get_max_iterations_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_max_iterations(), DEFAULT_MAX_ITERATIONS);
        assert_eq!(config.get_max_iterations(), 50);
    }

    #[test]
    fn test_get_max_iterations_custom() {
        let config = OrchestratorConfig {
            max_iterations: Some(100),
            ..Default::default()
        };
        assert_eq!(config.get_max_iterations(), 100);
    }

    #[test]
    fn test_get_max_iterations_zero_disables_limit() {
        let config = OrchestratorConfig {
            max_iterations: Some(0),
            ..Default::default()
        };
        assert_eq!(config.get_max_iterations(), 0);
    }

    #[test]
    fn test_parse_jsonc_with_max_iterations() {
        let jsonc = r#"{
            "max_iterations": 75
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.max_iterations, Some(75));
        assert_eq!(config.get_max_iterations(), 75);
    }

    #[test]
    fn test_parse_jsonc_max_iterations_zero() {
        let jsonc = r#"{
            "max_iterations": 0
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.max_iterations, Some(0));
        assert_eq!(config.get_max_iterations(), 0);
    }

    #[test]
    fn test_get_propose_command_default() {
        let config = OrchestratorConfig::default();
        assert!(config.get_propose_command().is_none());
    }

    #[test]
    fn test_get_propose_command_configured() {
        let config = OrchestratorConfig {
            propose_command: Some("claude -p '/openspec:proposal {proposal}'".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_propose_command(),
            Some("claude -p '/openspec:proposal {proposal}'")
        );
    }

    #[test]
    fn test_parse_jsonc_with_propose_command() {
        let jsonc = r#"{
            "propose_command": "opencode run '/openspec:proposal {proposal}'"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.propose_command,
            Some("opencode run '/openspec:proposal {proposal}'".to_string())
        );
    }

    #[test]
    fn test_get_worktree_command_default() {
        let config = OrchestratorConfig::default();
        assert!(config.get_worktree_command().is_none());
    }

    #[test]
    fn test_get_worktree_command_configured() {
        let config = OrchestratorConfig {
            worktree_command: Some("cmd --repo {repo_root}".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_worktree_command(),
            Some("cmd --repo {repo_root}")
        );
    }

    #[test]
    fn test_parse_jsonc_with_worktree_command() {
        let jsonc = r#"{
            "worktree_command": "cmd --cwd {workspace_dir}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(
            config.worktree_command,
            Some("cmd --cwd {workspace_dir}".to_string())
        );
    }

    #[test]
    fn test_expand_worktree_command() {
        let template = "cmd {workspace_dir} {repo_root}";
        let result =
            OrchestratorConfig::expand_worktree_command(template, "/tmp/worktree", "/repo/root");
        assert_eq!(result, "cmd /tmp/worktree /repo/root");
    }

    #[test]
    fn test_expand_proposal_simple() {
        let template = "claude -p '{proposal}'";
        let result = OrchestratorConfig::expand_proposal(template, "Add login feature");
        assert_eq!(result, "claude -p 'Add login feature'");
    }

    #[test]
    fn test_expand_proposal_multiline() {
        let template = "claude -p '{proposal}'";
        let proposal = "Add login feature\n- Username\n- Password";
        let result = OrchestratorConfig::expand_proposal(template, proposal);
        assert_eq!(
            result,
            "claude -p 'Add login feature\n- Username\n- Password'"
        );
    }

    // === Tests for hooks config in OrchestratorConfig (hooks spec 3.1) ===

    #[test]
    fn test_hooks_config_can_be_parsed_from_jsonc() {
        let jsonc = r#"{
            "hooks": {
                "on_queue_add": "echo 'Added {change_id}'",
                "on_queue_remove": "echo 'Removed {change_id}'"
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let hooks = config.get_hooks();

        use crate::hooks::HookType;
        assert!(hooks.get(HookType::OnQueueAdd).is_some());
        assert!(hooks.get(HookType::OnQueueRemove).is_some());
    }

    #[test]
    fn test_hooks_config_with_all_hook_types() {
        let jsonc = r#"{
            "hooks": {
                "on_start": "echo start",
                "on_finish": "echo finish",
                "on_error": "echo error",
                "on_change_start": "echo change_start",
                "pre_apply": "echo pre_apply",
                "post_apply": "echo post_apply",
                "on_change_complete": "echo change_complete",
                "pre_archive": "echo pre_archive",
                "post_archive": "echo post_archive",
                "on_change_end": "echo change_end",
                "on_queue_add": "echo queue_add",
                "on_queue_remove": "echo queue_remove"
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let hooks = config.get_hooks();

        use crate::hooks::HookType;
        assert!(hooks.get(HookType::OnStart).is_some());
        assert!(hooks.get(HookType::OnFinish).is_some());
        assert!(hooks.get(HookType::OnError).is_some());
        assert!(hooks.get(HookType::OnChangeStart).is_some());
        assert!(hooks.get(HookType::PreApply).is_some());
        assert!(hooks.get(HookType::PostApply).is_some());
        assert!(hooks.get(HookType::OnChangeComplete).is_some());
        assert!(hooks.get(HookType::PreArchive).is_some());
        assert!(hooks.get(HookType::PostArchive).is_some());
        assert!(hooks.get(HookType::OnChangeEnd).is_some());
        assert!(hooks.get(HookType::OnQueueAdd).is_some());
        assert!(hooks.get(HookType::OnQueueRemove).is_some());
    }

    #[test]
    fn test_get_hooks_returns_default_when_not_configured() {
        let config = OrchestratorConfig::default();
        let hooks = config.get_hooks();

        // Default HooksConfig should have no hooks configured
        use crate::hooks::HookType;
        assert!(hooks.get(HookType::OnStart).is_none());
        assert!(hooks.get(HookType::OnQueueAdd).is_none());
    }

    // === Tests for parallel execution config (parallel-execution spec) ===

    #[test]
    fn test_parallel_mode_can_be_enabled() {
        let config = OrchestratorConfig {
            parallel_mode: Some(true),
            ..Default::default()
        };
        assert!(config.get_parallel_mode());
    }

    #[test]
    fn test_resolve_parallel_mode_prefers_cli_override() {
        let config = OrchestratorConfig {
            parallel_mode: Some(false),
            ..Default::default()
        };
        assert!(config.resolve_parallel_mode(true, false));
    }

    #[test]
    fn test_resolve_parallel_mode_defaults_to_git_detection() {
        let config = OrchestratorConfig::default();
        assert!(!config.resolve_parallel_mode(false, false));
        assert!(config.resolve_parallel_mode(false, true));
    }

    #[test]
    fn test_resolve_parallel_mode_uses_config_value() {
        let enabled = OrchestratorConfig {
            parallel_mode: Some(true),
            ..Default::default()
        };
        let disabled = OrchestratorConfig {
            parallel_mode: Some(false),
            ..Default::default()
        };

        assert!(enabled.resolve_parallel_mode(false, false));
        assert!(!disabled.resolve_parallel_mode(false, true));
    }

    #[test]
    fn test_max_concurrent_workspaces_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(
            config.get_max_concurrent_workspaces(),
            DEFAULT_MAX_CONCURRENT_WORKSPACES
        );
        // Default is 3 according to defaults.rs
        assert_eq!(config.get_max_concurrent_workspaces(), 3);
    }

    #[test]
    fn test_max_concurrent_workspaces_can_be_configured() {
        let config = OrchestratorConfig {
            max_concurrent_workspaces: Some(8),
            ..Default::default()
        };
        assert_eq!(config.get_max_concurrent_workspaces(), 8);
    }

    #[test]
    fn test_workspace_base_dir_default_is_none() {
        let config = OrchestratorConfig::default();
        assert!(config.get_workspace_base_dir().is_none());
    }

    #[test]
    fn test_workspace_base_dir_can_be_configured() {
        let config = OrchestratorConfig {
            workspace_base_dir: Some("/tmp/ws".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_workspace_base_dir(), Some("/tmp/ws"));
    }

    #[test]
    fn test_workspace_base_dir_empty_string_treated_as_none() {
        let config = OrchestratorConfig {
            workspace_base_dir: Some("".to_string()),
            ..Default::default()
        };
        assert!(config.get_workspace_base_dir().is_none());
    }

    #[test]
    fn test_vcs_backend_defaults_to_auto() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_vcs_backend(), VcsBackend::Auto);
    }

    #[test]
    fn test_vcs_backend_can_be_set_to_git() {
        let config = OrchestratorConfig {
            vcs_backend: Some(VcsBackend::Git),
            ..Default::default()
        };
        assert_eq!(config.get_vcs_backend(), VcsBackend::Git);
    }

    #[test]
    fn test_use_llm_analysis_defaults_to_true() {
        let config = OrchestratorConfig::default();
        assert!(config.use_llm_analysis());
    }

    #[test]
    fn test_use_llm_analysis_can_be_disabled() {
        let config = OrchestratorConfig {
            use_llm_analysis: Some(false),
            ..Default::default()
        };
        assert!(!config.use_llm_analysis());
    }

    #[test]
    fn test_parse_jsonc_parallel_config() {
        let jsonc = r#"{
            "parallel_mode": true,
            "max_concurrent_workspaces": 6,
            "workspace_base_dir": "/custom/path",
            "vcs_backend": "git",
            "use_llm_analysis": false
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        assert!(config.get_parallel_mode());
        assert_eq!(config.get_max_concurrent_workspaces(), 6);
        assert_eq!(config.get_workspace_base_dir(), Some("/custom/path"));
        assert_eq!(config.get_vcs_backend(), VcsBackend::Git);
        assert!(!config.use_llm_analysis());
    }

    // === Tests for resolve_command config ===

    #[test]
    fn test_resolve_command_missing_returns_error() {
        let config = OrchestratorConfig::default();
        // Should return error when resolve command is missing
        assert!(config.get_resolve_command().is_err());
    }

    #[test]
    fn test_resolve_command_can_be_configured() {
        let config = OrchestratorConfig {
            resolve_command: Some("custom-resolver {conflict_files}".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_resolve_command().unwrap(),
            "custom-resolver {conflict_files}"
        );
    }

    #[test]
    fn test_expand_conflict_files_placeholder() {
        let template = "claude resolve {conflict_files}";
        let conflict_files = "src/main.rs src/lib.rs";
        let result = OrchestratorConfig::expand_conflict_files(template, conflict_files);
        let expected = format!(
            "claude resolve {}",
            shlex::try_quote(conflict_files).unwrap()
        );
        assert_eq!(result, expected);
    }

    // === Tests for command queue config ===

    #[test]
    fn test_command_queue_config_defaults() {
        let config = OrchestratorConfig::default();
        // Default values should be None (will use defaults from defaults.rs)
        assert!(config.command_queue_stagger_delay_ms.is_none());
        assert!(config.command_queue_max_retries.is_none());
        assert!(config.command_queue_retry_delay_ms.is_none());
        assert!(config.command_queue_retry_patterns.is_none());
        assert!(config.command_queue_retry_if_duration_under_secs.is_none());
    }

    #[test]
    fn test_command_queue_config_custom() {
        let jsonc = r#"{
            "command_queue_stagger_delay_ms": 3000,
            "command_queue_max_retries": 3,
            "command_queue_retry_delay_ms": 10000,
            "command_queue_retry_patterns": [
                "Custom error pattern",
                "Another pattern"
            ],
            "command_queue_retry_if_duration_under_secs": 10
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        assert_eq!(config.command_queue_stagger_delay_ms, Some(3000));
        assert_eq!(config.command_queue_max_retries, Some(3));
        assert_eq!(config.command_queue_retry_delay_ms, Some(10000));
        assert_eq!(
            config.command_queue_retry_patterns,
            Some(vec![
                "Custom error pattern".to_string(),
                "Another pattern".to_string()
            ])
        );
        assert_eq!(config.command_queue_retry_if_duration_under_secs, Some(10));
    }

    #[test]
    fn test_parse_jsonc_with_command_queue() {
        let jsonc = r#"{
            // Command queue configuration
            "command_queue_stagger_delay_ms": 1500,
            "command_queue_max_retries": 5
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        assert_eq!(config.command_queue_stagger_delay_ms, Some(1500));
        assert_eq!(config.command_queue_max_retries, Some(5));
    }

    #[test]
    fn test_acceptance_max_continues_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(
            config.get_acceptance_max_continues(),
            DEFAULT_ACCEPTANCE_MAX_CONTINUES
        );
        assert_eq!(config.get_acceptance_max_continues(), 10);
    }

    #[test]
    fn test_acceptance_max_continues_custom() {
        let config = OrchestratorConfig {
            acceptance_max_continues: Some(4),
            ..Default::default()
        };
        assert_eq!(config.get_acceptance_max_continues(), 4);
    }

    #[test]
    fn test_parse_jsonc_with_acceptance_max_continues() {
        let jsonc = r#"{
            "acceptance_max_continues": 5
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.acceptance_max_continues, Some(5));
        assert_eq!(config.get_acceptance_max_continues(), 5);
    }

    #[test]
    fn test_inactivity_timeout_max_retries_default_is_three() {
        let config = OrchestratorConfig::default();
        assert!(config.command_inactivity_timeout_max_retries.is_none());
        assert_eq!(
            config.get_command_inactivity_timeout_max_retries(),
            3,
            "Default inactivity timeout max retries must be 3"
        );
    }

    #[test]
    fn test_inactivity_timeout_max_retries_can_be_disabled() {
        let jsonc = r#"{
            "command_inactivity_timeout_max_retries": 0
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.command_inactivity_timeout_max_retries, Some(0));
        assert_eq!(config.get_command_inactivity_timeout_max_retries(), 0);
    }

    #[test]
    fn test_inactivity_timeout_max_retries_can_be_configured() {
        let jsonc = r#"{
            "command_inactivity_timeout_max_retries": 3
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.command_inactivity_timeout_max_retries, Some(3));
        assert_eq!(config.get_command_inactivity_timeout_max_retries(), 3);
    }

    #[test]
    fn test_merge_stall_detection_defaults() {
        let config = OrchestratorConfig::default();
        let merge_stall = config.get_merge_stall_detection();
        assert!(merge_stall.enabled);
        assert_eq!(merge_stall.threshold_minutes, 30);
        assert_eq!(merge_stall.check_interval_seconds, 60);
    }

    #[test]
    fn test_parse_merge_stall_detection_config() {
        let jsonc = r#"{
            "merge_stall_detection": {
                "enabled": true,
                "threshold_minutes": 30,
                "check_interval_seconds": 60
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let merge_stall = config.get_merge_stall_detection();
        assert!(merge_stall.enabled);
        assert_eq!(merge_stall.threshold_minutes, 30);
        assert_eq!(merge_stall.check_interval_seconds, 60);
    }

    #[test]
    fn test_merge_stall_detection_disabled() {
        let jsonc = r#"{
            "merge_stall_detection": {
                "enabled": false
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let merge_stall = config.get_merge_stall_detection();
        assert!(!merge_stall.enabled);
    }

    // === Tests for XDG config path precedence ===

    #[test]
    #[allow(deprecated)]
    fn test_get_xdg_config_path_returns_path() {
        // Test that get_xdg_config_path returns a valid path
        let result = super::get_xdg_config_path();
        assert!(result.is_some());

        let path = result.unwrap();
        let path_str = path.to_string_lossy();

        // Should end with cflx/config.jsonc
        assert!(
            path_str.ends_with("cflx/config.jsonc"),
            "Expected path to end with cflx/config.jsonc, got: {:?}",
            path
        );

        // Should contain either .config (XDG default) or custom XDG_CONFIG_HOME
        assert!(
            path_str.contains(".config") || std::env::var("XDG_CONFIG_HOME").is_ok(),
            "Expected path to contain .config or use XDG_CONFIG_HOME, got: {:?}",
            path
        );
    }

    #[test]
    fn test_get_xdg_env_config_path() {
        use std::env;

        // Test without XDG_CONFIG_HOME
        env::remove_var("XDG_CONFIG_HOME");
        assert!(super::get_xdg_env_config_path().is_none());

        // Test with XDG_CONFIG_HOME set
        env::set_var("XDG_CONFIG_HOME", "/custom/config");
        let result = super::get_xdg_env_config_path();
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path.to_str().unwrap(), "/custom/config/cflx/config.jsonc");

        // Clean up
        env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn test_get_xdg_default_config_path() {
        // Should always return a path if home directory is available
        let result = super::get_xdg_default_config_path();
        if let Some(path) = result {
            let path_str = path.to_string_lossy();
            assert!(path_str.contains(".config"));
            assert!(path_str.ends_with("cflx/config.jsonc"));
        }
    }

    #[test]
    fn test_get_platform_config_path_returns_path() {
        // Test that get_platform_config_path returns a valid path
        let result = super::get_platform_config_path();

        // Platform path may be None if dirs::config_dir() is None (rare)
        if let Some(path) = result {
            let path_str = path.to_string_lossy();
            assert!(
                path_str.ends_with("cflx/config.jsonc"),
                "Expected path to end with cflx/config.jsonc, got: {:?}",
                path
            );
        }
    }

    #[test]
    #[ignore] // Requires sequential execution due to env::current_dir() manipulation
    fn test_load_xdg_config_precedence() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories for XDG and platform configs
        let xdg_dir = TempDir::new().unwrap();
        let platform_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        // Create XDG config with all required commands
        let xdg_config_dir = xdg_dir.path().join("cflx");
        fs::create_dir_all(&xdg_config_dir).unwrap();
        fs::write(
            xdg_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "xdg-agent apply {change_id}",
                "archive_command": "xdg-agent archive {change_id}",
                "analyze_command": "xdg-agent analyze",
                "acceptance_command": "xdg-agent acceptance",
                "resolve_command": "xdg-agent resolve"
            }"#,
        )
        .unwrap();

        // Create platform config with different content
        let platform_config_dir = platform_dir.path().join("cflx");
        fs::create_dir_all(&platform_config_dir).unwrap();
        fs::write(
            platform_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "platform-agent apply {change_id}",
                "archive_command": "platform-agent archive {change_id}",
                "analyze_command": "platform-agent analyze",
                "acceptance_command": "platform-agent acceptance",
                "resolve_command": "platform-agent resolve"
            }"#,
        )
        .unwrap();

        // Save original environment and directory
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME and change to work directory (no project config)
        env::set_var("XDG_CONFIG_HOME", xdg_dir.path());
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config - should prefer XDG path
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }

        // XDG config should be loaded
        assert_eq!(
            config.get_apply_command().unwrap(),
            "xdg-agent apply {change_id}",
            "Expected XDG config to be loaded"
        );
    }

    #[test]
    #[ignore] // Requires isolated environment (may load global config)
    fn test_load_platform_fallback_when_xdg_missing() {
        use std::env;
        use tempfile::TempDir;

        // Create temporary work directory with no config files
        let work_dir = TempDir::new().unwrap();

        // Save original environment and directory
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME to non-existent path
        let nonexistent = work_dir.path().join("nonexistent");
        env::set_var("XDG_CONFIG_HOME", &nonexistent);
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config - should fail due to missing required commands
        let result = OrchestratorConfig::load(None);

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }

        // Should return error since no config files exist with required commands
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing required config"));
    }

    #[test]
    #[ignore] // Requires sequential execution due to env::current_dir() manipulation
    fn test_project_config_takes_priority_over_xdg() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories
        let xdg_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        // Create XDG config with all required commands
        let xdg_config_dir = xdg_dir.path().join("cflx");
        fs::create_dir_all(&xdg_config_dir).unwrap();
        fs::write(
            xdg_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "xdg-agent apply {change_id}",
                "archive_command": "xdg-agent archive {change_id}",
                "analyze_command": "xdg-agent analyze",
                "acceptance_command": "xdg-agent acceptance",
                "resolve_command": "xdg-agent resolve"
            }"#,
        )
        .unwrap();

        // Create project config with different apply_command (other commands inherited from XDG)
        fs::write(
            work_dir.path().join(PROJECT_CONFIG_FILE),
            r#"{"apply_command": "project-agent apply {change_id}"}"#,
        )
        .unwrap();

        // Save original environment and directory
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME and change to work directory
        env::set_var("XDG_CONFIG_HOME", xdg_dir.path());
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config - should prefer project config for apply_command
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }

        // Project config should take priority for apply_command
        assert_eq!(
            config.get_apply_command().unwrap(),
            "project-agent apply {change_id}",
            "Expected project config to take priority over XDG config"
        );
        // Other commands should be inherited from XDG
        assert_eq!(
            config.get_archive_command().unwrap(),
            "xdg-agent archive {change_id}"
        );
    }

    // Test for merge-based config loading
    #[test]
    fn test_config_merge_partial_project_inherits_global() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories
        let xdg_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        // Create XDG config with all commands
        let xdg_config_dir = xdg_dir.path().join("cflx");
        fs::create_dir_all(&xdg_config_dir).unwrap();
        fs::write(
            xdg_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "global-agent apply {change_id}",
                "archive_command": "global-agent archive {change_id}",
                "analyze_command": "global-agent analyze '{prompt}'"
            }"#,
        )
        .unwrap();

        // Create project config with only apply_command (partial config)
        let project_config_path = work_dir.path().join(PROJECT_CONFIG_FILE);
        fs::write(
            &project_config_path,
            r#"{"apply_command": "project-agent apply {change_id}"}"#,
        )
        .unwrap();

        // Save original environment
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME and work directory
        env::set_var("XDG_CONFIG_HOME", xdg_dir.path());
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config (should merge project and global)
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }

        // Project config should override apply_command
        assert_eq!(
            config.get_apply_command().unwrap(),
            "project-agent apply {change_id}",
            "Project config should override apply_command"
        );

        // Global config should provide archive_command and analyze_command
        assert_eq!(
            config.get_archive_command().unwrap(),
            "global-agent archive {change_id}",
            "Global config should provide archive_command when missing in project config"
        );
        assert_eq!(
            config.get_analyze_command().unwrap(),
            "global-agent analyze '{prompt}'",
            "Global config should provide analyze_command when missing in project config"
        );
    }

    #[test]
    fn test_hooks_deep_merge() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories
        let xdg_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        // Create XDG config with on_start and pre_apply hooks
        let xdg_config_dir = xdg_dir.path().join("cflx");
        fs::create_dir_all(&xdg_config_dir).unwrap();
        fs::write(
            xdg_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "test apply",
                "hooks": {
                    "on_start": "echo global start",
                    "pre_apply": "echo global pre_apply"
                }
            }"#,
        )
        .unwrap();

        // Create project config with on_finish and post_apply hooks
        let project_config_path = work_dir.path().join(PROJECT_CONFIG_FILE);
        fs::write(
            &project_config_path,
            r#"{
                "hooks": {
                    "on_finish": "echo project finish",
                    "post_apply": "echo project post_apply"
                }
            }"#,
        )
        .unwrap();

        // Save original environment
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME and work directory
        env::set_var("XDG_CONFIG_HOME", xdg_dir.path());
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config (should deep merge hooks)
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }

        let hooks = config.get_hooks();

        // All hooks should be present (deep merge)
        use crate::hooks::HookType;
        assert!(hooks.get(HookType::OnStart).is_some());
        assert!(hooks.get(HookType::PreApply).is_some());
        assert!(hooks.get(HookType::OnFinish).is_some());
        assert!(hooks.get(HookType::PostApply).is_some());

        // Verify hook commands
        assert_eq!(
            hooks.get(HookType::OnStart).unwrap().command,
            "echo global start"
        );
        assert_eq!(
            hooks.get(HookType::PreApply).unwrap().command,
            "echo global pre_apply"
        );
        assert_eq!(
            hooks.get(HookType::OnFinish).unwrap().command,
            "echo project finish"
        );
        assert_eq!(
            hooks.get(HookType::PostApply).unwrap().command,
            "echo project post_apply"
        );
    }

    // === Tests for required command validation ===

    #[test]
    fn test_validate_required_commands_all_present() {
        let config = OrchestratorConfig {
            apply_command: Some("apply".to_string()),
            archive_command: Some("archive".to_string()),
            analyze_command: Some("analyze".to_string()),
            acceptance_command: Some("acceptance".to_string()),
            resolve_command: Some("resolve".to_string()),
            ..Default::default()
        };

        // Should not error when all required commands are present
        assert!(config.validate_required_commands().is_ok());
    }

    #[test]
    fn test_validate_required_commands_missing_apply() {
        let config = OrchestratorConfig {
            archive_command: Some("archive".to_string()),
            analyze_command: Some("analyze".to_string()),
            acceptance_command: Some("acceptance".to_string()),
            resolve_command: Some("resolve".to_string()),
            ..Default::default()
        };

        let result = config.validate_required_commands();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("apply_command"));
    }

    #[test]
    fn test_validate_required_commands_missing_multiple() {
        let config = OrchestratorConfig {
            apply_command: Some("apply".to_string()),
            // Missing: archive, analyze, acceptance, resolve
            ..Default::default()
        };

        let result = config.validate_required_commands();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("archive_command"));
        assert!(err_msg.contains("analyze_command"));
        assert!(err_msg.contains("acceptance_command"));
        assert!(err_msg.contains("resolve_command"));
    }

    #[test]
    #[ignore] // Requires isolated environment (may load global config)
    fn test_load_validation_fails_on_missing_commands() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".cflx.jsonc");

        // Write a config with missing required commands
        fs::write(
            &config_path,
            r#"{
                "apply_command": "test apply"
            }"#,
        )
        .unwrap();

        // Set current dir to temp dir
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Load should fail due to missing commands
        let result = OrchestratorConfig::load(None);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("archive_command"));
        assert!(err_msg.contains("analyze_command"));
        assert!(err_msg.contains("acceptance_command"));
        assert!(err_msg.contains("resolve_command"));

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[ignore] // Requires sequential execution due to env manipulation
    fn test_xdg_env_takes_priority_over_xdg_default() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories for XDG env and default paths
        let xdg_env_dir = TempDir::new().unwrap();
        let xdg_default_dir = TempDir::new().unwrap();
        let work_dir = TempDir::new().unwrap();

        // Create XDG env config (higher priority)
        let xdg_env_config_dir = xdg_env_dir.path().join("cflx");
        fs::create_dir_all(&xdg_env_config_dir).unwrap();
        fs::write(
            xdg_env_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "xdg-env-agent apply {change_id}",
                "archive_command": "xdg-env-agent archive {change_id}",
                "analyze_command": "xdg-env-agent analyze",
                "acceptance_command": "xdg-env-agent acceptance",
                "resolve_command": "xdg-env-agent resolve"
            }"#,
        )
        .unwrap();

        // Create XDG default config (lower priority) - this should be at ~/.config location
        // We'll simulate this by setting HOME to xdg_default_dir
        let home_config_dir = xdg_default_dir.path().join(".config").join("cflx");
        fs::create_dir_all(&home_config_dir).unwrap();
        fs::write(
            home_config_dir.join("config.jsonc"),
            r#"{
                "apply_command": "xdg-default-agent apply {change_id}",
                "archive_command": "xdg-default-agent archive {change_id}",
                "analyze_command": "xdg-default-agent analyze",
                "acceptance_command": "xdg-default-agent acceptance",
                "resolve_command": "xdg-default-agent resolve"
            }"#,
        )
        .unwrap();

        // Save original environment and directory
        let original_xdg = env::var("XDG_CONFIG_HOME").ok();
        let original_home = env::var("HOME").ok();
        let original_dir = env::current_dir().unwrap();

        // Set XDG_CONFIG_HOME (env) and HOME (for default path)
        env::set_var("XDG_CONFIG_HOME", xdg_env_dir.path());
        env::set_var("HOME", xdg_default_dir.path());
        env::set_current_dir(work_dir.path()).unwrap();

        // Load config - XDG env should take priority over XDG default
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore environment
        env::set_current_dir(original_dir).unwrap();
        match original_xdg {
            Some(val) => env::set_var("XDG_CONFIG_HOME", val),
            None => env::remove_var("XDG_CONFIG_HOME"),
        }
        match original_home {
            Some(val) => env::set_var("HOME", val),
            None => env::remove_var("HOME"),
        }

        // XDG env config should be loaded (higher priority)
        assert_eq!(
            config.get_apply_command().unwrap(),
            "xdg-env-agent apply {change_id}",
            "Expected XDG env config to take priority over XDG default config"
        );
    }

    // ── ServerConfig validation tests ──

    #[test]
    fn test_server_config_validate_loopback_no_auth_ok() {
        // Loopback bind without auth is allowed
        let config = ServerConfig {
            bind: "127.0.0.1".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::None,
                token: None,
                token_env: None,
            },
            ..ServerConfig::default()
        };
        assert!(
            config.validate().is_ok(),
            "Loopback bind without auth should be allowed"
        );
    }

    #[test]
    fn test_server_config_validate_loopback_with_auth_ok() {
        // Loopback bind with auth is also allowed
        let config = ServerConfig {
            bind: "127.0.0.1".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::BearerToken,
                token: Some("secret".to_string()),
                token_env: None,
            },
            ..ServerConfig::default()
        };
        assert!(
            config.validate().is_ok(),
            "Loopback bind with auth should be allowed"
        );
    }

    #[test]
    fn test_server_config_validate_non_loopback_no_auth_fails() {
        // Non-loopback bind without auth must fail
        let config = ServerConfig {
            bind: "0.0.0.0".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::None,
                token: None,
                token_env: None,
            },
            ..ServerConfig::default()
        };
        let result = config.validate();
        assert!(
            result.is_err(),
            "Non-loopback bind without auth should be rejected"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("bearer_token"),
            "Error message should mention bearer_token requirement"
        );
    }

    #[test]
    fn test_server_config_validate_non_loopback_bearer_token_ok() {
        // Non-loopback bind with valid bearer token is allowed
        let config = ServerConfig {
            bind: "0.0.0.0".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::BearerToken,
                token: Some("my-secret-token".to_string()),
                token_env: None,
            },
            ..ServerConfig::default()
        };
        assert!(
            config.validate().is_ok(),
            "Non-loopback bind with valid bearer token should be allowed"
        );
    }

    #[test]
    fn test_server_config_validate_non_loopback_bearer_token_empty_fails() {
        // Non-loopback bind with empty bearer token must fail
        let config = ServerConfig {
            bind: "0.0.0.0".to_string(),
            auth: ServerAuthConfig {
                mode: ServerAuthMode::BearerToken,
                token: Some("".to_string()),
                token_env: None,
            },
            ..ServerConfig::default()
        };
        let result = config.validate();
        assert!(
            result.is_err(),
            "Non-loopback bind with empty token should be rejected"
        );
    }

    #[test]
    fn test_server_config_is_loopback_bind() {
        // Test various loopback and non-loopback addresses
        let loopback_cases = ["127.0.0.1", "127.0.0.2", "127.1.2.3", "localhost", "::1"];
        for addr in &loopback_cases {
            let config = ServerConfig {
                bind: addr.to_string(),
                ..ServerConfig::default()
            };
            assert!(config.is_loopback_bind(), "'{}' should be loopback", addr);
        }

        let non_loopback_cases = ["0.0.0.0", "192.168.1.1", "10.0.0.1", "::"];
        for addr in &non_loopback_cases {
            let config = ServerConfig {
                bind: addr.to_string(),
                ..ServerConfig::default()
            };
            assert!(
                !config.is_loopback_bind(),
                "'{}' should not be loopback",
                addr
            );
        }
    }

    // ── ServerConfig::apply_cli_overrides data_dir tests ──

    #[test]
    fn test_server_config_apply_cli_overrides_data_dir() {
        // Verify that --data-dir CLI override sets data_dir on ServerConfig
        let mut config = ServerConfig::default();
        let custom_dir = std::path::Path::new("/var/lib/cflx");

        config.apply_cli_overrides(None, None, None, None, Some(custom_dir));

        assert_eq!(
            config.data_dir,
            std::path::PathBuf::from("/var/lib/cflx"),
            "data_dir should be overridden by CLI --data-dir option"
        );
    }

    #[test]
    fn test_server_config_apply_cli_overrides_data_dir_not_set_uses_default() {
        // When --data-dir is not provided, data_dir remains the default value
        let mut config = ServerConfig::default();
        let default_data_dir = config.data_dir.clone();

        config.apply_cli_overrides(None, None, None, None, None);

        assert_eq!(
            config.data_dir, default_data_dir,
            "data_dir should remain the default when CLI --data-dir is not specified"
        );
    }

    #[test]
    fn test_server_config_apply_cli_overrides_data_dir_with_other_overrides() {
        // Verify that data_dir override works correctly alongside other CLI overrides
        let mut config = ServerConfig::default();
        let custom_dir = std::path::Path::new("/tmp/cflx-server");

        config.apply_cli_overrides(
            Some("0.0.0.0"),
            Some(8080),
            Some("my-token"),
            Some(10),
            Some(custom_dir),
        );

        assert_eq!(config.bind, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.auth.mode, ServerAuthMode::BearerToken);
        assert_eq!(config.auth.token, Some("my-token".to_string()));
        assert_eq!(config.max_concurrent_total, 10);
        assert_eq!(
            config.data_dir,
            std::path::PathBuf::from("/tmp/cflx-server"),
            "data_dir should be overridden when provided alongside other CLI overrides"
        );
    }

    // ── server.resolve_command deprecation tests ──

    #[test]
    fn test_server_config_validate_rejects_deprecated_resolve_command() {
        // Verify that setting server.resolve_command causes a configuration error
        let config = ServerConfig {
            resolve_command: Some("echo resolve".to_string()),
            ..Default::default()
        };

        let result = config.validate();
        assert!(
            result.is_err(),
            "validate() should return Err when server.resolve_command is set"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("server.resolve_command"),
            "Error message should mention 'server.resolve_command', got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("top-level `resolve_command`"),
            "Error message should mention the top-level resolve_command as alternative, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_server_config_validate_accepts_config_without_resolve_command() {
        // Verify that server.resolve_command = None passes validation (loopback bind)
        let config = ServerConfig::default();
        assert!(
            config.resolve_command.is_none(),
            "Default ServerConfig should not have resolve_command set"
        );
        let result = config.validate();
        assert!(
            result.is_ok(),
            "validate() should succeed when server.resolve_command is not set"
        );
    }

    #[test]
    fn test_parse_server_config_with_resolve_command_is_parsed_but_rejected_at_validate() {
        // Verify that server.resolve_command in config JSON is deserialized
        // but then rejected by validate()
        let jsonc = r#"{
            "server": {
                "resolve_command": "echo resolve"
            }
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let server_config = config.server.unwrap_or_default();
        assert_eq!(
            server_config.resolve_command,
            Some("echo resolve".to_string()),
            "server.resolve_command should be deserialized from JSON"
        );
        // But validate() should reject it
        let result = server_config.validate();
        assert!(
            result.is_err(),
            "validate() should reject server.resolve_command"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("server.resolve_command"),
            "Error should mention server.resolve_command, got: {}",
            err_msg
        );
    }

    // === Characterization tests: config loading priority (task 1.1) ===

    /// Characterizes the full merge priority order used by OrchestratorConfig::load().
    /// Order (lowest to highest): platform default → XDG default → XDG env → project → custom.
    /// A later merge() call wins when it supplies a Some value; None never overrides Some.
    #[test]
    fn test_characterize_merge_priority_full_order() {
        // Simulate each config layer using merge() in the same order as load()
        let platform = OrchestratorConfig {
            apply_command: Some("platform-apply".to_string()),
            archive_command: Some("platform-archive".to_string()),
            analyze_command: Some("platform-analyze".to_string()),
            acceptance_command: Some("platform-acceptance".to_string()),
            resolve_command: Some("platform-resolve".to_string()),
            ..Default::default()
        };
        let xdg_default = OrchestratorConfig {
            apply_command: Some("xdg-default-apply".to_string()),
            ..Default::default()
        };
        let xdg_env = OrchestratorConfig {
            apply_command: Some("xdg-env-apply".to_string()),
            archive_command: Some("xdg-env-archive".to_string()),
            ..Default::default()
        };
        let project = OrchestratorConfig {
            apply_command: Some("project-apply".to_string()),
            ..Default::default()
        };
        let custom = OrchestratorConfig {
            apply_command: Some("custom-apply".to_string()),
            ..Default::default()
        };

        let mut merged = OrchestratorConfig::default();
        merged.merge(platform);
        merged.merge(xdg_default);
        merged.merge(xdg_env);
        merged.merge(project);
        merged.merge(custom);

        // custom wins for apply_command (set at every layer; last write wins)
        assert_eq!(merged.apply_command, Some("custom-apply".to_string()));
        // xdg_env wins for archive_command (project and custom did not set it)
        assert_eq!(merged.archive_command, Some("xdg-env-archive".to_string()));
        // only platform set analyze/acceptance/resolve
        assert_eq!(merged.analyze_command, Some("platform-analyze".to_string()));
        assert_eq!(
            merged.acceptance_command,
            Some("platform-acceptance".to_string())
        );
        assert_eq!(merged.resolve_command, Some("platform-resolve".to_string()));
    }

    /// Characterizes that a None value in a higher-priority config does NOT overwrite a Some
    /// value from a lower-priority config.
    #[test]
    fn test_characterize_none_does_not_override_some() {
        let mut base = OrchestratorConfig {
            apply_command: Some("base-apply".to_string()),
            max_iterations: Some(42),
            ..Default::default()
        };
        // Higher-priority config that leaves these fields as None
        base.merge(OrchestratorConfig::default());

        assert_eq!(base.apply_command, Some("base-apply".to_string()));
        assert_eq!(base.max_iterations, Some(42));
    }

    /// Characterizes that a custom config (highest priority) overrides a project config while
    /// project config overrides a global config for the same field.
    #[test]
    fn test_characterize_custom_beats_project_beats_global() {
        let global = OrchestratorConfig {
            apply_command: Some("global-apply".to_string()),
            archive_command: Some("global-archive".to_string()),
            ..Default::default()
        };
        let project = OrchestratorConfig {
            apply_command: Some("project-apply".to_string()),
            // archive_command deliberately absent
            ..Default::default()
        };
        let custom = OrchestratorConfig {
            apply_command: Some("custom-apply".to_string()),
            // archive_command deliberately absent
            ..Default::default()
        };

        let mut merged = OrchestratorConfig::default();
        merged.merge(global);
        merged.merge(project);
        merged.merge(custom);

        // custom > project > global for apply_command
        assert_eq!(merged.apply_command, Some("custom-apply".to_string()));
        // project and custom did not set archive_command → global value is preserved
        assert_eq!(merged.archive_command, Some("global-archive".to_string()));
    }

    // === Characterization tests: JSONC deserialization and defaults (task 1.2) ===

    /// Characterizes default numeric/bool values exposed through getter methods.
    #[test]
    fn test_characterize_default_getter_values() {
        let config = OrchestratorConfig::default();

        assert_eq!(config.get_max_iterations(), 50);
        assert_eq!(config.get_max_concurrent_workspaces(), 3);
        assert_eq!(config.get_acceptance_max_continues(), 10);
        assert_eq!(config.get_command_inactivity_timeout_secs(), 900);
        assert_eq!(config.get_command_inactivity_kill_grace_secs(), 5);
        assert_eq!(config.get_command_inactivity_timeout_max_retries(), 3);
        assert!(config.use_llm_analysis());
        assert!(!config.get_parallel_mode());
        assert!(config.get_stream_json_textify());
        assert!(config.get_command_strict_process_cleanup());
        assert_eq!(config.get_vcs_backend(), VcsBackend::Auto);
    }

    /// Characterizes that LoggingConfig defaults are applied when the field is absent from JSONC.
    #[test]
    fn test_characterize_logging_defaults_when_absent() {
        let config = OrchestratorConfig::parse_jsonc("{}").unwrap();
        // logging is None in the raw struct …
        assert!(config.logging.is_none());
        // … but get_logging() materialises defaults
        let logging = config.get_logging();
        assert!(logging.suppress_repetitive_debug);
        assert_eq!(logging.summary_interval_secs, 60);
    }

    /// Characterizes that JSONC comments (// and /* */) and trailing commas are accepted.
    #[test]
    fn test_characterize_jsonc_comment_styles_and_trailing_comma() {
        let jsonc = r#"{
            // single-line comment
            "apply_command": "cmd-apply",  // inline comment
            /* multi-line
               comment */
            "archive_command": "cmd-archive",
            "analyze_command": "cmd-analyze", // trailing comma on next line:
            "acceptance_command": "cmd-acceptance",
            "resolve_command": "cmd-resolve",
        }"#;

        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        assert_eq!(config.apply_command, Some("cmd-apply".to_string()));
        assert_eq!(config.archive_command, Some("cmd-archive".to_string()));
        assert_eq!(config.analyze_command, Some("cmd-analyze".to_string()));
        assert_eq!(
            config.acceptance_command,
            Some("cmd-acceptance".to_string())
        );
        assert_eq!(config.resolve_command, Some("cmd-resolve".to_string()));
    }

    /// Characterizes that absent optional fields stay None after parsing, while getters
    /// return the hardcoded defaults.
    #[test]
    fn test_characterize_absent_optional_fields_stay_none() {
        let config = OrchestratorConfig::parse_jsonc(r#"{"apply_command": "x"}"#).unwrap();

        // Raw optional fields are None
        assert!(config.archive_command.is_none());
        assert!(config.logging.is_none());
        assert!(config.stall_detection.is_none());
        assert!(config.max_iterations.is_none());
        assert!(config.parallel_mode.is_none());

        // But getters resolve to their defaults
        assert_eq!(config.get_max_iterations(), 50);
        assert!(!config.get_parallel_mode());
    }

    #[test]
    fn test_load_server_config_and_resolve_command_includes_proposal_session_config() {
        let jsonc = r#"{
            "server": {
                "bind": "127.0.0.1",
                "port": 41234
            },
            "resolve_command": "echo resolve",
            "proposal_session": {
                "transport_command": "custom-opencode",
                "transport_args": ["--foo", "bar"],
                "transport_env": {
                    "FOO": "bar"
                },
                "session_inactivity_timeout_secs": 42
            }
        }"#;

        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();
        let server_config = config.server.clone().unwrap();
        let resolve_command = config.resolve_command.clone();
        let proposal_session = config.proposal_session.clone().unwrap();

        assert_eq!(server_config.port, 41234);
        assert_eq!(resolve_command.as_deref(), Some("echo resolve"));
        assert_eq!(proposal_session.transport_command, "custom-opencode");
        assert_eq!(proposal_session.transport_args, vec!["--foo", "bar"]);
        assert_eq!(
            proposal_session
                .transport_env
                .get("FOO")
                .map(String::as_str),
            Some("bar")
        );
        assert_eq!(proposal_session.session_inactivity_timeout_secs, 42);
    }
}
