//! Type definitions and business-logic impls for orchestrator configuration.
//!
//! Contains all configuration structs, enums, their Default implementations,
//! merge/validation/accessor methods. File I/O is handled by the sibling `load`
//! module.

use std::collections::HashMap;

use crate::error::{OrchestratorError, Result};
use crate::hooks::HooksConfig;
use crate::vcs::VcsBackend;
use serde::{Deserialize, Serialize};

use super::defaults::{self, *};
use super::expand;

// ── serde default helpers ──────────────────────────────────────────────────

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

// ── Logging ────────────────────────────────────────────────────────────────

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

// ── Stall detection ────────────────────────────────────────────────────────

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

impl Default for StallDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_STALL_DETECTION_ENABLED,
            threshold: DEFAULT_STALL_DETECTION_THRESHOLD,
        }
    }
}

// ── Error circuit breaker ──────────────────────────────────────────────────

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

// ── Merge stall detection ──────────────────────────────────────────────────

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

// ── OrchestratorConfig ─────────────────────────────────────────────────────

/// Orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorConfig {
    /// Server daemon configuration (used only by `cflx server` subcommand).
    /// When present in global config, its values are applied before CLI overrides.
    #[serde(default)]
    pub server: Option<ServerConfig>,

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

    /// Maximum number of retries after inactivity timeout.
    /// Default: 3. Set to 0 to disable retries entirely.
    /// When the command is terminated by inactivity timeout it is retried up to this many times.
    #[serde(default)]
    pub command_inactivity_timeout_max_retries: Option<u32>,

    /// Enable stream-json output textification.
    /// When true (default), stdout lines that are Claude Code stream-json (NDJSON) events
    /// are converted to human-readable text before being emitted to logs.
    /// Set to false to disable conversion and emit raw JSON lines for troubleshooting.
    /// Default: true
    #[serde(default)]
    pub stream_json_textify: Option<bool>,

    /// Enable strict post-completion process-group cleanup.
    /// When true (default), after a command finishes (success, failure, cancellation, or
    /// inactivity timeout), the orchestrator sends SIGTERM then SIGKILL to the entire
    /// spawned process group to prevent orphaned background processes.
    /// Set to false to disable for debugging scenarios where intentional background
    /// processes should survive command completion.
    /// Default: true
    #[serde(default)]
    pub command_strict_process_cleanup: Option<bool>,

    /// Proposal session configuration (ACP-based interactive proposal creation).
    #[serde(default)]
    pub proposal_session: Option<ProposalSessionConfig>,
}

// ── ProposalSessionConfig ──────────────────────────────────────────────────

/// Configuration for proposal sessions (OpenCode transport interactive proposal creation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionConfig {
    /// OpenCode subprocess command (default: "opencode")
    #[serde(default = "default_proposal_transport_command", alias = "acp_command")]
    pub transport_command: String,

    /// Arguments passed to the OpenCode command before `serve --port 0 ...`
    #[serde(default = "default_proposal_transport_args", alias = "acp_args")]
    pub transport_args: Vec<String>,

    /// Additional environment variables for the OpenCode subprocess
    #[serde(default, alias = "acp_env")]
    pub transport_env: HashMap<String, String>,

    /// Inactivity timeout in seconds before OpenCode process is killed (default: 1800)
    #[serde(default = "default_proposal_session_inactivity_timeout_secs")]
    pub session_inactivity_timeout_secs: u64,
}

fn default_proposal_transport_command() -> String {
    defaults::DEFAULT_PROPOSAL_TRANSPORT_COMMAND.to_string()
}

fn default_proposal_transport_args() -> Vec<String> {
    defaults::DEFAULT_PROPOSAL_TRANSPORT_ARGS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn default_proposal_session_inactivity_timeout_secs() -> u64 {
    defaults::DEFAULT_PROPOSAL_SESSION_INACTIVITY_TIMEOUT_SECS
}

impl Default for ProposalSessionConfig {
    fn default() -> Self {
        Self {
            transport_command: default_proposal_transport_command(),
            transport_args: default_proposal_transport_args(),
            transport_env: HashMap::new(),
            session_inactivity_timeout_secs: default_proposal_session_inactivity_timeout_secs(),
        }
    }
}

// ── ServerAuthMode ─────────────────────────────────────────────────────────

/// Authentication mode for the server daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerAuthMode {
    /// No authentication (only safe for loopback addresses)
    #[default]
    None,
    /// Bearer token authentication (required for non-loopback addresses)
    BearerToken,
}

