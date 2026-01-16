//! Default values and constants for orchestrator configuration.

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

/// Default resolve command template (OpenCode)
/// Supports `{prompt}` placeholder for the resolve prompt
pub const DEFAULT_RESOLVE_COMMAND: &str = "opencode run {prompt}";

/// Default analyze command template (OpenCode)
pub const DEFAULT_ANALYZE_COMMAND: &str = "opencode run --format json {prompt}";

/// Default prompt for apply command - includes path context.
/// The hardcoded system prompt in agent.rs is always appended.
pub const DEFAULT_APPLY_PROMPT: &str = r#"
<system-context>
IMPORTANT: You are running in the repository root directory.
The change you are working on is located at: openspec/changes/{change_id}/
All file paths should be relative to the repository root.
</system-context>
"#;

/// Default prompt for archive command - includes path context
pub const DEFAULT_ARCHIVE_PROMPT: &str = r#"
<system-context>
IMPORTANT: You are running in the repository root directory.
To archive the change, move the directory from:
  openspec/changes/{change_id}/
to:
  openspec/specs/{change_id}/

All file paths should be relative to the repository root.
</system-context>
"#;

/// Default maximum iterations for the orchestration loop
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

/// Default maximum concurrent workspaces for parallel execution
pub const DEFAULT_MAX_CONCURRENT_WORKSPACES: usize = 3;

/// Default workspace base directory (uses system temp)
#[allow(dead_code)]
pub const DEFAULT_WORKSPACE_BASE_DIR: &str = "";

/// Default suppression for repetitive debug logs
pub const DEFAULT_SUPPRESS_REPETITIVE_DEBUG: bool = true;

/// Default interval (seconds) for summary log output
pub const DEFAULT_LOG_SUMMARY_INTERVAL_SECS: u64 = 60;

/// Default enablement for stall detection
pub const DEFAULT_STALL_DETECTION_ENABLED: bool = true;

/// Default threshold for consecutive empty WIP commits before stalling
pub const DEFAULT_STALL_DETECTION_THRESHOLD: u32 = 3;

/// Default delay between command executions (milliseconds)
pub const DEFAULT_STAGGER_DELAY_MS: u64 = 2000;

/// Default maximum number of retries for commands
pub const DEFAULT_MAX_RETRIES: u32 = 2;

/// Default delay between retries (milliseconds)
pub const DEFAULT_RETRY_DELAY_MS: u64 = 5000;

/// Default threshold for retry based on execution duration (seconds)
pub const DEFAULT_RETRY_IF_DURATION_UNDER_SECS: u64 = 5;

/// Default error patterns that trigger automatic retry
pub fn default_retry_patterns() -> Vec<String> {
    vec![
        // Module resolution errors
        r"Cannot find module".to_string(),
        r"ResolveMessage:".to_string(),
        // npm/bun registry errors
        r"ENOTFOUND registry\.npmjs\.org".to_string(),
        r"ETIMEDOUT.*registry".to_string(),
        // File lock errors
        r"EBADF.*lock".to_string(),
        r"Lock acquisition failed".to_string(),
    ]
}

/// Default enablement for error circuit breaker
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_ENABLED: bool = true;

/// Default threshold for consecutive same errors before opening circuit
pub const DEFAULT_ERROR_CIRCUIT_BREAKER_THRESHOLD: usize = 5;
