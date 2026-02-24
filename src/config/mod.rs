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

fn default_stall_detection_enabled() -> bool {
    DEFAULT_STALL_DETECTION_ENABLED
}

fn default_stall_detection_threshold() -> u32 {
    DEFAULT_STALL_DETECTION_THRESHOLD
}

fn default_error_circuit_breaker_enabled() -> bool {
    DEFAULT_ERROR_CIRCUIT_BREAKER_ENABLED
}

fn default_error_circuit_breaker_threshold() -> usize {
    DEFAULT_ERROR_CIRCUIT_BREAKER_THRESHOLD
}

fn default_merge_stall_detection_enabled() -> bool {
    defaults::DEFAULT_MERGE_STALL_DETECTION_ENABLED
}

fn default_merge_stall_threshold_minutes() -> u64 {
    defaults::DEFAULT_MERGE_STALL_THRESHOLD_MINUTES
}

fn default_merge_stall_check_interval_seconds() -> u64 {
    defaults::DEFAULT_MERGE_STALL_CHECK_INTERVAL_SECONDS
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

/// Stall detection configuration for empty WIP commits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StallDetectionConfig {
    /// Enable stall detection based on consecutive empty WIP commits.
    #[serde(default = "default_stall_detection_enabled")]
    pub enabled: bool,
    /// Consecutive empty commit threshold before stalling.
    #[serde(default = "default_stall_detection_threshold")]
    pub threshold: u32,
}

/// Error circuit breaker configuration for detecting repeated failures.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorCircuitBreakerConfig {
    /// Enable circuit breaker for same error detection.
    #[serde(default = "default_error_circuit_breaker_enabled")]
    pub enabled: bool,
    /// Consecutive same error threshold before opening circuit.
    #[serde(default = "default_error_circuit_breaker_threshold")]
    pub threshold: usize,
}

impl Default for ErrorCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: default_error_circuit_breaker_enabled(),
            threshold: default_error_circuit_breaker_threshold(),
        }
    }
}

impl Default for StallDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_STALL_DETECTION_ENABLED,
            threshold: DEFAULT_STALL_DETECTION_THRESHOLD,
        }
    }
}

/// Merge stall detection configuration for monitoring merge progress.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MergeStallDetectionConfig {
    /// Enable merge stall detection based on merge commit inactivity.
    #[serde(default = "default_merge_stall_detection_enabled")]
    pub enabled: bool,
    /// Threshold in minutes for merge inactivity before triggering stall.
    #[serde(default = "default_merge_stall_threshold_minutes")]
    pub threshold_minutes: u64,
    /// Check interval in seconds for monitoring merge progress.
    #[serde(default = "default_merge_stall_check_interval_seconds")]
    pub check_interval_seconds: u64,
}

