//! Hook execution module for OpenSpec Orchestrator.
//!
//! Provides a system for executing user-defined commands at various stages
//! of the orchestration process.

use crate::error::{OrchestratorError, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Default timeout for hook execution in seconds
pub const DEFAULT_HOOK_TIMEOUT: u64 = 60;

/// Types of hooks that can be executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookType {
    /// Triggered when the orchestrator starts
    OnStart,
    /// Triggered before the first apply (once per run)
    OnFirstApply,
    /// Triggered at the start of each iteration
    OnIterationStart,
    /// Triggered before each apply
    PreApply,
    /// Triggered after each successful apply
    PostApply,
    /// Triggered when a change reaches 100% task completion
    OnChangeComplete,
    /// Triggered before each archive
    PreArchive,
    /// Triggered after each successful archive
    PostArchive,
    /// Triggered at the end of each iteration
    OnIterationEnd,
    /// Triggered when the queue state changes
    OnQueueChange,
    /// Triggered when the orchestrator finishes
    OnFinish,
    /// Triggered on error
    OnError,
}

impl HookType {
    /// Get the configuration key name for this hook type
    pub fn config_key(&self) -> &'static str {
        match self {
            HookType::OnStart => "on_start",
            HookType::OnFirstApply => "on_first_apply",
            HookType::OnIterationStart => "on_iteration_start",
            HookType::PreApply => "pre_apply",
            HookType::PostApply => "post_apply",
            HookType::OnChangeComplete => "on_change_complete",
            HookType::PreArchive => "pre_archive",
            HookType::PostArchive => "post_archive",
            HookType::OnIterationEnd => "on_iteration_end",
            HookType::OnQueueChange => "on_queue_change",
            HookType::OnFinish => "on_finish",
            HookType::OnError => "on_error",
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
    #[serde(default)]
    pub on_start: Option<HookConfigValue>,
    #[serde(default)]
    pub on_first_apply: Option<HookConfigValue>,
    #[serde(default)]
    pub on_iteration_start: Option<HookConfigValue>,
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
    pub on_iteration_end: Option<HookConfigValue>,
    #[serde(default)]
    pub on_queue_change: Option<HookConfigValue>,
    #[serde(default)]
    pub on_finish: Option<HookConfigValue>,
    #[serde(default)]
    pub on_error: Option<HookConfigValue>,
}

impl HooksConfig {
    /// Get the hook configuration for a specific hook type
    pub fn get(&self, hook_type: HookType) -> Option<HookConfig> {
        let value = match hook_type {
            HookType::OnStart => self.on_start.clone(),
            HookType::OnFirstApply => self.on_first_apply.clone(),
            HookType::OnIterationStart => self.on_iteration_start.clone(),
            HookType::PreApply => self.pre_apply.clone(),
            HookType::PostApply => self.post_apply.clone(),
            HookType::OnChangeComplete => self.on_change_complete.clone(),
            HookType::PreArchive => self.pre_archive.clone(),
            HookType::PostArchive => self.post_archive.clone(),
            HookType::OnIterationEnd => self.on_iteration_end.clone(),
            HookType::OnQueueChange => self.on_queue_change.clone(),
            HookType::OnFinish => self.on_finish.clone(),
            HookType::OnError => self.on_error.clone(),
        };
        value.map(|v| v.into_hook_config())
    }

    /// Check if any hooks are configured
    #[allow(dead_code)]
    pub fn has_any_hooks(&self) -> bool {
        self.on_start.is_some()
            || self.on_first_apply.is_some()
            || self.on_iteration_start.is_some()
            || self.pre_apply.is_some()
            || self.post_apply.is_some()
            || self.on_change_complete.is_some()
            || self.pre_archive.is_some()
            || self.post_archive.is_some()
            || self.on_iteration_end.is_some()
            || self.on_queue_change.is_some()
            || self.on_finish.is_some()
            || self.on_error.is_some()
    }
}

/// Context information passed to hooks
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    /// Current change ID (if applicable)
    pub change_id: Option<String>,
    /// Current iteration number
    pub iteration: u32,
    /// Total number of changes in initial snapshot
    pub total_changes: usize,
    /// Current queue size (remaining changes)
    pub queue_size: usize,
    /// Completed tasks for current change
    pub completed_tasks: Option<u32>,
    /// Total tasks for current change
    pub total_tasks: Option<u32>,
    /// Finish status (completed, iteration_limit)
    pub status: Option<String>,
    /// Error message (for on_error hook)
    pub error: Option<String>,
    /// Whether running in dry-run mode
    pub dry_run: bool,
}

