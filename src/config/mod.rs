//! Configuration module for OpenSpec Orchestrator.
//!
//! Supports JSONC format (JSON with Comments) for configuration files.
//! Configuration is loaded with the following priority:
//! 1. Project config: `.openspec-orchestrator.jsonc`
//! 2. Global config: `~/.config/openspec-orchestrator/config.jsonc`
//! 3. Default values (OpenCode-based commands)
//!
//! # Module Structure
//!
//! - `defaults` - Default values and path constants
//! - `expand` - Placeholder expansion utilities
//! - `jsonc` - JSONC parser (reusable by other modules)

pub mod defaults;
pub mod expand;
pub mod jsonc;

use crate::error::{OrchestratorError, Result};
use crate::hooks::HooksConfig;
use crate::vcs::VcsBackend;
use defaults::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

fn default_suppress_repetitive_debug() -> bool {
    DEFAULT_SUPPRESS_REPETITIVE_DEBUG
}

fn default_log_summary_interval_secs() -> u64 {
    DEFAULT_LOG_SUMMARY_INTERVAL_SECS
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoggingConfig {
    /// Suppress repetitive debug logs when state has not changed.
    #[serde(default = "default_suppress_repetitive_debug")]
    pub suppress_repetitive_debug: bool,

    /// Interval in seconds for emitting status summaries (0 disables summaries).
    #[serde(default = "default_log_summary_interval_secs")]
    pub summary_interval_secs: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            suppress_repetitive_debug: DEFAULT_SUPPRESS_REPETITIVE_DEBUG,
            summary_interval_secs: DEFAULT_LOG_SUMMARY_INTERVAL_SECS,
        }
    }
}

/// Orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorConfig {
    /// Command template for applying changes.
    /// Supports `{change_id}` placeholder.
    #[serde(default)]
    pub apply_command: Option<String>,

    /// Command template for archiving changes.
    /// Supports `{change_id}` placeholder.
    #[serde(default)]
    pub archive_command: Option<String>,

    /// Command template for dependency analysis.
    /// Supports `{prompt}` placeholder.
    #[serde(default)]
    pub analyze_command: Option<String>,

    /// System prompt for apply command.
    /// Injected into the `{prompt}` placeholder in apply_command.
    #[serde(default)]
    pub apply_prompt: Option<String>,

    /// System prompt for archive command.
    /// Injected into the `{prompt}` placeholder in archive_command.
    #[serde(default)]
    pub archive_prompt: Option<String>,

    /// Hook configurations for various orchestration stages.
    /// All hooks are optional.
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Logging configuration for TUI debug output.
    #[serde(default)]
    pub logging: Option<LoggingConfig>,

    /// Delay between completion check retries in milliseconds.
    /// Default: 500ms
    #[serde(default)]
    pub completion_check_delay_ms: Option<u64>,

    /// Maximum number of retries for completion check.
    /// Default: 3
    #[serde(default)]
    pub completion_check_max_retries: Option<u32>,

    /// Maximum number of iterations for the orchestration loop.
    /// Set to 0 to disable the limit.
    /// Default: 50
    #[serde(default)]
    pub max_iterations: Option<u32>,

    /// Enable parallel execution mode (requires git).
    /// Default: false (off by default)
    #[serde(default)]
    pub parallel_mode: Option<bool>,

    /// Maximum number of concurrent workspaces for parallel execution.
    /// Default: 3
    #[serde(default)]
    pub max_concurrent_workspaces: Option<usize>,

    /// Base directory for creating workspaces.
    /// Default: system temp directory
    #[serde(default)]
    pub workspace_base_dir: Option<String>,

    /// Command template for conflict resolution.
    /// Supports `{conflict_info}` placeholder.
    /// If not set, uses automatic AI-based resolution.
    #[serde(default)]
    pub resolve_command: Option<String>,

    /// Enable LLM-based analysis for parallelization.
    /// When true (default), uses analyze_command to determine dependencies between changes.
    /// When false, skips analysis and runs all changes in parallel (no dependency inference).
    #[serde(default)]
    pub use_llm_analysis: Option<bool>,

    /// VCS backend to use for parallel execution.
    /// Options: "auto" (default) or "git"
    /// - auto: Automatically detect Git repository
    /// - git: Use git worktrees (warns if working directory has changes)
    #[serde(default)]
    pub vcs_backend: Option<VcsBackend>,

    /// Command template for proposing new changes from TUI.
    /// Supports `{proposal}` placeholder for the proposal text.
    /// Example: "opencode run '{proposal}'"
    #[serde(default)]
    pub propose_command: Option<String>,
}

