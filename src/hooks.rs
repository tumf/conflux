//! Hook execution module for OpenSpec Orchestrator.
//!
//! Provides a system for executing user-defined commands at various stages
//! of the orchestration process.

use crate::config::expand;
use crate::error::{OrchestratorError, Result};
use crate::events::{ExecutionEvent, LogEntry};
use crate::orchestration::output::OutputHandler;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Default timeout for hook execution in seconds
pub const DEFAULT_HOOK_TIMEOUT: u64 = 60;

/// Maximum bytes of hook output to display before truncating
pub const HOOK_OUTPUT_TRUNCATE_BYTES: usize = 1024;

/// Types of hooks that can be executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum HookType {
    // === Run lifecycle ===
    /// Triggered when the orchestrator starts (once per run)
    OnStart,
    /// Triggered when the orchestrator finishes (once per run)
    OnFinish,
    /// Triggered on error
    OnError,

    // === Change lifecycle ===
    /// Triggered when starting to process a new change (once per change)
    OnChangeStart,
    /// Triggered before each apply execution
    PreApply,
    /// Triggered after each successful apply
    PostApply,
    /// Triggered when a change reaches 100% task completion
    OnChangeComplete,
    /// Triggered before each archive execution
    PreArchive,
    /// Triggered after each successful archive
    PostArchive,
    /// Triggered when change processing ends (once per change, after archive)
    OnChangeEnd,
    /// Triggered when a change is merged to base branch
    OnMerged,

    // === User interaction (TUI only) ===
    /// Triggered when user adds a change to queue (Space key)
    OnQueueAdd,
    /// Triggered when user removes a change from queue (Space key)
    OnQueueRemove,
}

impl HookType {
    /// Get the configuration key name for this hook type
    pub fn config_key(&self) -> &'static str {
        match self {
            // Run lifecycle
            HookType::OnStart => "on_start",
            HookType::OnFinish => "on_finish",
            HookType::OnError => "on_error",
            // Change lifecycle
            HookType::OnChangeStart => "on_change_start",
            HookType::PreApply => "pre_apply",
            HookType::PostApply => "post_apply",
            HookType::OnChangeComplete => "on_change_complete",
            HookType::PreArchive => "pre_archive",
            HookType::PostArchive => "post_archive",
            HookType::OnChangeEnd => "on_change_end",
            HookType::OnMerged => "on_merged",
            // User interaction (TUI only)
            HookType::OnQueueAdd => "on_queue_add",
            HookType::OnQueueRemove => "on_queue_remove",
        }
    }
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config_key())
    }
}

/// Configuration for a single hook
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookConfig {
    /// The command to execute
    pub command: String,
    /// Whether to continue if the hook fails (default: true)
    #[serde(default = "default_continue_on_failure")]
    pub continue_on_failure: bool,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_continue_on_failure() -> bool {
    true
}

fn default_timeout() -> u64 {
    DEFAULT_HOOK_TIMEOUT
}

impl HookConfig {
    /// Create a new HookConfig with just a command (using defaults)
    pub fn from_command(command: String) -> Self {
        Self {
            command,
            continue_on_failure: true,
            timeout: DEFAULT_HOOK_TIMEOUT,
        }
    }
}

/// Wrapper type that can deserialize from either a string or a HookConfig object
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HookConfigValue {
    /// Simple string command (uses defaults)
    Simple(String),
    /// Full configuration object
    Full(HookConfig),
}

impl<'de> Deserialize<'de> for HookConfigValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct HookConfigValueVisitor;

        impl<'de> Visitor<'de> for HookConfigValueVisitor {
            type Value = HookConfigValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a hook configuration object")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(HookConfigValue::Simple(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(HookConfigValue::Simple(value))
            }

            fn visit_map<M>(self, map: M) -> std::result::Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let config = HookConfig::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(HookConfigValue::Full(config))
            }
        }

        deserializer.deserialize_any(HookConfigValueVisitor)
    }
}

impl HookConfigValue {
    /// Convert to a HookConfig, applying defaults for simple string form
    pub fn into_hook_config(self) -> HookConfig {
        match self {
            HookConfigValue::Simple(cmd) => HookConfig::from_command(cmd),
            HookConfigValue::Full(config) => config,
        }
    }
}

/// Configuration for all hooks
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    // === Run lifecycle ===
    #[serde(default)]
    pub on_start: Option<HookConfigValue>,
    #[serde(default)]
    pub on_finish: Option<HookConfigValue>,
    #[serde(default)]
    pub on_error: Option<HookConfigValue>,

    // === Change lifecycle ===
    #[serde(default)]
    pub on_change_start: Option<HookConfigValue>,
    #[serde(default)]
    pub pre_apply: Option<HookConfigValue>,
    #[serde(default)]
    pub post_apply: Option<HookConfigValue>,
    #[serde(default)]
    pub on_change_complete: Option<HookConfigValue>,
    #[serde(default)]
    pub pre_archive: Option<HookConfigValue>,
    #[serde(default)]
    pub post_archive: Option<HookConfigValue>,
    #[serde(default)]
    pub on_change_end: Option<HookConfigValue>,
    #[serde(default)]
    pub on_merged: Option<HookConfigValue>,

    // === User interaction (TUI only) ===
    #[serde(default)]
    pub on_queue_add: Option<HookConfigValue>,
    #[serde(default)]
    pub on_queue_remove: Option<HookConfigValue>,
}