// ── ServerAuthConfig ───────────────────────────────────────────────────────

/// Authentication configuration for the server daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerAuthConfig {
    /// Authentication mode
    #[serde(default)]
    pub mode: ServerAuthMode,
    /// Bearer token for authentication (required when mode = bearer_token)
    #[serde(default)]
    pub token: Option<String>,
    /// Environment variable name to read the bearer token from.
    /// If set, the token is resolved from the environment variable at startup.
    /// Takes precedence over `token` when both are set.
    #[serde(default)]
    pub token_env: Option<String>,
}

impl Default for ServerAuthConfig {
    fn default() -> Self {
        Self {
            mode: ServerAuthMode::None,
            token: None,
            token_env: None,
        }
    }
}

impl ServerAuthConfig {
    /// Resolve the effective bearer token.
    /// If `token_env` is set, read the token from the named environment variable.
    /// If `token_env` is not set (or the variable is unset/empty), fall back to `token`.
    pub fn resolve_token(&self) -> Option<String> {
        if let Some(env_var) = &self.token_env {
            if let Ok(val) = std::env::var(env_var) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        self.token.clone()
    }
}

// ── ServerConfig ───────────────────────────────────────────────────────────

fn default_server_bind() -> String {
    defaults::DEFAULT_SERVER_BIND.to_string()
}

fn default_server_port() -> u16 {
    defaults::DEFAULT_SERVER_PORT
}

fn default_server_max_concurrent_total() -> usize {
    defaults::DEFAULT_SERVER_MAX_CONCURRENT_TOTAL
}

fn default_server_data_dir() -> std::path::PathBuf {
    defaults::default_server_data_dir()
}

/// Server daemon configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address for the server (default: 127.0.0.1)
    #[serde(default = "default_server_bind")]
    pub bind: String,

    /// Port for the server (default: 39876)
    #[serde(default = "default_server_port")]
    pub port: u16,

    /// Authentication configuration
    #[serde(default)]
    pub auth: ServerAuthConfig,

    /// Maximum number of concurrent project executions globally
    #[serde(default = "default_server_max_concurrent_total")]
    pub max_concurrent_total: usize,

    /// Directory for persistent server data (projects registry, etc.)
    #[serde(default = "default_server_data_dir")]
    pub data_dir: std::path::PathBuf,

    /// DEPRECATED: `server.resolve_command` is no longer supported.
    /// Use the top-level `resolve_command` in your config file instead.
    /// Setting this field will cause a configuration error at startup.
    #[serde(default)]
    pub resolve_command: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: default_server_bind(),
            port: default_server_port(),
            auth: ServerAuthConfig::default(),
            max_concurrent_total: default_server_max_concurrent_total(),
            data_dir: default_server_data_dir(),
            resolve_command: None,
        }
    }
}

impl ServerConfig {
    /// Check if the bind address is loopback (127.0.0.0/8 or ::1).
    pub fn is_loopback_bind(&self) -> bool {
        let addr = self.bind.trim();
        // IPv4 loopback: 127.x.x.x
        if addr.starts_with("127.") || addr == "localhost" {
            return true;
        }
        // IPv6 loopback
        if addr == "::1" || addr == "[::1]" {
            return true;
        }
        false
    }