impl OrchestratorConfig {
    /// Create a new empty configuration
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the apply command, falling back to default if not set
    pub fn get_apply_command(&self) -> &str {
        self.apply_command
            .as_deref()
            .unwrap_or(DEFAULT_APPLY_COMMAND)
    }

    /// Get the archive command, falling back to default if not set
    pub fn get_archive_command(&self) -> &str {
        self.archive_command
            .as_deref()
            .unwrap_or(DEFAULT_ARCHIVE_COMMAND)
    }

    /// Get the analyze command, falling back to default if not set
    pub fn get_analyze_command(&self) -> &str {
        self.analyze_command
            .as_deref()
            .unwrap_or(DEFAULT_ANALYZE_COMMAND)
    }

    /// Get the apply prompt, falling back to default if not set
    pub fn get_apply_prompt(&self) -> &str {
        self.apply_prompt.as_deref().unwrap_or(DEFAULT_APPLY_PROMPT)
    }

    /// Get the archive prompt, falling back to default if not set
    pub fn get_archive_prompt(&self) -> &str {
        self.archive_prompt
            .as_deref()
            .unwrap_or(DEFAULT_ARCHIVE_PROMPT)
    }

    /// Get the hooks configuration, returning default (empty) if not set
    pub fn get_hooks(&self) -> HooksConfig {
        self.hooks.clone().unwrap_or_default()
    }

    /// Get logging configuration, returning defaults if not set.
    pub fn get_logging(&self) -> LoggingConfig {
        self.logging.clone().unwrap_or_default()
    }