impl Default for MergeStallDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: defaults::DEFAULT_MERGE_STALL_DETECTION_ENABLED,
            threshold_minutes: defaults::DEFAULT_MERGE_STALL_THRESHOLD_MINUTES,
            check_interval_seconds: defaults::DEFAULT_MERGE_STALL_CHECK_INTERVAL_SECONDS,
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

    /// Command template for acceptance testing after apply.
    /// Supports `{change_id}` and `{prompt}` placeholders.
    #[serde(default)]
    pub acceptance_command: Option<String>,

    /// System prompt for apply command.
    /// Injected into the `{prompt}` placeholder in apply_command.
    #[serde(default)]
    pub apply_prompt: Option<String>,

    /// System prompt for acceptance command.
    /// Injected into the `{prompt}` placeholder in acceptance_command.
    #[serde(default)]
    pub acceptance_prompt: Option<String>,

    /// Controls how the acceptance `{prompt}` is constructed.
    /// - full: DEPRECATED - now behaves identically to context_only (no embedded system prompt)
    /// - context_only: only include change metadata + diff/history context
    ///
    /// The "full" mode is now deprecated and unified with "context_only".
    /// All acceptance instructions must come from the command template.
    #[serde(default)]
    pub acceptance_prompt_mode: Option<AcceptancePromptMode>,

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

    /// Stall detection configuration (empty WIP commit detection).
    #[serde(default)]
    pub stall_detection: Option<StallDetectionConfig>,

    /// Error circuit breaker configuration (same error detection).
    #[serde(default)]
    pub error_circuit_breaker: Option<ErrorCircuitBreakerConfig>,

    /// Merge stall detection configuration (merge commit inactivity).
    #[serde(default)]
    pub merge_stall_detection: Option<MergeStallDetectionConfig>,

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

    /// Command template for merge/conflict resolution.
    /// Supports `{prompt}` placeholder.
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

    /// Command template for creating a proposal worktree from TUI.
    /// Supports `{workspace_dir}` and `{repo_root}` placeholders.
    #[serde(default)]
    pub worktree_command: Option<String>,

    /// Delay between command executions (milliseconds).
    /// Default: 2000ms (2 seconds)
    #[serde(default)]
    pub command_queue_stagger_delay_ms: Option<u64>,

    /// Maximum number of retries for commands.
    /// Default: 2
    #[serde(default)]
    pub command_queue_max_retries: Option<u32>,

    /// Delay between retries (milliseconds).
    /// Default: 5000ms (5 seconds)
    #[serde(default)]
    pub command_queue_retry_delay_ms: Option<u64>,

    /// Error patterns that trigger automatic retry (regex).
    /// Default: module resolution, registry, and lock errors
    #[serde(default)]
    pub command_queue_retry_patterns: Option<Vec<String>>,

    /// Retry if execution duration is under this threshold (seconds).
    /// Default: 5 seconds
    #[serde(default)]
    pub command_queue_retry_if_duration_under_secs: Option<u64>,

    /// Maximum number of acceptance CONTINUE retries before treating as FAIL.
    /// Default: 2
    #[serde(default)]
    pub acceptance_max_continues: Option<u32>,

    /// Inactivity timeout for commands (seconds).
    /// 0 = disabled
    /// Default: 900 (15 minutes)
    #[serde(default)]
    pub command_inactivity_timeout_secs: Option<u64>,

    /// Grace period before force-killing inactive commands (seconds).
    /// Default: 5
    #[serde(default)]
    pub command_inactivity_kill_grace_secs: Option<u64>,

    /// Enable stream-json output textification.
    /// When true (default), stdout lines that are Claude Code stream-json (NDJSON) events
    /// are converted to human-readable text before being emitted to logs.
    /// Set to false to disable conversion and emit raw JSON lines for troubleshooting.
    /// Default: true
    #[serde(default)]
    pub stream_json_textify: Option<bool>,
}

/// Acceptance prompt mode.
/// Full is deprecated and now behaves identically to ContextOnly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AcceptancePromptMode {
    /// DEPRECATED: Now behaves identically to ContextOnly.
    /// Kept for backward compatibility.
    #[default]
    Full,
    /// Only inject variable context (change metadata, diff, history).
    /// Fixed acceptance instructions come from the command template.
    ContextOnly,
}

