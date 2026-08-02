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
    /// Empty-WIP count after which apply retries may switch to escalation.
    /// When unset, apply escalation is disabled even if an escalation command is configured.
    #[serde(default)]
    pub apply_escalation_after_empty_wip: Option<u32>,
    /// Maximum number of escalation command uses per empty-WIP stall sequence.
    /// When unset, apply escalation is disabled even if an escalation command is configured.
    #[serde(default)]
    pub apply_escalation_max_uses_per_stall: Option<u32>,
}

impl Default for StallDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_STALL_DETECTION_ENABLED,
            threshold: DEFAULT_STALL_DETECTION_THRESHOLD,
            apply_escalation_after_empty_wip: DEFAULT_APPLY_ESCALATION_AFTER_EMPTY_WIP,
            apply_escalation_max_uses_per_stall: DEFAULT_APPLY_ESCALATION_MAX_USES_PER_STALL,
        }
    }
}

impl StallDetectionConfig {
    /// Validate empty-WIP stall and escalation policy boundaries.
    pub fn validate(&self) -> Result<()> {
        if let Some(after) = self.apply_escalation_after_empty_wip {
            if after >= self.threshold {
                return Err(OrchestratorError::ConfigLoad(format!(
                    "stall_detection.apply_escalation_after_empty_wip ({after}) must be less than stall_detection.threshold ({})",
                    self.threshold
                )));
            }
        }

        if matches!(self.apply_escalation_max_uses_per_stall, Some(0)) {
            return Err(OrchestratorError::ConfigLoad(
                "stall_detection.apply_escalation_max_uses_per_stall must be at least 1 when set"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// Return true when both escalation policy knobs make escalation eligible.
    pub fn apply_escalation_policy_enabled(&self) -> bool {
        self.apply_escalation_after_empty_wip.is_some()
            && self.apply_escalation_max_uses_per_stall.is_some()
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

// ── OrchestratorConfig ─────────────────────────────────────────────────────

/// Orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrchestratorConfig {
    /// Command template for applying changes.
    /// Supports `{change_id}` placeholder.
    #[serde(default)]
    pub apply_command: Option<String>,

    /// Optional command template for late empty-WIP apply retries.
    /// Supports `{change_id}` and `{prompt}` placeholders. When absent, escalation is a silent no-op.
    #[serde(default)]
    pub apply_escalation_command: Option<String>,

    /// Optional command template for diagnosing final empty-WIP apply stalls.
    /// Supports `{change_id}` and `{prompt}` placeholders. When absent, diagnosis is a silent no-op.
    #[serde(default)]
    pub apply_stall_diagnose_command: Option<String>,

    /// Command template for archiving changes.
    /// Supports `{change_id}` placeholder.
    #[serde(default)]
    pub archive_command: Option<String>,

    /// Operation skill loaded for apply prompts.
    #[serde(default)]
    pub apply_skill: Option<String>,

    /// Operation skill loaded for archive prompts.
    #[serde(default)]
    pub archive_skill: Option<String>,

    /// Operation skill loaded for dependency-analysis prompts.
    #[serde(default)]
    pub analyze_skill: Option<String>,

    /// Operation skill loaded for acceptance prompts.
    #[serde(default)]
    pub accept_skill: Option<String>,

    /// Operation skill loaded for rejecting-review prompts.
    #[serde(default)]
    pub rejecting_skill: Option<String>,

    /// Operation skill loaded for cleanup-review prompts.
    #[serde(default)]
    pub cleanup_review_skill: Option<String>,

    /// Operation skill loaded for resolve prompts.
    #[serde(default)]
    pub resolve_skill: Option<String>,

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

    /// Optional raw guidance appended after the final generated apply prompt.
    #[serde(default)]
    pub apply_append_prompt: Option<String>,

    /// System prompt for acceptance command.
    /// Injected into the `{prompt}` placeholder in acceptance_command.
    #[serde(default)]
    pub acceptance_prompt: Option<String>,

    /// Optional raw guidance appended after the final generated acceptance prompt.
    #[serde(default)]
    pub acceptance_append_prompt: Option<String>,

    /// Controls how the acceptance `{prompt}` is constructed.
    /// - full: DEPRECATED - now behaves identically to context_only (no embedded system prompt)
    /// - context_only: only include change metadata + diff/history context
    ///
    /// The "full" mode is now deprecated and unified with "context_only".
    /// Acceptance operation guidance comes from the selected portable accept_skill.
    #[serde(default)]
    pub acceptance_prompt_mode: Option<AcceptancePromptMode>,

    /// System prompt for archive command.
    /// Injected into the `{prompt}` placeholder in archive_command.
    #[serde(default)]
    pub archive_prompt: Option<String>,

    /// Optional raw guidance appended after the final generated archive prompt.
    #[serde(default)]
    pub archive_append_prompt: Option<String>,

    /// Optional raw guidance appended after the final generated analyze prompt.
    #[serde(default)]
    pub analyze_append_prompt: Option<String>,

    /// Optional raw guidance appended after the final generated resolve prompt.
    #[serde(default)]
    pub resolve_append_prompt: Option<String>,

    /// Additional environment variables for Conflux-owned agent commands.
    #[serde(default)]
    pub envs: Option<HashMap<String, String>>,

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

    /// Optional external lifecycle integration (observability-only adapter process).
    ///
    /// Absent by default: no adapter process is started and behavior is unchanged.
    #[serde(default)]
    pub lifecycle_integration: Option<LifecycleIntegrationConfig>,
}

// ── LifecycleIntegrationConfig ─────────────────────────────────────────────

/// Configuration for one optional external lifecycle adapter.
///
/// The adapter is spawned once per cflx process as a child command with piped
/// stdin and the inherited process environment. It receives newline-delimited
/// JSON lifecycle messages. It is observability-only: it never influences
/// workflow routing and all failures are reported as bounded diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LifecycleIntegrationConfig {
    /// Explicitly enable/disable the configured adapter.
    ///
    /// When unset, a configured non-empty `command` is treated as enabled.
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Adapter argv. The first element is the executable; remaining elements are arguments.
    #[serde(default)]
    pub command: Vec<String>,

    /// Bounded queue capacity for pending lifecycle messages.
    #[serde(default)]
    pub queue_capacity: Option<usize>,

    /// Bounded per-message write timeout in milliseconds.
    #[serde(default)]
    pub write_timeout_ms: Option<u64>,

    /// Bounded shutdown deadline in milliseconds.
    #[serde(default)]
    pub shutdown_timeout_ms: Option<u64>,
}

impl LifecycleIntegrationConfig {
    /// Return true when this integration should start an adapter process.
    ///
    /// An explicit `enabled: false` always wins. Otherwise the presence of a
    /// non-empty `command` is the opt-in signal.
    pub fn is_enabled(&self) -> bool {
        match self.enabled {
            Some(false) => false,
            Some(true) => true,
            None => !self.command.is_empty(),
        }
    }

    /// Bounded queue capacity, falling back to the built-in default.
    pub fn queue_capacity(&self) -> usize {
        self.queue_capacity
            .unwrap_or(DEFAULT_LIFECYCLE_QUEUE_CAPACITY)
    }

    /// Bounded per-message write timeout, falling back to the built-in default.
    pub fn write_timeout_ms(&self) -> u64 {
        self.write_timeout_ms
            .unwrap_or(DEFAULT_LIFECYCLE_WRITE_TIMEOUT_MS)
    }

    /// Bounded shutdown deadline, falling back to the built-in default.
    pub fn shutdown_timeout_ms(&self) -> u64 {
        self.shutdown_timeout_ms
            .unwrap_or(DEFAULT_LIFECYCLE_SHUTDOWN_TIMEOUT_MS)
    }

    /// Validate the adapter definition with actionable diagnostics.
    ///
    /// A disabled integration is always valid so that operators can keep a
    /// commented-out adapter definition without breaking startup.
    pub fn validate(&self) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        if self.command.is_empty() {
            return Err(OrchestratorError::ConfigLoad(
                "Configuration error: `lifecycle_integration.command` must be a non-empty argv array, for example [\"cflx-herdr-adapter\"]".to_string(),
            ));
        }

        if self.command[0].trim().is_empty() {
            return Err(OrchestratorError::ConfigLoad(
                "Configuration error: `lifecycle_integration.command[0]` must be a non-empty executable name".to_string(),
            ));
        }

        if matches!(self.queue_capacity, Some(0)) {
            return Err(OrchestratorError::ConfigLoad(
                "Configuration error: `lifecycle_integration.queue_capacity` must be at least 1"
                    .to_string(),
            ));
        }

        if matches!(self.write_timeout_ms, Some(0)) {
            return Err(OrchestratorError::ConfigLoad(
                "Configuration error: `lifecycle_integration.write_timeout_ms` must be at least 1"
                    .to_string(),
            ));
        }

        if matches!(self.shutdown_timeout_ms, Some(0)) {
            return Err(OrchestratorError::ConfigLoad(
                "Configuration error: `lifecycle_integration.shutdown_timeout_ms` must be at least 1"
                    .to_string(),
            ));
        }

        Ok(())
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
    /// Only inject selected skill prelude and variable context (change metadata, diff, history).
    /// Acceptance guidance comes from the selected portable accept_skill.
    ContextOnly,
}

// ── OrchestratorConfig impls ───────────────────────────────────────────────

fn overwrite_if_some<T>(target: &mut Option<T>, source: Option<T>) {
    if source.is_some() {
        *target = source;
    }
}

fn merge_hooks_config(target: &mut Option<HooksConfig>, source: Option<HooksConfig>) {
    match (target.as_mut(), source) {
        (Some(target_hooks), Some(source_hooks)) => target_hooks.merge(source_hooks),
        (None, Some(source_hooks)) => *target = Some(source_hooks),
        (_, None) => {}
    }
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
        let Self {
            apply_command,
            apply_escalation_command,
            apply_stall_diagnose_command,
            archive_command,
            apply_skill,
            archive_skill,
            analyze_skill,
            accept_skill,
            rejecting_skill,
            cleanup_review_skill,
            resolve_skill,
            analyze_command,
            acceptance_command,
            apply_prompt,
            apply_append_prompt,
            acceptance_prompt,
            acceptance_append_prompt,
            acceptance_prompt_mode,
            archive_prompt,
            archive_append_prompt,
            analyze_append_prompt,
            resolve_append_prompt,
            envs,
            hooks,
            logging,
            stall_detection,
            error_circuit_breaker,
            completion_check_delay_ms,
            completion_check_max_retries,
            max_iterations,
            parallel_mode,
            max_concurrent_workspaces,
            workspace_base_dir,
            resolve_command,
            use_llm_analysis,
            vcs_backend,
            propose_command,
            worktree_command,
            command_queue_stagger_delay_ms,
            command_queue_max_retries,
            command_queue_retry_delay_ms,
            command_queue_retry_patterns,
            command_queue_retry_if_duration_under_secs,
            acceptance_max_continues,
            command_inactivity_timeout_secs,
            command_inactivity_kill_grace_secs,
            command_inactivity_timeout_max_retries,
            stream_json_textify,
            command_strict_process_cleanup,
            lifecycle_integration,
        } = other;

        // Plain Option fields use the standard config precedence rule:
        // a higher-priority Some value overwrites, while None preserves the existing value.
        overwrite_if_some(&mut self.apply_command, apply_command);
        overwrite_if_some(&mut self.apply_escalation_command, apply_escalation_command);
        overwrite_if_some(
            &mut self.apply_stall_diagnose_command,
            apply_stall_diagnose_command,
        );
        overwrite_if_some(&mut self.archive_command, archive_command);
        overwrite_if_some(&mut self.apply_skill, apply_skill);
        overwrite_if_some(&mut self.archive_skill, archive_skill);
        overwrite_if_some(&mut self.analyze_skill, analyze_skill);
        overwrite_if_some(&mut self.accept_skill, accept_skill);
        overwrite_if_some(&mut self.rejecting_skill, rejecting_skill);
        overwrite_if_some(&mut self.cleanup_review_skill, cleanup_review_skill);
        overwrite_if_some(&mut self.resolve_skill, resolve_skill);
        overwrite_if_some(&mut self.analyze_command, analyze_command);
        overwrite_if_some(&mut self.acceptance_command, acceptance_command);
        overwrite_if_some(&mut self.resolve_command, resolve_command);

        overwrite_if_some(&mut self.apply_prompt, apply_prompt);
        overwrite_if_some(&mut self.apply_append_prompt, apply_append_prompt);
        overwrite_if_some(&mut self.acceptance_prompt, acceptance_prompt);
        overwrite_if_some(&mut self.acceptance_append_prompt, acceptance_append_prompt);
        overwrite_if_some(&mut self.archive_prompt, archive_prompt);
        overwrite_if_some(&mut self.archive_append_prompt, archive_append_prompt);
        overwrite_if_some(&mut self.analyze_append_prompt, analyze_append_prompt);
        overwrite_if_some(&mut self.resolve_append_prompt, resolve_append_prompt);
        overwrite_if_some(&mut self.acceptance_prompt_mode, acceptance_prompt_mode);

        if let Some(envs) = envs {
            self.envs.get_or_insert_with(HashMap::new).extend(envs);
        }

        merge_hooks_config(&mut self.hooks, hooks);

        overwrite_if_some(&mut self.logging, logging);
        overwrite_if_some(&mut self.stall_detection, stall_detection);
        overwrite_if_some(&mut self.error_circuit_breaker, error_circuit_breaker);

        overwrite_if_some(
            &mut self.completion_check_delay_ms,
            completion_check_delay_ms,
        );
        overwrite_if_some(
            &mut self.completion_check_max_retries,
            completion_check_max_retries,
        );
        overwrite_if_some(&mut self.max_iterations, max_iterations);

        overwrite_if_some(&mut self.parallel_mode, parallel_mode);
        overwrite_if_some(
            &mut self.max_concurrent_workspaces,
            max_concurrent_workspaces,
        );
        overwrite_if_some(&mut self.workspace_base_dir, workspace_base_dir);
        overwrite_if_some(&mut self.use_llm_analysis, use_llm_analysis);
        overwrite_if_some(&mut self.vcs_backend, vcs_backend);

        overwrite_if_some(&mut self.propose_command, propose_command);
        overwrite_if_some(&mut self.worktree_command, worktree_command);

        overwrite_if_some(
            &mut self.command_queue_stagger_delay_ms,
            command_queue_stagger_delay_ms,
        );
        overwrite_if_some(
            &mut self.command_queue_max_retries,
            command_queue_max_retries,
        );
        overwrite_if_some(
            &mut self.command_queue_retry_delay_ms,
            command_queue_retry_delay_ms,
        );
        overwrite_if_some(
            &mut self.command_queue_retry_patterns,
            command_queue_retry_patterns,
        );
        overwrite_if_some(
            &mut self.command_queue_retry_if_duration_under_secs,
            command_queue_retry_if_duration_under_secs,
        );

        overwrite_if_some(&mut self.acceptance_max_continues, acceptance_max_continues);
        overwrite_if_some(
            &mut self.command_inactivity_timeout_secs,
            command_inactivity_timeout_secs,
        );
        overwrite_if_some(
            &mut self.command_inactivity_kill_grace_secs,
            command_inactivity_kill_grace_secs,
        );
        overwrite_if_some(
            &mut self.command_inactivity_timeout_max_retries,
            command_inactivity_timeout_max_retries,
        );
        overwrite_if_some(&mut self.stream_json_textify, stream_json_textify);
        overwrite_if_some(
            &mut self.command_strict_process_cleanup,
            command_strict_process_cleanup,
        );
        overwrite_if_some(&mut self.lifecycle_integration, lifecycle_integration);
    }

    /// Get the configured external lifecycle integration, if any.
    ///
    /// Returns `None` when unconfigured so callers can keep the default
    /// "no adapter process is started" behavior without extra branching.
    pub fn get_lifecycle_integration(&self) -> Option<&LifecycleIntegrationConfig> {
        self.lifecycle_integration.as_ref()
    }

    /// Validate the optional external lifecycle integration definition.
    pub fn validate_lifecycle_integration(&self) -> Result<()> {
        match self.lifecycle_integration.as_ref() {
            Some(integration) => integration.validate(),
            None => Ok(()),
        }
    }

    /// Get the apply command (required, returns error if not set)
    pub fn get_apply_command(&self) -> Result<&str> {
        self.apply_command
            .as_deref()
            .ok_or_else(|| OrchestratorError::ConfigLoad("Missing required config: apply_command. Please set it in .cflx.jsonc or global config.".to_string()))
    }

    /// Get the optional apply escalation command template.
    pub fn get_apply_escalation_command(&self) -> Option<&str> {
        self.apply_escalation_command.as_deref()
    }

    /// Get the optional apply stall diagnosis command template.
    pub fn get_apply_stall_diagnose_command(&self) -> Option<&str> {
        self.apply_stall_diagnose_command.as_deref()
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

    fn configured_operation_skill<'a>(
        configured: Option<&'a str>,
        default: &'static str,
    ) -> &'a str {
        configured.unwrap_or(default)
    }

    /// Get the dependency-analysis operation skill, falling back to the built-in default.
    pub fn get_analyze_skill(&self) -> &str {
        Self::configured_operation_skill(self.analyze_skill.as_deref(), DEFAULT_ANALYZE_SKILL)
    }

    /// Get the apply operation skill, falling back to the built-in default.
    pub fn get_apply_skill(&self) -> &str {
        Self::configured_operation_skill(self.apply_skill.as_deref(), DEFAULT_APPLY_SKILL)
    }

    /// Get the rejecting-review operation skill, falling back to the built-in default.
    pub fn get_rejecting_skill(&self) -> &str {
        Self::configured_operation_skill(self.rejecting_skill.as_deref(), DEFAULT_REJECTING_SKILL)
    }

    /// Get the cleanup-review operation skill, falling back to the built-in default.
    pub fn get_cleanup_review_skill(&self) -> &str {
        Self::configured_operation_skill(
            self.cleanup_review_skill.as_deref(),
            DEFAULT_CLEANUP_REVIEW_SKILL,
        )
    }

    /// Get the acceptance operation skill, falling back to the built-in default.
    pub fn get_accept_skill(&self) -> &str {
        Self::configured_operation_skill(self.accept_skill.as_deref(), DEFAULT_ACCEPT_SKILL)
    }

    /// Get the archive operation skill, falling back to the built-in default.
    pub fn get_archive_skill(&self) -> &str {
        Self::configured_operation_skill(self.archive_skill.as_deref(), DEFAULT_ARCHIVE_SKILL)
    }

    /// Get the conflict-resolution operation skill, falling back to the built-in default.
    pub fn get_resolve_skill(&self) -> &str {
        Self::configured_operation_skill(self.resolve_skill.as_deref(), DEFAULT_RESOLVE_SKILL)
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

    pub fn get_apply_append_prompt(&self) -> Option<&str> {
        self.apply_append_prompt.as_deref()
    }

    pub fn get_acceptance_append_prompt(&self) -> Option<&str> {
        self.acceptance_append_prompt.as_deref()
    }

    pub fn get_archive_append_prompt(&self) -> Option<&str> {
        self.archive_append_prompt.as_deref()
    }

    pub fn get_analyze_append_prompt(&self) -> Option<&str> {
        self.analyze_append_prompt.as_deref()
    }

    pub fn get_resolve_append_prompt(&self) -> Option<&str> {
        self.resolve_append_prompt.as_deref()
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

    /// Get configured agent command environment variables, expanded from the parent process environment.
    pub fn get_command_envs(&self) -> HashMap<String, String> {
        self.envs
            .as_ref()
            .map(|envs| {
                envs.iter()
                    .map(|(key, value)| (key.clone(), expand::expand_env_value(value)))
                    .collect()
            })
            .unwrap_or_default()
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

    fn validate_operation_skill_value(field: &str, value: Option<&str>) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        if value.trim().is_empty() {
            return Err(OrchestratorError::ConfigLoad(format!(
                "Configuration error: `{field}` must not be empty"
            )));
        }
        if value.contains('\n') || value.contains('\r') {
            return Err(OrchestratorError::ConfigLoad(format!(
                "Configuration error: `{field}` must not contain newline characters"
            )));
        }
        Ok(())
    }

    /// Validate configured operation skill preludes.
    pub fn validate_operation_skills(&self) -> Result<()> {
        Self::validate_operation_skill_value("analyze_skill", self.analyze_skill.as_deref())?;
        Self::validate_operation_skill_value("apply_skill", self.apply_skill.as_deref())?;
        Self::validate_operation_skill_value("rejecting_skill", self.rejecting_skill.as_deref())?;
        Self::validate_operation_skill_value(
            "cleanup_review_skill",
            self.cleanup_review_skill.as_deref(),
        )?;
        Self::validate_operation_skill_value("accept_skill", self.accept_skill.as_deref())?;
        Self::validate_operation_skill_value("archive_skill", self.archive_skill.as_deref())?;
        Self::validate_operation_skill_value("resolve_skill", self.resolve_skill.as_deref())?;
        Ok(())
    }

    /// Validate that all required commands are present in the merged configuration.
    /// Required commands: apply_command, archive_command, analyze_command, acceptance_command, resolve_command
    pub fn validate_required_commands(&self) -> Result<()> {
        self.get_stall_detection().validate()?;
        self.validate_operation_skills()?;
        self.validate_lifecycle_integration()?;

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

#[cfg(test)]
mod lifecycle_integration_config_tests {
    use super::*;

    fn enabled_config(command: Vec<&str>) -> LifecycleIntegrationConfig {
        LifecycleIntegrationConfig {
            command: command.into_iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn lifecycle_integration_is_absent_by_default() {
        let config = OrchestratorConfig::default();

        assert!(config.get_lifecycle_integration().is_none());
        assert!(config.validate_lifecycle_integration().is_ok());
    }

    #[test]
    fn configured_argv_enables_adapter() {
        let integration = enabled_config(vec!["my-adapter", "--flag"]);

        assert!(integration.is_enabled());
        assert!(integration.validate().is_ok());
    }

    #[test]
    fn explicit_disable_wins_over_configured_argv() {
        let integration = LifecycleIntegrationConfig {
            enabled: Some(false),
            ..enabled_config(vec!["my-adapter"])
        };

        assert!(!integration.is_enabled());
        assert!(integration.validate().is_ok());
    }

    #[test]
    fn empty_argv_is_rejected_with_actionable_diagnostic() {
        let integration = LifecycleIntegrationConfig {
            enabled: Some(true),
            ..Default::default()
        };

        let err = integration
            .validate()
            .expect_err("empty argv must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("lifecycle_integration.command"),
            "diagnostic must name the offending key: {message}"
        );
        assert!(
            message.contains("non-empty argv array"),
            "diagnostic must explain the expected shape: {message}"
        );
    }

    #[test]
    fn blank_executable_is_rejected() {
        let integration = enabled_config(vec!["   "]);

        let err = integration
            .validate()
            .expect_err("blank executable must be rejected");
        assert!(err.to_string().contains("lifecycle_integration.command[0]"));
    }

    #[test]
    fn zero_bounds_are_rejected() {
        for integration in [
            LifecycleIntegrationConfig {
                queue_capacity: Some(0),
                ..enabled_config(vec!["my-adapter"])
            },
            LifecycleIntegrationConfig {
                write_timeout_ms: Some(0),
                ..enabled_config(vec!["my-adapter"])
            },
            LifecycleIntegrationConfig {
                shutdown_timeout_ms: Some(0),
                ..enabled_config(vec!["my-adapter"])
            },
        ] {
            assert!(
                integration.validate().is_err(),
                "zero bound must be rejected: {integration:?}"
            );
        }
    }

    #[test]
    fn bounded_defaults_are_applied_when_unset() {
        let integration = enabled_config(vec!["my-adapter"]);

        assert_eq!(
            integration.queue_capacity(),
            DEFAULT_LIFECYCLE_QUEUE_CAPACITY
        );
        assert_eq!(
            integration.write_timeout_ms(),
            DEFAULT_LIFECYCLE_WRITE_TIMEOUT_MS
        );
        assert_eq!(
            integration.shutdown_timeout_ms(),
            DEFAULT_LIFECYCLE_SHUTDOWN_TIMEOUT_MS
        );
    }

    #[test]
    fn required_command_validation_surfaces_lifecycle_errors() {
        let config = OrchestratorConfig {
            apply_command: Some("apply".to_string()),
            archive_command: Some("archive".to_string()),
            analyze_command: Some("analyze".to_string()),
            acceptance_command: Some("accept".to_string()),
            resolve_command: Some("resolve".to_string()),
            lifecycle_integration: Some(LifecycleIntegrationConfig {
                enabled: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        let err = config
            .validate_required_commands()
            .expect_err("invalid lifecycle integration must fail required-command validation");
        assert!(err.to_string().contains("lifecycle_integration.command"));
    }

    #[test]
    fn merge_overwrites_lifecycle_integration_and_preserves_unset() {
        let mut base = OrchestratorConfig {
            lifecycle_integration: Some(enabled_config(vec!["base-adapter"])),
            ..Default::default()
        };

        base.merge(OrchestratorConfig::default());
        assert_eq!(
            base.get_lifecycle_integration().map(|i| i.command.clone()),
            Some(vec!["base-adapter".to_string()])
        );

        base.merge(OrchestratorConfig {
            lifecycle_integration: Some(enabled_config(vec!["project-adapter"])),
            ..Default::default()
        });
        assert_eq!(
            base.get_lifecycle_integration().map(|i| i.command.clone()),
            Some(vec!["project-adapter".to_string()])
        );
    }

    #[test]
    fn existing_configuration_without_lifecycle_key_still_parses() {
        let parsed: OrchestratorConfig =
            serde_json::from_str(r#"{"apply_command": "apply", "archive_command": "archive"}"#)
                .expect("legacy configuration must remain parseable");

        assert!(parsed.get_lifecycle_integration().is_none());
    }

    #[test]
    fn lifecycle_integration_parses_from_configuration_document() {
        let parsed: OrchestratorConfig = serde_json::from_str(
            r#"{"lifecycle_integration": {"command": ["adapter", "--pane"], "shutdown_timeout_ms": 500}}"#,
        )
        .expect("lifecycle integration must parse");

        let integration = parsed
            .get_lifecycle_integration()
            .expect("integration should be present");
        assert!(integration.is_enabled());
        assert_eq!(integration.command, vec!["adapter", "--pane"]);
        assert_eq!(integration.shutdown_timeout_ms(), 500);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_skill_accessors_return_defaults_when_unset() {
        let config = OrchestratorConfig::default();

        assert_eq!(config.get_analyze_skill(), DEFAULT_ANALYZE_SKILL);
        assert_eq!(config.get_apply_skill(), DEFAULT_APPLY_SKILL);
        assert_eq!(config.get_rejecting_skill(), DEFAULT_REJECTING_SKILL);
        assert_eq!(
            config.get_cleanup_review_skill(),
            DEFAULT_CLEANUP_REVIEW_SKILL
        );
        assert_eq!(config.get_accept_skill(), DEFAULT_ACCEPT_SKILL);
        assert_eq!(config.get_archive_skill(), DEFAULT_ARCHIVE_SKILL);
        assert_eq!(config.get_resolve_skill(), DEFAULT_RESOLVE_SKILL);
    }

    #[test]
    fn operation_skill_accessors_return_configured_values() {
        let config = OrchestratorConfig {
            analyze_skill: Some("team-analyze".to_string()),
            apply_skill: Some("team-apply".to_string()),
            rejecting_skill: Some("team-rejecting".to_string()),
            cleanup_review_skill: Some("team-cleanup-review".to_string()),
            accept_skill: Some("cflx-accept-with-speca".to_string()),
            archive_skill: Some("team-archive".to_string()),
            resolve_skill: Some("team-resolve".to_string()),
            ..Default::default()
        };

        assert_eq!(config.get_analyze_skill(), "team-analyze");
        assert_eq!(config.get_apply_skill(), "team-apply");
        assert_eq!(config.get_rejecting_skill(), "team-rejecting");
        assert_eq!(config.get_cleanup_review_skill(), "team-cleanup-review");
        assert_eq!(config.get_accept_skill(), "cflx-accept-with-speca");
        assert_eq!(config.get_archive_skill(), "team-archive");
        assert_eq!(config.get_resolve_skill(), "team-resolve");
    }

    #[test]
    fn operation_skill_merge_preserves_lower_precedence_when_omitted() {
        let mut lower = OrchestratorConfig {
            accept_skill: Some("cflx-accept-with-speca".to_string()),
            resolve_skill: Some("team-resolve".to_string()),
            ..Default::default()
        };

        lower.merge(OrchestratorConfig::default());

        assert_eq!(lower.get_accept_skill(), "cflx-accept-with-speca");
        assert_eq!(lower.get_resolve_skill(), "team-resolve");
    }

    #[test]
    fn operation_skill_merge_overrides_accept_and_resolve() {
        let mut config = OrchestratorConfig {
            accept_skill: Some("lower-accept".to_string()),
            resolve_skill: Some("lower-resolve".to_string()),
            ..Default::default()
        };
        let higher = OrchestratorConfig {
            accept_skill: Some("cflx-accept-with-speca".to_string()),
            resolve_skill: Some("team-resolve".to_string()),
            ..Default::default()
        };

        config.merge(higher);

        assert_eq!(config.get_accept_skill(), "cflx-accept-with-speca");
        assert_eq!(config.get_resolve_skill(), "team-resolve");
    }

    #[test]
    fn operation_skill_validation_rejects_empty_or_newline_values() {
        let empty = OrchestratorConfig {
            accept_skill: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(empty.validate_operation_skills().is_err());

        let newline = OrchestratorConfig {
            resolve_skill: Some("team-resolve\nload skills: other".to_string()),
            ..Default::default()
        };
        assert!(newline.validate_operation_skills().is_err());
    }
}