impl HookContext {
    /// Create a new HookContext with basic info
    pub fn new(iteration: u32, total_changes: usize, queue_size: usize, dry_run: bool) -> Self {
        Self {
            iteration,
            total_changes,
            queue_size,
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

    /// Convert context to environment variables
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if let Some(ref change_id) = self.change_id {
            vars.insert("OPENSPEC_CHANGE_ID".to_string(), change_id.clone());
        }
        vars.insert("OPENSPEC_ITERATION".to_string(), self.iteration.to_string());
        vars.insert(
            "OPENSPEC_TOTAL_CHANGES".to_string(),
            self.total_changes.to_string(),
        );
        vars.insert(
            "OPENSPEC_QUEUE_SIZE".to_string(),
            self.queue_size.to_string(),
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
        if let Some(ref status) = self.status {
            vars.insert("OPENSPEC_STATUS".to_string(), status.clone());
        }
        if let Some(ref error) = self.error {
            vars.insert("OPENSPEC_ERROR".to_string(), error.clone());
        }
        vars.insert("OPENSPEC_DRY_RUN".to_string(), self.dry_run.to_string());

        vars
    }

    /// Expand placeholders in a command string
    pub fn expand_placeholders(&self, template: &str) -> String {
        let mut result = template.to_string();

        if let Some(ref change_id) = self.change_id {
            result = result.replace("{change_id}", change_id);
        }
        result = result.replace("{iteration}", &self.iteration.to_string());
        result = result.replace("{total_changes}", &self.total_changes.to_string());
        result = result.replace("{queue_size}", &self.queue_size.to_string());
        if let Some(completed) = self.completed_tasks {
            result = result.replace("{completed_tasks}", &completed.to_string());
        }
        if let Some(total) = self.total_tasks {
            result = result.replace("{total_tasks}", &total.to_string());
        }
        if let Some(ref status) = self.status {
            result = result.replace("{status}", status);
        }
        if let Some(ref error) = self.error {
            result = result.replace("{error}", error);
        }

        result
    }
}

/// Hook runner that executes hooks based on configuration
#[derive(Debug, Clone)]
pub struct HookRunner {
    config: HooksConfig,
}

impl HookRunner {
    /// Create a new HookRunner with the given configuration
    pub fn new(config: HooksConfig) -> Self {
        Self { config }
    }

    /// Create a HookRunner with no hooks configured
    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            config: HooksConfig::default(),
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

        info!("Running {} hook: {}", hook_type, command);
        debug!("Hook timeout: {}s", hook_config.timeout);

        match self
            .execute_hook(hook_type, &command, &env_vars, timeout_duration)
            .await
        {
            Ok(success) => {
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

    /// Execute a hook command with the given environment variables and timeout
    async fn execute_hook(
        &self,
        hook_type: HookType,
        command: &str,
        env_vars: &HashMap<String, String>,
        timeout_duration: Duration,
    ) -> Result<bool> {
        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c
        };

        // Set environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

        let child = cmd.spawn().map_err(|e| OrchestratorError::HookFailed {
            hook_type: hook_type.to_string(),
            message: format!("Failed to spawn hook process: {}", e),
        })?;

        // Wait with timeout
        match timeout(timeout_duration, child.wait_with_output()).await {
            Ok(result) => {
                let output = result.map_err(|e| OrchestratorError::HookFailed {
                    hook_type: hook_type.to_string(),
                    message: format!("Failed to wait for hook process: {}", e),
                })?;
                Ok(output.status.success())
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
            .with_status("completed");

        let template = "Change {change_id} iter {iteration} of {total_changes} queue {queue_size}";
        let result = context.expand_placeholders(template);
        assert_eq!(result, "Change test-change iter 2 of 5 queue 3");
    }

    #[test]
    fn test_hook_context_to_env_vars() {
        let context = HookContext::new(1, 5, 3, true).with_change("my-change", 2, 10);

        let vars = context.to_env_vars();
        assert_eq!(
            vars.get("OPENSPEC_CHANGE_ID"),
            Some(&"my-change".to_string())
        );
        assert_eq!(vars.get("OPENSPEC_ITERATION"), Some(&"1".to_string()));
        assert_eq!(vars.get("OPENSPEC_TOTAL_CHANGES"), Some(&"5".to_string()));
        assert_eq!(vars.get("OPENSPEC_QUEUE_SIZE"), Some(&"3".to_string()));
        assert_eq!(vars.get("OPENSPEC_COMPLETED_TASKS"), Some(&"2".to_string()));
        assert_eq!(vars.get("OPENSPEC_TOTAL_TASKS"), Some(&"10".to_string()));
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
}