impl OrchestratorConfig {
    /// Create a new empty configuration
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another config into this one, with the other config taking priority
    /// for fields that are `Some`.
    pub fn merge(&mut self, other: Self) {
        // Command fields
        if other.apply_command.is_some() {
            self.apply_command = other.apply_command;
        }
        if other.archive_command.is_some() {
            self.archive_command = other.archive_command;
        }
        if other.analyze_command.is_some() {
            self.analyze_command = other.analyze_command;
        }
        if other.acceptance_command.is_some() {
            self.acceptance_command = other.acceptance_command;
        }
        if other.resolve_command.is_some() {
            self.resolve_command = other.resolve_command;
        }

        // Prompt fields
        if other.apply_prompt.is_some() {
            self.apply_prompt = other.apply_prompt;
        }
        if other.acceptance_prompt.is_some() {
            self.acceptance_prompt = other.acceptance_prompt;
        }
        if other.archive_prompt.is_some() {
            self.archive_prompt = other.archive_prompt;
        }

        // Hooks - deep merge each field individually
        if other.hooks.is_some() {
            match (&mut self.hooks, other.hooks) {
                (Some(self_hooks), Some(other_hooks)) => {
                    self_hooks.merge(other_hooks);
                }
                (None, Some(other_hooks)) => {
                    self.hooks = Some(other_hooks);
                }
                _ => {}
            }
        }

        // Logging config
        if other.logging.is_some() {
            self.logging = other.logging;
        }

        // Stall detection
        if other.stall_detection.is_some() {
            self.stall_detection = other.stall_detection;
        }

        // Error circuit breaker
        if other.error_circuit_breaker.is_some() {
            self.error_circuit_breaker = other.error_circuit_breaker;
        }

        // Merge stall detection
        if other.merge_stall_detection.is_some() {
            self.merge_stall_detection = other.merge_stall_detection;
        }

        // Completion check config
        if other.completion_check_delay_ms.is_some() {
            self.completion_check_delay_ms = other.completion_check_delay_ms;
        }
        if other.completion_check_max_retries.is_some() {
            self.completion_check_max_retries = other.completion_check_max_retries;
        }

        // Iteration limit
        if other.max_iterations.is_some() {
            self.max_iterations = other.max_iterations;
        }

        // Parallel execution config
        if other.parallel_mode.is_some() {
            self.parallel_mode = other.parallel_mode;
        }
        if other.max_concurrent_workspaces.is_some() {
            self.max_concurrent_workspaces = other.max_concurrent_workspaces;
        }
        if other.workspace_base_dir.is_some() {
            self.workspace_base_dir = other.workspace_base_dir;
        }
        if other.use_llm_analysis.is_some() {
            self.use_llm_analysis = other.use_llm_analysis;
        }
        if other.vcs_backend.is_some() {
            self.vcs_backend = other.vcs_backend;
        }

        // TUI commands
        if other.propose_command.is_some() {
            self.propose_command = other.propose_command;
        }
        if other.worktree_command.is_some() {
            self.worktree_command = other.worktree_command;
        }

        // Command queue config
        if other.command_queue_stagger_delay_ms.is_some() {
            self.command_queue_stagger_delay_ms = other.command_queue_stagger_delay_ms;
        }
        if other.command_queue_max_retries.is_some() {
            self.command_queue_max_retries = other.command_queue_max_retries;
        }
        if other.command_queue_retry_delay_ms.is_some() {
            self.command_queue_retry_delay_ms = other.command_queue_retry_delay_ms;
        }
        if other.command_queue_retry_patterns.is_some() {
            self.command_queue_retry_patterns = other.command_queue_retry_patterns;
        }
        if other.command_queue_retry_if_duration_under_secs.is_some() {
            self.command_queue_retry_if_duration_under_secs =
                other.command_queue_retry_if_duration_under_secs;
        }

        // Acceptance config
        if other.acceptance_max_continues.is_some() {
            self.acceptance_max_continues = other.acceptance_max_continues;
        }

        // Inactivity timeout config
        if other.command_inactivity_timeout_secs.is_some() {
            self.command_inactivity_timeout_secs = other.command_inactivity_timeout_secs;
        }
        if other.command_inactivity_kill_grace_secs.is_some() {
            self.command_inactivity_kill_grace_secs = other.command_inactivity_kill_grace_secs;
        }

        // Stream-JSON textification
        if other.stream_json_textify.is_some() {
            self.stream_json_textify = other.stream_json_textify;
        }
    }

    /// Get the apply command (required, returns error if not set)
    pub fn get_apply_command(&self) -> Result<&str> {
        self.apply_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: apply_command. Please set it in .cflx.jsonc or global config.".to_string()))
    }

    /// Get the archive command (required, returns error if not set)
    pub fn get_archive_command(&self) -> Result<&str> {
        self.archive_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: archive_command. Please set it in .cflx.jsonc or global config.".to_string()))
    }