    /// Get the maximum iterations limit.
    /// Returns 0 if explicitly set to 0 (disabled), otherwise returns configured or default value.
    /// A value of 0 means no limit.
    pub fn get_max_iterations(&self) -> u32 {
        self.max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS)
    }

    /// Get whether parallel mode is enabled.
    /// Default: false (off by default)
    #[allow(dead_code)]
    pub fn get_parallel_mode(&self) -> bool {
        self.parallel_mode.unwrap_or(false)
    }

    /// Get the maximum concurrent workspaces limit.
    /// Default: 3
    pub fn get_max_concurrent_workspaces(&self) -> usize {
        self.max_concurrent_workspaces
            .unwrap_or(DEFAULT_MAX_CONCURRENT_WORKSPACES)
    }

    /// Get the workspace base directory.
    /// Returns None if using system temp directory.
    pub fn get_workspace_base_dir(&self) -> Option<&str> {
        self.workspace_base_dir.as_deref().filter(|s| !s.is_empty())
    }

    /// Get the resolve command for conflict resolution, falling back to default if not set.
    pub fn get_resolve_command(&self) -> &str {
        self.resolve_command
            .as_deref()
            .unwrap_or(DEFAULT_RESOLVE_COMMAND)
    }

    /// Check if LLM-based analysis is enabled for parallelization.
    /// Default: true (use LLM to analyze dependencies between changes)
    /// Set to false to skip LLM analysis and run all changes in parallel.
    pub fn use_llm_analysis(&self) -> bool {
        self.use_llm_analysis.unwrap_or(true)
    }

    /// Get the VCS backend to use for parallel execution.
    /// Default: Auto (automatically detect Git)
    pub fn get_vcs_backend(&self) -> VcsBackend {
        self.vcs_backend.unwrap_or(VcsBackend::Auto)
    }

    /// Get the propose command template, if configured.
    /// Returns None if not set (propose feature is disabled).
    pub fn get_propose_command(&self) -> Option<&str> {
        self.propose_command.as_deref()
    }

    /// Expand `{proposal}` placeholder in a command template.
    pub fn expand_proposal(template: &str, proposal: &str) -> String {
        expand::expand_proposal(template, proposal)
    }

    /// Expand `{conflict_files}` placeholder in a command template
    #[allow(dead_code)]
    pub fn expand_conflict_files(template: &str, conflict_files: &str) -> String {
        expand::expand_conflict_files(template, conflict_files)
    }

    /// Expand `{change_id}` placeholder in a command template
    pub fn expand_change_id(template: &str, change_id: &str) -> String {
        expand::expand_change_id(template, change_id)
    }

    /// Expand `{prompt}` placeholder in a command template
    pub fn expand_prompt(template: &str, prompt: &str) -> String {
        expand::expand_prompt(template, prompt)
    }

    /// Load configuration from a JSONC file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        Self::parse_jsonc(&content)
    }

    /// Parse JSONC content (JSON with Comments)
    pub fn parse_jsonc(content: &str) -> Result<Self> {
        jsonc::parse(content)
    }

    /// Load configuration with priority:
    /// 1. Custom config path (if provided)
    /// 2. Project config (`.openspec-orchestrator.jsonc`)
    /// 3. Global config (`~/.config/openspec-orchestrator/config.jsonc`)
    /// 4. Default configuration
    pub fn load(custom_path: Option<&Path>) -> Result<Self> {
        // 1. Custom config path
        if let Some(path) = custom_path {
            info!("Loading config from custom path: {:?}", path);
            return Self::load_from_file(path);
        }

        // 2. Project config
        let project_config_path = PathBuf::from(PROJECT_CONFIG_FILE);
        if project_config_path.exists() {
            info!("Loading project config from: {:?}", project_config_path);
            return Self::load_from_file(&project_config_path);
        }

        // 3. Global config
        if let Some(global_path) = get_global_config_path() {
            if global_path.exists() {
                info!("Loading global config from: {:?}", global_path);
                return Self::load_from_file(&global_path);
            }
        }

        // 4. Default configuration
        debug!("No config file found, using defaults");
        Ok(Self::default())
    }
}

/// Get the path to the global configuration file
///
/// Returns `~/.config/openspec-orchestrator/config.jsonc` on Unix-like systems.
pub fn get_global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|config_dir| config_dir.join(GLOBAL_CONFIG_DIR).join(GLOBAL_CONFIG_FILE))
}