    /// Validate the server configuration.
    /// Returns error if non-loopback bind is used without bearer token authentication,
    /// or if the deprecated `server.resolve_command` field is set.
    pub fn validate(&self) -> crate::error::Result<()> {
        // Check for deprecated server.resolve_command field
        if self.resolve_command.is_some() {
            return Err(crate::error::OrchestratorError::ConfigLoad(
                "Configuration error: `server.resolve_command` is no longer supported. \
                Please remove it from your config and use the top-level `resolve_command` instead."
                    .to_string(),
            ));
        }

        if !self.is_loopback_bind() {
            match self.auth.mode {
                ServerAuthMode::BearerToken => {
                    // Accept token from token_env (env var resolution) or token field
                    if self
                        .auth
                        .resolve_token()
                        .as_deref()
                        .unwrap_or("")
                        .is_empty()
                    {
                        return Err(crate::error::OrchestratorError::ConfigLoad(
                            "Server: non-loopback bind requires auth.token or auth.token_env to be set when auth.mode=bearer_token".to_string(),
                        ));
                    }
                }
                ServerAuthMode::None => {
                    return Err(crate::error::OrchestratorError::ConfigLoad(
                        "Server: non-loopback bind requires auth.mode=bearer_token with a token"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Apply CLI overrides (bind, port, auth_token, max_concurrent_total, data_dir).
    pub fn apply_cli_overrides(
        &mut self,
        bind: Option<&str>,
        port: Option<u16>,
        auth_token: Option<&str>,
        max_concurrent_total: Option<usize>,
        data_dir: Option<&std::path::Path>,
    ) {
        if let Some(b) = bind {
            self.bind = b.to_string();
        }
        if let Some(p) = port {
            self.port = p;
        }
        if let Some(token) = auth_token {
            self.auth.mode = ServerAuthMode::BearerToken;
            self.auth.token = Some(token.to_string());
        }
        if let Some(max) = max_concurrent_total {
            self.max_concurrent_total = max;
        }
        if let Some(dir) = data_dir {
            self.data_dir = dir.to_path_buf();
        }
    }
}

// ── AcceptancePromptMode ───────────────────────────────────────────────────

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

// ── OrchestratorConfig impls ───────────────────────────────────────────────

impl OrchestratorConfig {
    /// Create a new empty configuration
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge another config into this one, with the other config taking priority
    /// for fields that are `Some`.
    pub fn merge(&mut self, other: Self) {
        // Server config
        if other.server.is_some() {
            self.server = other.server;
        }

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
        if other.command_inactivity_timeout_max_retries.is_some() {
            self.command_inactivity_timeout_max_retries =
                other.command_inactivity_timeout_max_retries;
        }

        // Stream-JSON textification
        if other.stream_json_textify.is_some() {
            self.stream_json_textify = other.stream_json_textify;
        }

        // Strict process cleanup
        if other.command_strict_process_cleanup.is_some() {
            self.command_strict_process_cleanup = other.command_strict_process_cleanup;
        }

        // acceptance_prompt_mode
        if other.acceptance_prompt_mode.is_some() {
            self.acceptance_prompt_mode = other.acceptance_prompt_mode;
        }

        // Proposal session config
        if other.proposal_session.is_some() {
            self.proposal_session = other.proposal_session;
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

    /// Get the maximum number of retries after inactivity timeout.
    /// Default: 3. Set to 0 to disable retries.
    pub fn get_command_inactivity_timeout_max_retries(&self) -> u32 {
        self.command_inactivity_timeout_max_retries
            .unwrap_or(defaults::DEFAULT_COMMAND_INACTIVITY_TIMEOUT_MAX_RETRIES)
    }

    /// Get whether stream-json output textification is enabled.
    /// Default: true (convert stream-json events to human-readable text)
    pub fn get_stream_json_textify(&self) -> bool {
        self.stream_json_textify
            .unwrap_or(defaults::DEFAULT_STREAM_JSON_TEXTIFY)
    }

    /// Get whether strict post-completion process-group cleanup is enabled.
    /// Default: true (always sweep the process group after command completion)
    pub fn get_command_strict_process_cleanup(&self) -> bool {
        self.command_strict_process_cleanup
            .unwrap_or(defaults::DEFAULT_COMMAND_STRICT_PROCESS_CLEANUP)
    }

    /// Expand `{change_id}` placeholder in a command template
    pub fn expand_change_id(template: &str, change_id: &str) -> String {
        expand::expand_change_id(template, change_id)
    }

    /// Expand `{prompt}` placeholder in a command template
    pub fn expand_prompt(template: &str, prompt: &str) -> String {
        expand::expand_prompt(template, prompt)
    }

    /// Validate that all required commands are present in the merged configuration.
    /// Required commands: apply_command, archive_command, analyze_command, acceptance_command, resolve_command
    pub fn validate_required_commands(&self) -> Result<()> {
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
