//! Configuration module for OpenSpec Orchestrator.
//!
//! Supports JSONC format (JSON with Comments) for configuration files.
//! Configuration is loaded with the following priority:
//! 1. Project config: `.openspec-orchestrator.jsonc`
//! 2. Global config: `~/.config/openspec-orchestrator/config.jsonc`
//! 3. Default values (OpenCode-based commands)

use crate::error::{OrchestratorError, Result};
use crate::hooks::HooksConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Project-level configuration file name
pub const PROJECT_CONFIG_FILE: &str = ".openspec-orchestrator.jsonc";

/// Global configuration directory name
pub const GLOBAL_CONFIG_DIR: &str = "openspec-orchestrator";

/// Global configuration file name within the config directory
pub const GLOBAL_CONFIG_FILE: &str = "config.jsonc";

/// Default apply command template (OpenCode)
pub const DEFAULT_APPLY_COMMAND: &str = "opencode run '/openspec-apply {change_id}'";

/// Default archive command template (OpenCode)
pub const DEFAULT_ARCHIVE_COMMAND: &str = "opencode run '/openspec-archive {change_id}'";

/// Default analyze command template (OpenCode)
pub const DEFAULT_ANALYZE_COMMAND: &str = "opencode run --format json '{prompt}'";

/// Default delay between completion check retries in milliseconds
pub const DEFAULT_COMPLETION_CHECK_DELAY_MS: u64 = 500;

/// Default maximum number of retries for completion check
pub const DEFAULT_COMPLETION_CHECK_MAX_RETRIES: u32 = 3;

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

    /// Hook configurations for various orchestration stages.
    /// All hooks are optional.
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Delay between completion check retries in milliseconds.
    /// Default: 500ms
    #[serde(default)]
    pub completion_check_delay_ms: Option<u64>,

    /// Maximum number of retries for completion check.
    /// Default: 3
    #[serde(default)]
    pub completion_check_max_retries: Option<u32>,
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

    /// Get the hooks configuration, returning default (empty) if not set
    pub fn get_hooks(&self) -> HooksConfig {
        self.hooks.clone().unwrap_or_default()
    }

    /// Get the completion check delay in milliseconds
    pub fn get_completion_check_delay_ms(&self) -> u64 {
        self.completion_check_delay_ms
            .unwrap_or(DEFAULT_COMPLETION_CHECK_DELAY_MS)
    }

    /// Get the maximum number of completion check retries
    pub fn get_completion_check_max_retries(&self) -> u32 {
        self.completion_check_max_retries
            .unwrap_or(DEFAULT_COMPLETION_CHECK_MAX_RETRIES)
    }

    /// Expand `{change_id}` placeholder in a command template
    pub fn expand_change_id(template: &str, change_id: &str) -> String {
        template.replace("{change_id}", change_id)
    }

    /// Expand `{prompt}` placeholder in a command template
    pub fn expand_prompt(template: &str, prompt: &str) -> String {
        template.replace("{prompt}", prompt)
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
        let json = strip_jsonc_features(content);
        serde_json::from_str(&json)
            .map_err(|e| OrchestratorError::ConfigParse(format!("Failed to parse config: {}", e)))
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

/// Strip JSONC features (comments and trailing commas) from content
///
/// This function handles:
/// - Single-line comments (`// ...`)
/// - Multi-line comments (`/* ... */`)
/// - Trailing commas before `]` or `}`
fn strip_jsonc_features(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if in_string {
            result.push(c);
            if c == '\\' {
                escape_next = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                result.push(c);
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    // Single-line comment: skip until end of line
                    chars.next(); // consume second '/'
                    while let Some(&next) = chars.peek() {
                        if next == '\n' {
                            break;
                        }
                        chars.next();
                    }
                } else if chars.peek() == Some(&'*') {
                    // Multi-line comment: skip until '*/'
                    chars.next(); // consume '*'
                    while let Some(next) = chars.next() {
                        if next == '*' && chars.peek() == Some(&'/') {
                            chars.next(); // consume '/'
                            break;
                        }
                    }
                } else {
                    result.push(c);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }

    // Remove trailing commas before ] or }
    remove_trailing_commas(&result)
}

/// Remove trailing commas before `]` or `}`
fn remove_trailing_commas(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == ',' {
            // Look ahead, skipping whitespace, for ] or }
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == ']' || chars[j] == '}') {
                // Skip the comma (trailing comma)
                i += 1;
                continue;
            }
        }

        result.push(c);
        i += 1;
    }

    result
}

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
    fn test_strip_jsonc_preserves_url_in_string() {
        let jsonc = r#"{"url": "https://example.com/path"}"#;
        let stripped = strip_jsonc_features(jsonc);
        assert!(stripped.contains("https://example.com/path"));
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
}