// Re-export commonly used items for convenience
pub use defaults::{
    DEFAULT_ANALYZE_COMMAND, DEFAULT_APPLY_COMMAND, DEFAULT_APPLY_PROMPT, DEFAULT_ARCHIVE_COMMAND,
    DEFAULT_ARCHIVE_PROMPT, DEFAULT_MAX_CONCURRENT_WORKSPACES, DEFAULT_MAX_ITERATIONS,
    GLOBAL_CONFIG_DIR, GLOBAL_CONFIG_FILE, PROJECT_CONFIG_FILE,
};

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
    fn test_get_commands_with_defaults() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.get_apply_command(), DEFAULT_APPLY_COMMAND);
        assert_eq!(config.get_archive_command(), DEFAULT_ARCHIVE_COMMAND);
        assert_eq!(config.get_analyze_command(), DEFAULT_ANALYZE_COMMAND);
    }

    #[test]
    fn test_get_commands_with_custom_values() {
        let config = OrchestratorConfig {
            apply_command: Some("custom apply {change_id}".to_string()),
            archive_command: Some("custom archive {change_id}".to_string()),
            analyze_command: Some("custom analyze '{prompt}'".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_apply_command(), "custom apply {change_id}");
        assert_eq!(config.get_archive_command(), "custom archive {change_id}");
        assert_eq!(config.get_analyze_command(), "custom analyze '{prompt}'");
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
            "archive_command": "codex run 'openspec-archive {change_id}'",

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
            Some("codex run 'openspec-archive {change_id}'".to_string())
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
    fn test_partial_config_with_fallback() {
        let jsonc = r#"{
            "apply_command": "custom apply {change_id}"
        }"#;
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        // Custom value should be used
        assert_eq!(config.get_apply_command(), "custom apply {change_id}");

        // Defaults should be used for missing values
        assert_eq!(config.get_archive_command(), DEFAULT_ARCHIVE_COMMAND);
        assert_eq!(config.get_analyze_command(), DEFAULT_ANALYZE_COMMAND);
    }

    #[test]
    fn test_empty_config_uses_all_defaults() {
        let jsonc = "{}";
        let config = OrchestratorConfig::parse_jsonc(jsonc).unwrap();

        assert_eq!(config.get_apply_command(), DEFAULT_APPLY_COMMAND);
        assert_eq!(config.get_archive_command(), DEFAULT_ARCHIVE_COMMAND);
        assert_eq!(config.get_analyze_command(), DEFAULT_ANALYZE_COMMAND);
    }

    #[test]
    fn test_load_from_custom_path() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary config file
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"{{"apply_command": "custom-agent apply {{change_id}}"}}"#
        )
        .unwrap();

        // Load from custom path
        let config = OrchestratorConfig::load(Some(temp_file.path())).unwrap();

        assert_eq!(config.get_apply_command(), "custom-agent apply {change_id}");
    }

    #[test]
    fn test_load_returns_default_when_no_config_exists() {
        use std::env;
        use tempfile::TempDir;

        // Create a temporary directory with no config files
        let temp_dir = TempDir::new().unwrap();

        // Save current directory and change to temp dir
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Load config (should return defaults since no config file exists)
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();

        // Should use default values
        assert_eq!(config.get_apply_command(), DEFAULT_APPLY_COMMAND);
        assert_eq!(config.get_archive_command(), DEFAULT_ARCHIVE_COMMAND);
        assert_eq!(config.get_analyze_command(), DEFAULT_ANALYZE_COMMAND);
    }

    #[test]
    fn test_load_project_config_takes_priority() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();

        // Create project config file
        let project_config_path = temp_dir.path().join(PROJECT_CONFIG_FILE);
        fs::write(
            &project_config_path,
            r#"{"apply_command": "project-agent apply {change_id}"}"#,
        )
        .unwrap();

        // Save current directory and change to temp dir
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        // Load config (should use project config)
        let config = OrchestratorConfig::load(None).unwrap();

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();

        // Project config should be used
        assert_eq!(
            config.get_apply_command(),
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
                "on_queue_remove": "echo queue_remove",
                "on_approve": "echo approve",
                "on_unapprove": "echo unapprove"
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
        assert!(hooks.get(HookType::OnApprove).is_some());
        assert!(hooks.get(HookType::OnUnapprove).is_some());
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
    fn test_parallel_mode_defaults_to_false() {
        let config = OrchestratorConfig::default();
        assert!(!config.get_parallel_mode());
    }

    #[test]
    fn test_parallel_mode_can_be_enabled() {
        let config = OrchestratorConfig {
            parallel_mode: Some(true),
            ..Default::default()
        };
        assert!(config.get_parallel_mode());
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
    fn test_resolve_command_has_default() {
        let config = OrchestratorConfig::default();
        // Should have a default resolve command
        assert!(!config.get_resolve_command().is_empty());
    }

    #[test]
    fn test_resolve_command_can_be_configured() {
        let config = OrchestratorConfig {
            resolve_command: Some("custom-resolver {conflict_files}".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.get_resolve_command(),
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
}