    /// Get the analyze command (required, returns error if not set)
    pub fn get_analyze_command(&self) -> Result<&str> {
        self.analyze_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: analyze_command. Please set it in .cflx.jsonc or global config.".to_string()))
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

    /// Get the acceptance command (required, returns error if not set)
    pub fn get_acceptance_command(&self) -> Result<&str> {
        self.acceptance_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: acceptance_command. Please set it in .cflx.jsonc or global config.".to_string()))
    }

    /// Get the acceptance prompt, falling back to default if not set
    pub fn get_acceptance_prompt(&self) -> &str {
        self.acceptance_prompt
            .as_deref()
            .unwrap_or(DEFAULT_ACCEPTANCE_PROMPT)
    }

    pub fn get_acceptance_prompt_mode(&self) -> AcceptancePromptMode {
        self.acceptance_prompt_mode.clone().unwrap_or_default()
    }

    /// Get the hooks configuration, returning default (empty) if not set
    pub fn get_hooks(&self) -> HooksConfig {
        self.hooks.clone().unwrap_or_default()
    }

    /// Get logging configuration, returning defaults if not set.
    pub fn get_logging(&self) -> LoggingConfig {
        self.logging.clone().unwrap_or_default()
    }

    /// Get stall detection configuration, returning defaults if not set.
    pub fn get_stall_detection(&self) -> StallDetectionConfig {
        self.stall_detection.clone().unwrap_or_default()
    }

    /// Get error circuit breaker configuration, returning defaults if not set.
    pub fn get_error_circuit_breaker(&self) -> ErrorCircuitBreakerConfig {
        self.error_circuit_breaker.clone().unwrap_or_default()
    }

    /// Get merge stall detection configuration, returning defaults if not set.
    pub fn get_merge_stall_detection(&self) -> MergeStallDetectionConfig {
        self.merge_stall_detection.clone().unwrap_or_default()
    }

    /// Get the maximum iterations limit.
    /// Returns 0 if explicitly set to 0 (disabled), otherwise returns configured or default value.
    /// A value of 0 means no limit.
    pub fn get_max_iterations(&self) -> u32 {
        self.max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS)
    }

    /// Get whether parallel mode is explicitly enabled in config.
    /// Default: false (unset)
    #[allow(dead_code)]
    pub fn get_parallel_mode(&self) -> bool {
        self.parallel_mode.unwrap_or(false)
    }