impl HooksConfig {
    /// Merge another HooksConfig into this one, with the other config taking priority
    /// for fields that are `Some`. This enables deep merging of hook configurations.
    pub fn merge(&mut self, other: Self) {
        // Macro to reduce repetition
        macro_rules! merge_hook {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field;
                }
            };
        }

        // Run lifecycle
        merge_hook!(on_start);
        merge_hook!(on_finish);
        merge_hook!(on_error);

        // Change lifecycle
        merge_hook!(on_change_start);
        merge_hook!(pre_apply);
        merge_hook!(post_apply);
        merge_hook!(on_change_complete);
        merge_hook!(pre_archive);
        merge_hook!(post_archive);
        merge_hook!(on_change_end);
        merge_hook!(on_merged);

        // User interaction (TUI only)
        merge_hook!(on_queue_add);
        merge_hook!(on_queue_remove);
    }

    /// Get the hook configuration for a specific hook type
    pub fn get(&self, hook_type: HookType) -> Option<HookConfig> {
        let value = match hook_type {
            // Run lifecycle
            HookType::OnStart => self.on_start.clone(),
            HookType::OnFinish => self.on_finish.clone(),
            HookType::OnError => self.on_error.clone(),
            // Change lifecycle
            HookType::OnChangeStart => self.on_change_start.clone(),
            HookType::PreApply => self.pre_apply.clone(),
            HookType::PostApply => self.post_apply.clone(),
            HookType::OnChangeComplete => self.on_change_complete.clone(),
            HookType::PreArchive => self.pre_archive.clone(),
            HookType::PostArchive => self.post_archive.clone(),
            HookType::OnChangeEnd => self.on_change_end.clone(),
            HookType::OnMerged => self.on_merged.clone(),
            // User interaction (TUI only)
            HookType::OnQueueAdd => self.on_queue_add.clone(),
            HookType::OnQueueRemove => self.on_queue_remove.clone(),
        };
        value.map(|v| v.into_hook_config())
    }

    /// Check if any hooks are configured
    #[allow(dead_code)]
    pub fn has_any_hooks(&self) -> bool {
        self.on_start.is_some()
            || self.on_finish.is_some()
            || self.on_error.is_some()
            || self.on_change_start.is_some()
            || self.pre_apply.is_some()
            || self.post_apply.is_some()
            || self.on_change_complete.is_some()
            || self.pre_archive.is_some()
            || self.post_archive.is_some()
            || self.on_change_end.is_some()
            || self.on_merged.is_some()
            || self.on_queue_add.is_some()
            || self.on_queue_remove.is_some()
    }
}

/// Context information passed to hooks
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    /// Current change ID (always set except for on_start/on_finish)
    pub change_id: Option<String>,
    /// Number of changes processed so far (completed + archived)
    pub changes_processed: usize,
    /// Total number of changes in initial queue
    pub total_changes: usize,
    /// Remaining changes in queue
    pub remaining_changes: usize,
    /// Completed tasks for current change
    pub completed_tasks: Option<u32>,
    /// Total tasks for current change
    pub total_tasks: Option<u32>,
    /// Apply count for current change (how many times applied)
    pub apply_count: u32,
    /// Finish status (for on_finish: "completed", "iteration_limit", "cancelled")
    pub status: Option<String>,
    /// Error message (for on_error hook)
    pub error: Option<String>,
    /// Whether running in dry-run mode
    pub dry_run: bool,
    /// Workspace path (for parallel mode)
    pub workspace_path: Option<String>,
    /// Group index (for parallel mode)
    pub group_index: Option<u32>,
}

impl HookContext {
    /// Create a new HookContext with basic run-level info
    pub fn new(
        changes_processed: usize,
        total_changes: usize,
        remaining_changes: usize,
        dry_run: bool,
    ) -> Self {
        Self {
            changes_processed,
            total_changes,
            remaining_changes,
            dry_run,
            ..Default::default()
        }
    }

    /// Set the change-related fields
    pub fn with_change(mut self, change_id: &str, completed_tasks: u32, total_tasks: u32) -> Self {
        self.change_id = Some(change_id.to_string());
        self.completed_tasks = Some(completed_tasks);
        self.total_tasks = Some(total_tasks);
        self
    }

    /// Set the apply count for the current change
    pub fn with_apply_count(mut self, apply_count: u32) -> Self {
        self.apply_count = apply_count;
        self
    }

    /// Set the status field (for on_finish)
    pub fn with_status(mut self, status: &str) -> Self {
        self.status = Some(status.to_string());
        self
    }

    /// Set the error field (for on_error)
    pub fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }

    /// Set parallel execution context (workspace path and group index)
    pub fn with_parallel_context(mut self, workspace_path: &str, group_index: Option<u32>) -> Self {
        self.workspace_path = Some(workspace_path.to_string());
        self.group_index = group_index;
        self
    }

    /// Convert context to environment variables
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if let Some(ref change_id) = self.change_id {
            vars.insert("OPENSPEC_CHANGE_ID".to_string(), change_id.clone());
        }
        vars.insert(
            "OPENSPEC_CHANGES_PROCESSED".to_string(),
            self.changes_processed.to_string(),
        );
        vars.insert(
            "OPENSPEC_TOTAL_CHANGES".to_string(),
            self.total_changes.to_string(),
        );
        vars.insert(
            "OPENSPEC_REMAINING_CHANGES".to_string(),
            self.remaining_changes.to_string(),
        );
        if let Some(completed) = self.completed_tasks {
            vars.insert(
                "OPENSPEC_COMPLETED_TASKS".to_string(),
                completed.to_string(),
            );
        }
        if let Some(total) = self.total_tasks {
            vars.insert("OPENSPEC_TOTAL_TASKS".to_string(), total.to_string());
        }
        vars.insert(
            "OPENSPEC_APPLY_COUNT".to_string(),
            self.apply_count.to_string(),
        );
        if let Some(ref status) = self.status {
            vars.insert("OPENSPEC_STATUS".to_string(), status.clone());
        }
        if let Some(ref error) = self.error {
            vars.insert("OPENSPEC_ERROR".to_string(), error.clone());
        }
        vars.insert("OPENSPEC_DRY_RUN".to_string(), self.dry_run.to_string());

        // Parallel mode specific variables
        if let Some(ref workspace_path) = self.workspace_path {
            vars.insert(
                "OPENSPEC_WORKSPACE_PATH".to_string(),
                workspace_path.clone(),
            );
        }
        if let Some(group_index) = self.group_index {
            vars.insert("OPENSPEC_GROUP_INDEX".to_string(), group_index.to_string());
        }

        vars
    }

    /// Expand placeholders in a command string.
    ///
    /// Placeholder values are shell-escaped consistently with config command expansion.
    pub fn expand_placeholders(&self, template: &str) -> String {
        let mut result = template.to_string();

        if let Some(ref change_id) = self.change_id {
            result = expand::expand_placeholder(&result, "{change_id}", change_id);
        }
        result = expand::expand_placeholder(
            &result,
            "{changes_processed}",
            &self.changes_processed.to_string(),
        );
        result =
            expand::expand_placeholder(&result, "{total_changes}", &self.total_changes.to_string());
        result = expand::expand_placeholder(
            &result,
            "{remaining_changes}",
            &self.remaining_changes.to_string(),
        );
        if let Some(completed) = self.completed_tasks {
            result =
                expand::expand_placeholder(&result, "{completed_tasks}", &completed.to_string());
        }
        if let Some(total) = self.total_tasks {
            result = expand::expand_placeholder(&result, "{total_tasks}", &total.to_string());
        }
        result =
            expand::expand_placeholder(&result, "{apply_count}", &self.apply_count.to_string());
        if let Some(ref status) = self.status {
            result = expand::expand_placeholder(&result, "{status}", status);
        }
        if let Some(ref error) = self.error {
            result = expand::expand_placeholder(&result, "{error}", error);
        }

        result
    }
}