    /// Resolve parallel mode based on CLI override and git detection.
    /// Priority: CLI --parallel > config.parallel_mode > git detection default.
    pub fn resolve_parallel_mode(&self, cli_parallel: bool, git_repo_detected: bool) -> bool {
        if cli_parallel {
            return true;
        }

        match self.parallel_mode {
            Some(value) => value,
            None => git_repo_detected,
        }
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

    /// Get the resolve command for conflict resolution (required, returns error if not set).
    pub fn get_resolve_command(&self) -> Result<&str> {
        self.resolve_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: resolve_command. Please set it in .cflx.jsonc or global config.".to_string()))
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
    #[allow(dead_code)]
    pub fn get_propose_command(&self) -> Option<&str> {
        self.propose_command.as_deref()
    }

    /// Get the worktree command template, if configured.
    /// Returns None if not set (worktree flow is disabled).
    pub fn get_worktree_command(&self) -> Option<&str> {
        self.worktree_command.as_deref()
    }

    /// Expand `{proposal}` placeholder in a command template.
    #[allow(dead_code)]
    pub fn expand_proposal(template: &str, proposal: &str) -> String {
        expand::expand_proposal(template, proposal)
    }

    /// Expand `{workspace_dir}` and `{repo_root}` placeholders in a command template.
    pub fn expand_worktree_command(template: &str, workspace_dir: &str, repo_root: &str) -> String {
        expand::expand_worktree_command(template, workspace_dir, repo_root)
    }

    /// Expand `{conflict_files}` placeholder in a command template
    #[allow(dead_code)]
    pub fn expand_conflict_files(template: &str, conflict_files: &str) -> String {
        expand::expand_conflict_files(template, conflict_files)
    }

    /// Get the maximum number of acceptance CONTINUE retries.
    /// Default: 2
    pub fn get_acceptance_max_continues(&self) -> u32 {
        self.acceptance_max_continues
            .unwrap_or(defaults::DEFAULT_ACCEPTANCE_MAX_CONTINUES)
    }

    /// Get the inactivity timeout for commands (seconds).
    /// Returns 0 if disabled.
    /// Default: 900 (15 minutes)
    pub fn get_command_inactivity_timeout_secs(&self) -> u64 {
        self.command_inactivity_timeout_secs
            .unwrap_or(defaults::DEFAULT_COMMAND_INACTIVITY_TIMEOUT_SECS)
    }

    /// Get the grace period before force-killing inactive commands (seconds).
    /// Default: 5
    pub fn get_command_inactivity_kill_grace_secs(&self) -> u64 {
        self.command_inactivity_kill_grace_secs
            .unwrap_or(defaults::DEFAULT_COMMAND_INACTIVITY_KILL_GRACE_SECS)
    }

    /// Get whether stream-json output textification is enabled.
    /// Default: true (convert stream-json events to human-readable text)
    pub fn get_stream_json_textify(&self) -> bool {
        self.stream_json_textify
            .unwrap_or(defaults::DEFAULT_STREAM_JSON_TEXTIFY)
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

    /// Load configuration with merge-based priority:
    /// 1. Start with platform default config (lowest priority)
    /// 2. Merge XDG config (default path) if exists
    /// 3. Merge XDG config (environment variable path) if exists
    /// 4. Merge project config if exists
    /// 5. Merge custom config if provided (highest priority)
    ///
    /// For each field, the last config that has `Some` value wins.
    /// This allows partial configs to inherit from global configs.
    ///
    /// After merging, validates that all required commands are present.
    pub fn load(custom_path: Option<&Path>) -> Result<Self> {
        let mut config = Self::default();

        // 1. Platform default config (lowest priority)
        if let Some(platform_path) = get_platform_config_path() {
            if platform_path.exists() {
                debug!("Loading platform config from: {:?}", platform_path);
                let platform_config = Self::load_from_file(&platform_path)?;
                config.merge(platform_config);
            }
        }

        // 2. XDG config (default path: ~/.config)
        if let Some(xdg_default_path) = get_xdg_default_config_path() {
            if xdg_default_path.exists() {
                debug!("Loading XDG default config from: {:?}", xdg_default_path);
                let xdg_default_config = Self::load_from_file(&xdg_default_path)?;
                config.merge(xdg_default_config);
            }
        }

        // 3. XDG config (environment variable: $XDG_CONFIG_HOME)
        if let Some(xdg_env_path) = get_xdg_env_config_path() {
            if xdg_env_path.exists() {
                debug!("Loading XDG env config from: {:?}", xdg_env_path);
                let xdg_env_config = Self::load_from_file(&xdg_env_path)?;
                config.merge(xdg_env_config);
            }
        }

        // 4. Project config (higher priority than global)
        let project_config_path = PathBuf::from(PROJECT_CONFIG_FILE);
        if project_config_path.exists() {
            debug!("Loading project config from: {:?}", project_config_path);
            let project_config = Self::load_from_file(&project_config_path)?;
            config.merge(project_config);
        }

        // 5. Custom config path (highest priority)
        if let Some(path) = custom_path {
            debug!("Loading custom config from: {:?}", path);
            let custom_config = Self::load_from_file(path)?;
            config.merge(custom_config);
        }

        // Validate required commands after merging
        config.validate_required_commands()?;

        info!("Configuration loaded and merged successfully");
        Ok(config)
    }

    /// Validate that all required commands are present in the merged configuration.
    /// Required commands: apply_command, archive_command, analyze_command, acceptance_command, resolve_command
    fn validate_required_commands(&self) -> Result<()> {
        let mut missing = Vec::new();

        if self.apply_command.is_none() {
            missing.push("apply_command");
        }
        if self.archive_command.is_none() {
            missing.push("archive_command");
        }
        if self.analyze_command.is_none() {
            missing.push("analyze_command");
        }
        if self.acceptance_command.is_none() {
            missing.push("acceptance_command");
        }
        if self.resolve_command.is_none() {
            missing.push("resolve_command");
        }

        if !missing.is_empty() {
            return Err(OrchestratorError::ConfigLoad(format!(
                "Missing required config: {}. Please set them in .cflx.jsonc or global config.",
                missing.join(", ")
            )));
        }

        Ok(())
    }
}

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

// Re-export commonly used items for convenience
pub use defaults::{
    DEFAULT_MAX_CONCURRENT_WORKSPACES, DEFAULT_MAX_ITERATIONS, GLOBAL_CONFIG_DIR,
    GLOBAL_CONFIG_FILE, PROJECT_CONFIG_FILE,
};

#[allow(unused_imports)]
pub use defaults::DEFAULT_ACCEPTANCE_MAX_CONTINUES;

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
}