/// Truncate hook output to `limit` bytes, respecting UTF-8 char boundaries.
///
/// Returns `(display_slice, was_truncated)`.
fn truncate_hook_output(s: &str, limit: usize) -> (&str, bool) {
    if s.len() <= limit {
        return (s, false);
    }
    // Walk back from `limit` to find a valid char boundary
    let mut boundary = limit;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    (&s[..boundary], true)
}

/// Hook runner that executes hooks based on configuration
#[derive(Clone)]
pub struct HookRunner {
    config: HooksConfig,
    /// Optional event sender for hook logs (TUI/parallel mode)
    event_tx: Option<mpsc::Sender<ExecutionEvent>>,
    /// Optional output handler for CLI-visible hook logs
    output_handler: Option<Arc<dyn OutputHandler>>,
}

impl std::fmt::Debug for HookRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRunner")
            .field("config", &self.config)
            .field("event_tx", &self.event_tx.is_some())
            .field("output_handler", &self.output_handler.is_some())
            .finish()
    }
}

impl HookRunner {
    /// Create a new HookRunner with the given configuration
    pub fn new(config: HooksConfig) -> Self {
        Self {
            config,
            event_tx: None,
            output_handler: None,
        }
    }

    /// Create a HookRunner with the given configuration and output handler for CLI-visible logs.
    ///
    /// Use this in CLI (`cflx run`) mode so hook command invocations and captured output
    /// are surfaced in the user-visible log stream.
    pub fn with_output_handler(
        config: HooksConfig,
        output_handler: Arc<dyn OutputHandler>,
    ) -> Self {
        Self {
            config,
            event_tx: None,
            output_handler: Some(output_handler),
        }
    }

    /// Create a HookRunner with the given configuration and event sender
    pub fn with_event_tx(config: HooksConfig, event_tx: mpsc::Sender<ExecutionEvent>) -> Self {
        Self {
            config,
            event_tx: Some(event_tx),
            output_handler: None,
        }
    }

    /// Create a HookRunner with no hooks configured
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            config: HooksConfig::default(),
            event_tx: None,
            output_handler: None,
        }
    }

    /// Check if a specific hook is configured
    #[allow(dead_code)]
    pub fn has_hook(&self, hook_type: HookType) -> bool {
        self.config.get(hook_type).is_some()
    }

    /// Run a hook if configured
    ///
    /// Returns Ok(()) if:
    /// - Hook is not configured
    /// - Hook executed successfully
    /// - Hook failed but continue_on_failure is true
    ///
    /// Returns Err if hook failed and continue_on_failure is false
    pub async fn run_hook(&self, hook_type: HookType, context: &HookContext) -> Result<()> {
        let Some(hook_config) = self.config.get(hook_type) else {
            debug!("No hook configured for {}", hook_type);
            return Ok(());
        };

        let command = context.expand_placeholders(&hook_config.command);
        let env_vars = context.to_env_vars();
        let timeout_duration = Duration::from_secs(hook_config.timeout);

        info!(
            module = module_path!(),
            "Running {} hook: {}", hook_type, command
        );
        debug!("Hook timeout: {}s", hook_config.timeout);

        // Emit hook command to all configured log sinks (event channel and/or output handler)
        let cmd_msg = format!("Running {} hook: {}", hook_type, command);
        if let Some(ref tx) = self.event_tx {
            let _ = tx
                .send(ExecutionEvent::Log(LogEntry::info(cmd_msg.clone())))
                .await;
        }
        if let Some(ref handler) = self.output_handler {
            handler.on_info(&cmd_msg);
        }

        match self
            .execute_hook(hook_type, &command, &env_vars, timeout_duration)
            .await
        {
            Ok((success, stdout, stderr)) => {
                // Emit captured stdout – always, regardless of exit status
                if !stdout.is_empty() {
                    let (display, was_truncated) =
                        truncate_hook_output(&stdout, HOOK_OUTPUT_TRUNCATE_BYTES);
                    let mut msg = format!("{} hook stdout: {}", hook_type, display);
                    if was_truncated {
                        msg.push_str(&format!(
                            "\n[... {} bytes truncated]",
                            stdout.len() - HOOK_OUTPUT_TRUNCATE_BYTES
                        ));
                    }
                    if let Some(ref tx) = self.event_tx {
                        let _ = tx
                            .send(ExecutionEvent::Log(LogEntry::info(msg.clone())))
                            .await;
                    }
                    if let Some(ref handler) = self.output_handler {
                        handler.on_stdout(&msg);
                    }
                }

                // Emit captured stderr – always, regardless of exit status
                if !stderr.is_empty() {
                    let (display, was_truncated) =
                        truncate_hook_output(&stderr, HOOK_OUTPUT_TRUNCATE_BYTES);
                    let mut msg = format!("{} hook stderr: {}", hook_type, display);
                    if was_truncated {
                        msg.push_str(&format!(
                            "\n[... {} bytes truncated]",
                            stderr.len() - HOOK_OUTPUT_TRUNCATE_BYTES
                        ));
                    }
                    if let Some(ref tx) = self.event_tx {
                        let _ = tx
                            .send(ExecutionEvent::Log(LogEntry::warn(msg.clone())))
                            .await;
                    }
                    if let Some(ref handler) = self.output_handler {
                        handler.on_stderr(&msg);
                    }
                }

                if success {
                    info!("{} hook completed successfully", hook_type);
                    Ok(())
                } else if hook_config.continue_on_failure {
                    warn!(
                        "{} hook failed (non-zero exit), continuing due to continue_on_failure=true",
                        hook_type
                    );
                    Ok(())
                } else {
                    error!("{} hook failed (non-zero exit)", hook_type);
                    Err(OrchestratorError::HookFailed {
                        hook_type: hook_type.to_string(),
                        message: "Hook command returned non-zero exit code".to_string(),
                    })
                }
            }
            Err(e) => {
                if hook_config.continue_on_failure {
                    warn!(
                        "{} hook failed: {} (continuing due to continue_on_failure=true)",
                        hook_type, e
                    );
                    Ok(())
                } else {
                    error!("{} hook failed: {}", hook_type, e);
                    Err(e)
                }
            }
        }
    }

    /// Execute a hook command with the given environment variables and timeout.
    ///
    /// Returns `(success, stdout, stderr)` with stdout and stderr captured separately
    /// so callers can emit them with appropriate labels and log levels.
    async fn execute_hook(
        &self,
        hook_type: HookType,
        command: &str,
        env_vars: &HashMap<String, String>,
        timeout_duration: Duration,
    ) -> Result<(bool, String, String)> {
        // Use login shell ($SHELL -l -c) so user's PATH from .zprofile/.profile
        // is available, even when cflx is started from launchd/systemd/cron.
        let mut cmd = crate::shell_command::build_login_shell_command(command);

        debug!(
            module = module_path!(),
            "Executing {} hook command via login shell: {}", hook_type, command
        );
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // Capture stdout and stderr for logging
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| OrchestratorError::HookFailed {
            hook_type: hook_type.to_string(),
            message: format!("Failed to spawn hook process: {}", e),
        })?;

        // Capture output asynchronously
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // Wait with timeout
        match timeout(timeout_duration, child.wait()).await {
            Ok(result) => {
                let status = result.map_err(|e| OrchestratorError::HookFailed {
                    hook_type: hook_type.to_string(),
                    message: format!("Failed to wait for hook process: {}", e),
                })?;

                // Read stdout and stderr separately to preserve stream identity
                let mut stdout_output = String::new();
                if let Some(mut stdout_pipe) = stdout {
                    let mut buf = Vec::new();
                    if (stdout_pipe.read_to_end(&mut buf).await).is_ok() {
                        if let Ok(s) = String::from_utf8(buf) {
                            stdout_output = s;
                        }
                    }
                }
                let mut stderr_output = String::new();
                if let Some(mut stderr_pipe) = stderr {
                    let mut buf = Vec::new();
                    if (stderr_pipe.read_to_end(&mut buf).await).is_ok() {
                        if let Ok(s) = String::from_utf8(buf) {
                            stderr_output = s;
                        }
                    }
                }

                Ok((status.success(), stdout_output, stderr_output))
            }
            Err(_) => Err(OrchestratorError::HookTimeout {
                hook_type: hook_type.to_string(),
                timeout_secs: timeout_duration.as_secs(),
            }),
        }
    }

    /// Get the underlying configuration (for testing)
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn config(&self) -> &HooksConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::OnStart.to_string(), "on_start");
        assert_eq!(HookType::PreApply.to_string(), "pre_apply");
        assert_eq!(HookType::OnFinish.to_string(), "on_finish");
    }

    #[test]
    fn test_hook_config_from_command() {
        let config = HookConfig::from_command("echo test".to_string());
        assert_eq!(config.command, "echo test");
        assert!(config.continue_on_failure);
        assert_eq!(config.timeout, DEFAULT_HOOK_TIMEOUT);
    }

    #[test]
    fn test_hook_context_expand_placeholders() {
        let context = HookContext::new(2, 5, 3, false)
            .with_change("test-change", 3, 10)
            .with_apply_count(1)
            .with_status("completed");

        let template = "Change {change_id} processed {changes_processed} of {total_changes} remaining {remaining_changes} apply {apply_count}";
        let result = context.expand_placeholders(template);
        assert_eq!(
            result,
            "Change test-change processed 2 of 5 remaining 3 apply 1"
        );
    }

    #[test]
    fn test_hook_context_to_env_vars() {
        let context = HookContext::new(1, 5, 3, true)
            .with_change("my-change", 2, 10)
            .with_apply_count(2);

        let vars = context.to_env_vars();
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"my-change".to_string())
        );
        assert_eq!(
            vars.get("OPENSPEC_CHANGES_PROCESSED"),
            Some(&"1".to_string())
        );
        assert_eq!(vars.get("OPENSPEC_TOTAL_CHANGES"), Some(&"5".to_string()));
        assert_eq!(
            vars.get("OPENSPEC_REMAINING_CHANGES"),
            Some(&"3".to_string())
        );
        assert_eq!(vars.get("OPENSPEC_COMPLETED_TASKS"), Some(&"2".to_string()));
        assert_eq!(vars.get("OPENSPEC_TOTAL_TASKS"), Some(&"10".to_string()));
        assert_eq!(vars.get("OPENSPEC_APPLY_COUNT"), Some(&"2".to_string()));
        assert_eq!(vars.get("OPENSPEC_DRY_RUN"), Some(&"true".to_string()));
    }

    #[test]
    fn test_hooks_config_deserialize_simple_string() {
        let json = r#"{"on_start": "echo hello"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnStart).unwrap();
        assert_eq!(hook.command, "echo hello");
        assert!(hook.continue_on_failure);
        assert_eq!(hook.timeout, DEFAULT_HOOK_TIMEOUT);
    }

    #[test]
    fn test_hooks_config_deserialize_full_object() {
        let json = r#"{
            "on_start": {
                "command": "echo hello",
                "continue_on_failure": false,
                "timeout": 120
            }
        }"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnStart).unwrap();
        assert_eq!(hook.command, "echo hello");
        assert!(!hook.continue_on_failure);
        assert_eq!(hook.timeout, 120);
    }

    #[test]
    fn test_hooks_config_deserialize_mixed() {
        let json = r#"{
            "on_start": "echo start",
            "post_apply": {
                "command": "cargo test",
                "continue_on_failure": false,
                "timeout": 300
            }
        }"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();

        let on_start = config.get(HookType::OnStart).unwrap();
        assert_eq!(on_start.command, "echo start");
        assert!(on_start.continue_on_failure);

        let post_apply = config.get(HookType::PostApply).unwrap();
        assert_eq!(post_apply.command, "cargo test");
        assert!(!post_apply.continue_on_failure);
        assert_eq!(post_apply.timeout, 300);
    }

    #[test]
    fn test_hooks_config_has_any_hooks() {
        let empty = HooksConfig::default();
        assert!(!empty.has_any_hooks());

        let json = r#"{"on_start": "echo hello"}"#;
        let with_hook: HooksConfig = serde_json::from_str(json).unwrap();
        assert!(with_hook.has_any_hooks());
    }

    #[test]
    fn test_hook_runner_has_hook() {
        let json = r#"{"on_start": "echo hello"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);

        assert!(runner.has_hook(HookType::OnStart));
        assert!(!runner.has_hook(HookType::PreApply));
    }

    #[tokio::test]
    async fn test_hook_runner_run_hook_not_configured() {
        let runner = HookRunner::empty();
        let context = HookContext::default();

        // Should succeed even when hook is not configured
        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hook_runner_run_hook_success() {
        let json = r#"{"on_start": "echo hello"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::default();

        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hook_runner_run_hook_failure_with_continue() {
        let json = r#"{"on_start": {"command": "exit 1", "continue_on_failure": true}}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::default();

        // Should succeed because continue_on_failure is true
        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hook_runner_run_hook_failure_without_continue() {
        let json = r#"{"on_start": {"command": "exit 1", "continue_on_failure": false}}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::default();

        // Should fail because continue_on_failure is false
        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_hook_runner_timeout() {
        let json =
            r#"{"on_start": {"command": "sleep 10", "timeout": 1, "continue_on_failure": false}}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::default();

        // Should fail due to timeout
        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_err());
        if let Err(OrchestratorError::HookTimeout { timeout_secs, .. }) = result {
            assert_eq!(timeout_secs, 1);
        } else {
            panic!("Expected HookTimeout error");
        }
    }

    #[tokio::test]
    async fn test_hook_runner_with_env_vars() {
        let json = r#"{"on_start": "echo $OPENSPEC_CHANGE_ID"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::new(1, 5, 3, false).with_change("test-id", 2, 10);

        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());
    }

    // === Tests for on_queue_add hook (hooks spec 2.1) ===

    #[test]
    fn test_hooks_config_on_queue_add() {
        let json = r#"{"on_queue_add": "echo 'Added {change_id}'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnQueueAdd).unwrap();
        assert_eq!(hook.command, "echo 'Added {change_id}'");
    }

    #[tokio::test]
    async fn test_on_queue_add_hook_execution() {
        let json = r#"{"on_queue_add": "echo added"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::new(0, 5, 5, false).with_change("test-change", 0, 3);

        let result = runner.run_hook(HookType::OnQueueAdd, &context).await;
        assert!(result.is_ok());
    }

    // === Tests for on_queue_remove hook (hooks spec 2.2) ===

    #[test]
    fn test_hooks_config_on_queue_remove() {
        let json = r#"{"on_queue_remove": "echo 'Removed {change_id}'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnQueueRemove).unwrap();
        assert_eq!(hook.command, "echo 'Removed {change_id}'");
    }

    #[tokio::test]
    async fn test_on_queue_remove_hook_execution() {
        let json = r#"{"on_queue_remove": "echo removed"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::new(0, 5, 5, false).with_change("test-change", 0, 3);

        let result = runner.run_hook(HookType::OnQueueRemove, &context).await;
        assert!(result.is_ok());
    }

    // === Tests for on_change_start hook (hooks spec 2.5) ===

    #[test]
    fn test_hooks_config_on_change_start() {
        let json = r#"{"on_change_start": "echo 'Starting {change_id}'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnChangeStart).unwrap();
        assert_eq!(hook.command, "echo 'Starting {change_id}'");
    }

    #[tokio::test]
    async fn test_on_change_start_hook_receives_change_id() {
        let json = r#"{"on_change_start": "echo test"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::new(0, 3, 3, false).with_change("add-feature", 0, 5);

        let result = runner.run_hook(HookType::OnChangeStart, &context).await;
        assert!(result.is_ok());

        // Verify change_id is available in context
        let vars = context.to_env_vars();
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"add-feature".to_string())
        );
    }

    #[test]
    fn test_on_change_start_placeholder_expansion() {
        let context = HookContext::new(0, 3, 3, false).with_change("my-change", 0, 5);
        let template = "git commit -m 'changeset: {change_id}'";
        let result = context.expand_placeholders(template);
        assert_eq!(result, "git commit -m 'changeset: my-change'");
    }

    // === Tests for on_change_end hook (hooks spec 2.6) ===

    #[test]
    fn test_hooks_config_on_change_end() {
        let json = r#"{"on_change_end": "echo 'Finished {change_id}'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let hook = config.get(HookType::OnChangeEnd).unwrap();
        assert_eq!(hook.command, "echo 'Finished {change_id}'");
    }

    #[tokio::test]
    async fn test_on_change_end_hook_execution() {
        let json = r#"{"on_change_end": "echo finished"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        // After first change is archived: changes_processed=1, remaining=2
        let context = HookContext::new(1, 3, 2, false).with_change("change-a", 5, 5);

        let result = runner.run_hook(HookType::OnChangeEnd, &context).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_on_change_end_tracks_progress() {
        // Test that changes_processed/total_changes are correctly available
        let context = HookContext::new(1, 3, 2, false).with_change("change-a", 5, 5);
        let template = "echo '{changes_processed}/{total_changes}'";
        let result = context.expand_placeholders(template);
        assert_eq!(result, "echo '1/3'");
    }

    // === Tests for hook execution order (hooks spec 2.7) ===

    #[test]
    fn test_hook_types_config_key_order() {
        // Verify that hook config keys are correctly mapped
        assert_eq!(HookType::OnStart.config_key(), "on_start");
        assert_eq!(HookType::OnChangeStart.config_key(), "on_change_start");
        assert_eq!(HookType::PreApply.config_key(), "pre_apply");
        assert_eq!(HookType::PostApply.config_key(), "post_apply");
        assert_eq!(
            HookType::OnChangeComplete.config_key(),
            "on_change_complete"
        );
        assert_eq!(HookType::PreArchive.config_key(), "pre_archive");
        assert_eq!(HookType::PostArchive.config_key(), "post_archive");
        assert_eq!(HookType::OnChangeEnd.config_key(), "on_change_end");
        assert_eq!(HookType::OnMerged.config_key(), "on_merged");
        assert_eq!(HookType::OnFinish.config_key(), "on_finish");
    }

    // === Tests for TUI/CLI hook parity (hooks spec 2.8) ===
    // Note: Full parity testing requires integration tests.
    // These unit tests verify the same hook infrastructure is usable.

    #[test]
    fn test_hook_runner_is_reusable_for_tui_and_cli() {
        // Same HookRunner can be used in both modes
        let json = r#"{
            "on_change_start": "echo start",
            "on_change_end": "echo end"
        }"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config.clone());

        // TUI mode context
        let tui_context = HookContext::new(0, 3, 3, false).with_change("change-a", 0, 5);
        // CLI mode context (same structure)
        let cli_context = HookContext::new(0, 3, 3, false).with_change("change-a", 0, 5);

        // Both contexts produce identical environment variables
        assert_eq!(tui_context.to_env_vars(), cli_context.to_env_vars());
        assert!(runner.has_hook(HookType::OnChangeStart));
        assert!(runner.has_hook(HookType::OnChangeEnd));
    }

    // === Tests for apply_count increment (hooks spec context) ===

    #[test]
    fn test_apply_count_increments() {
        // Test apply_count is correctly tracked across multiple applies
        let context1 = HookContext::new(0, 1, 1, false)
            .with_change("my-change", 1, 3)
            .with_apply_count(1);
        let context2 = HookContext::new(0, 1, 1, false)
            .with_change("my-change", 2, 3)
            .with_apply_count(2);
        let context3 = HookContext::new(0, 1, 1, false)
            .with_change("my-change", 3, 3)
            .with_apply_count(3);

        let template = "echo 'Apply #{apply_count}'";
        assert_eq!(context1.expand_placeholders(template), "echo 'Apply #1'");
        assert_eq!(context2.expand_placeholders(template), "echo 'Apply #2'");
        assert_eq!(context3.expand_placeholders(template), "echo 'Apply #3'");
    }

    // === Tests for on_finish hook with status ===

    #[test]
    fn test_on_finish_with_status_placeholder() {
        let context = HookContext::new(3, 3, 0, false).with_status("completed");
        let template = "echo 'Status: {status}, Changes: {changes_processed}/{total_changes}'";
        let result = context.expand_placeholders(template);
        assert_eq!(result, "echo 'Status: completed, Changes: 3/3'");
    }

    #[test]
    fn test_on_finish_with_iteration_limit_status() {
        let context = HookContext::new(2, 3, 1, false).with_status("iteration_limit");
        let vars = context.to_env_vars();
        assert_eq!(
            vars.get("OPENSPEC_STATUS"),
            Some(&"iteration_limit".to_string())
        );
    }

    // === Tests for on_error hook with error message ===

    #[test]
    fn test_on_error_with_error_placeholder() {
        let context = HookContext::new(1, 3, 2, false)
            .with_change("failing-change", 2, 5)
            .with_error("LLM API timeout");
        let template = "echo '[on_error] change={change_id} error={error}'";
        let result = context.expand_placeholders(template);
        assert_eq!(
            result,
            "echo '[on_error] change=failing-change error=LLM API timeout'"
        );
    }

    #[test]
    fn test_on_error_env_vars() {
        let context = HookContext::new(1, 3, 2, false)
            .with_change("failing-change", 2, 5)
            .with_error("Connection refused");
        let vars = context.to_env_vars();
        assert_eq!(
            vars.get("OPENSPEC_ERROR"),
            Some(&"Connection refused".to_string())
        );
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"failing-change".to_string())
        );
    }

    // === Tests for on_start without change_id ===

    #[test]
    fn test_on_start_has_no_change_id() {
        // on_start should NOT have change_id available
        let context = HookContext::new(0, 3, 3, false);
        // No with_change() call
        let template = "echo '{change_id}'";
        let result = context.expand_placeholders(template);
        // change_id is not expanded (remains as placeholder)
        assert_eq!(result, "echo '{change_id}'");

        // But total_changes is available
        let template2 = "echo 'total={total_changes}'";
        let result2 = context.expand_placeholders(template2);
        assert_eq!(result2, "echo 'total=3'");
    }

    // === Tests for all hook types registered ===

    #[test]
    fn test_all_hook_types_can_be_configured() {
        let json = r#"{
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
            "on_merged": "echo merged",
            "on_queue_add": "echo queue_add",
            "on_queue_remove": "echo queue_remove"
        }"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);

        // All hook types should be configured
        assert!(runner.has_hook(HookType::OnStart));
        assert!(runner.has_hook(HookType::OnFinish));
        assert!(runner.has_hook(HookType::OnError));
        assert!(runner.has_hook(HookType::OnChangeStart));
        assert!(runner.has_hook(HookType::PreApply));
        assert!(runner.has_hook(HookType::PostApply));
        assert!(runner.has_hook(HookType::OnChangeComplete));
        assert!(runner.has_hook(HookType::PreArchive));
        assert!(runner.has_hook(HookType::PostArchive));
        assert!(runner.has_hook(HookType::OnChangeEnd));
        assert!(runner.has_hook(HookType::OnMerged));
        assert!(runner.has_hook(HookType::OnQueueAdd));
        assert!(runner.has_hook(HookType::OnQueueRemove));
    }

    // === Tests for parallel mode context (add-parallel-hooks spec) ===

    #[test]
    fn test_hook_context_with_parallel_context() {
        let context = HookContext::new(1, 5, 4, false)
            .with_change("test-change", 3, 10)
            .with_parallel_context("/tmp/workspace-test", Some(2));

        assert_eq!(
            context.workspace_path,
            Some("/tmp/workspace-test".to_string())
        );
        assert_eq!(context.group_index, Some(2));
    }

    #[test]
    fn test_hook_context_parallel_env_vars() {
        let context = HookContext::new(2, 8, 6, false)
            .with_change("parallel-change", 5, 10)
            .with_parallel_context("/workspace/change-1", Some(3));

        let vars = context.to_env_vars();

        // Verify parallel-mode specific env vars
        assert_eq!(
            vars.get("OPENSPEC_WORKSPACE_PATH"),
            Some(&"/workspace/change-1".to_string())
        );
        assert_eq!(vars.get("OPENSPEC_GROUP_INDEX"), Some(&"3".to_string()));

        // Verify standard env vars are still present
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"parallel-change".to_string())
        );
        assert_eq!(
            vars.get("OPENSPEC_CHANGES_PROCESSED"),
            Some(&"2".to_string())
        );
    }

    #[test]
    fn test_hook_context_parallel_env_vars_without_group_index() {
        let context = HookContext::new(0, 3, 3, false)
            .with_change("single-change", 0, 5)
            .with_parallel_context("/workspace/single", None);

        let vars = context.to_env_vars();

        assert_eq!(
            vars.get("OPENSPEC_WORKSPACE_PATH"),
            Some(&"/workspace/single".to_string())
        );
        // group_index is None, so no OPENSPEC_GROUP_INDEX
        assert!(!vars.contains_key("OPENSPEC_GROUP_INDEX"));
    }

    #[test]
    fn test_hook_context_no_parallel_context() {
        let context = HookContext::new(0, 1, 1, false).with_change("sequential-change", 0, 3);

        let vars = context.to_env_vars();

        // Neither workspace_path nor group_index should be set
        assert!(!vars.contains_key("OPENSPEC_WORKSPACE_PATH"));
        assert!(!vars.contains_key("OPENSPEC_GROUP_INDEX"));

        // Standard env vars should still work
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"sequential-change".to_string())
        );
    }

    // === Tests for hook output logging (add-hook-logs-view-output spec) ===

    #[tokio::test]
    async fn test_hook_output_captured_and_logged() {
        use tokio::sync::mpsc;

        let json = r#"{"on_start": "echo 'Hello from hook'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();

        let (tx, mut rx) = mpsc::channel(10);
        let runner = HookRunner::with_event_tx(config, tx);
        let context = HookContext::default();

        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());

        // Collect log events
        let mut log_messages = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let ExecutionEvent::Log(entry) = event {
                log_messages.push(entry.message);
            }
        }

        // Should have at least 2 logs: command execution + output
        assert!(!log_messages.is_empty(), "Expected at least 1 log message");

        // First log should be the command
        assert!(
            log_messages[0].contains("Running on_start hook"),
            "Expected command log, got: {}",
            log_messages[0]
        );

        // If there's output, it should be logged with the "stdout" label
        if log_messages.len() > 1 {
            assert!(
                log_messages[1].contains("Hello from hook")
                    || log_messages[1].contains("on_start hook stdout"),
                "Expected stdout output log, got: {}",
                log_messages[1]
            );
        }
    }

    #[tokio::test]
    async fn test_hook_without_event_tx_still_works() {
        let json = r#"{"on_start": "echo test"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let runner = HookRunner::new(config);
        let context = HookContext::default();

        // Should work without event_tx (no logs sent)
        let result = runner.run_hook(HookType::OnStart, &context).await;
        assert!(result.is_ok());
    }

    // === Regression tests for CLI hook output visibility ===

    use std::sync::{Arc, Mutex};

    /// Minimal OutputHandler that records all messages for test assertions.
    #[derive(Default, Clone)]
    struct RecordingOutputHandler {
        messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl RecordingOutputHandler {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn all(&self) -> Vec<(String, String)> {
            self.messages.lock().unwrap().clone()
        }

        fn content(&self) -> Vec<String> {
            self.all().into_iter().map(|(_, v)| v).collect()
        }
    }

    impl crate::orchestration::output::OutputHandler for RecordingOutputHandler {
        fn on_stdout(&self, line: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("stdout".into(), line.to_string()));
        }

        fn on_stderr(&self, line: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("stderr".into(), line.to_string()));
        }

        fn on_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("info".into(), message.to_string()));
        }

        fn on_warn(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("warn".into(), message.to_string()));
        }

        fn on_error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("error".into(), message.to_string()));
        }

        fn on_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("success".into(), message.to_string()));
        }
    }

    #[tokio::test]
    async fn test_cli_hook_stdout_visible_via_output_handler() {
        // Hook that writes to stdout; output must appear in the output_handler stream.
        let json = r#"{"on_start": "echo 'hello stdout'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let handler = RecordingOutputHandler::new();
        let runner = HookRunner::with_output_handler(config, Arc::new(handler.clone()));

        let result = runner
            .run_hook(HookType::OnStart, &HookContext::default())
            .await;
        assert!(result.is_ok());

        let content = handler.content();
        // Command log must appear first
        assert!(
            content.iter().any(|m| m.contains("Running on_start hook")),
            "Command log not found: {:?}",
            content
        );
        // stdout output must appear
        assert!(
            content
                .iter()
                .any(|m| m.contains("on_start hook stdout") && m.contains("hello stdout")),
            "stdout output not found: {:?}",
            content
        );
    }

    #[tokio::test]
    async fn test_cli_hook_stderr_visible_via_output_handler() {
        // Hook that writes only to stderr; output must appear in the output_handler stream.
        let json = r#"{"on_start": "echo 'hello stderr' >&2"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let handler = RecordingOutputHandler::new();
        let runner = HookRunner::with_output_handler(config, Arc::new(handler.clone()));

        let result = runner
            .run_hook(HookType::OnStart, &HookContext::default())
            .await;
        assert!(result.is_ok());

        let content = handler.content();
        assert!(
            content
                .iter()
                .any(|m| m.contains("on_start hook stderr") && m.contains("hello stderr")),
            "stderr output not found: {:?}",
            content
        );
    }

    #[tokio::test]
    async fn test_cli_hook_output_visible_even_on_failure() {
        // Hook exits non-zero but output must still be shown before the failure.
        let json = r#"{"on_start": {"command": "echo 'output before fail'; exit 1", "continue_on_failure": true}}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let handler = RecordingOutputHandler::new();
        let runner = HookRunner::with_output_handler(config, Arc::new(handler.clone()));

        // continue_on_failure=true so run_hook returns Ok
        let result = runner
            .run_hook(HookType::OnStart, &HookContext::default())
            .await;
        assert!(result.is_ok(), "expected Ok with continue_on_failure=true");

        let content = handler.content();
        assert!(
            content.iter().any(|m| m.contains("output before fail")),
            "output before failure not shown: {:?}",
            content
        );
    }

    #[tokio::test]
    async fn test_cli_hook_global_hooks_no_change_id() {
        // on_start / on_finish have no change_id; output must still surface.
        let json = r#"{"on_finish": "echo 'run finished'"}"#;
        let config: HooksConfig = serde_json::from_str(json).unwrap();
        let handler = RecordingOutputHandler::new();
        let runner = HookRunner::with_output_handler(config, Arc::new(handler.clone()));

        // Build a context with no change_id (as on_start/on_finish use)
        let ctx = HookContext::new(3, 3, 0, false).with_status("completed");

        let result = runner.run_hook(HookType::OnFinish, &ctx).await;
        assert!(result.is_ok());

        let content = handler.content();
        assert!(
            content.iter().any(|m| m.contains("Running on_finish hook")),
            "on_finish command log not shown: {:?}",
            content
        );
        assert!(
            content.iter().any(|m| m.contains("run finished")),
            "on_finish stdout not shown: {:?}",
            content
        );
    }

    #[tokio::test]
    async fn test_cli_hook_truncated_output_marked_explicitly() {
        // Generate output that exceeds HOOK_OUTPUT_TRUNCATE_BYTES to verify the explicit marker.
        let big_output = "x".repeat(HOOK_OUTPUT_TRUNCATE_BYTES + 100);
        let json = format!(r#"{{"on_start": "printf '{}'" }}"#, big_output);
        let config: HooksConfig = serde_json::from_str(&json).unwrap();
        let handler = RecordingOutputHandler::new();
        let runner = HookRunner::with_output_handler(config, Arc::new(handler.clone()));

        let result = runner
            .run_hook(HookType::OnStart, &HookContext::default())
            .await;
        assert!(result.is_ok());

        let content = handler.content();
        let has_truncation_marker = content.iter().any(|m| m.contains("bytes truncated"));
        assert!(
            has_truncation_marker,
            "Expected explicit truncation marker in output: {:?}",
            content
        );
    }

    #[test]
    fn test_truncate_hook_output_below_limit() {
        let s = "hello";
        let (result, truncated) = truncate_hook_output(s, 1024);
        assert_eq!(result, "hello");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_hook_output_at_limit() {
        let s = "a".repeat(1024);
        let (result, truncated) = truncate_hook_output(&s, 1024);
        assert_eq!(result.len(), 1024);
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_hook_output_above_limit() {
        let s = "b".repeat(2000);
        let (result, truncated) = truncate_hook_output(&s, 1024);
        assert_eq!(result.len(), 1024);
        assert!(truncated);
    }

    #[test]
    fn test_truncate_hook_output_multibyte_boundary() {
        // "日" is 3 bytes; 1025 bytes of "日" repeated would split a multi-byte char
        let s = "日".repeat(400); // 1200 bytes total
        let (result, truncated) = truncate_hook_output(&s, 1024);
        // Result must be valid UTF-8 (no partial multi-byte chars)
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(truncated);
        // Should be at most 1024 bytes and a valid char boundary
        assert!(result.len() <= 1024);
    }
}
