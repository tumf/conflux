//! Agent runner implementation for executing commands.

use super::history_ops;
use super::output::OutputLine;
use super::prompt::{build_acceptance_prompt, build_apply_prompt, build_archive_prompt};
use crate::command_queue::{CommandQueue, CommandQueueConfig, StreamingOutputLine};
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::{AcceptanceAttempt, AcceptanceHistory, ApplyHistory, ArchiveHistory};
use crate::process_manager::{ManagedChild, StreamingChildHandle};
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Manages agent process execution based on configuration
pub struct AgentRunner {
    config: OrchestratorConfig,
    /// Command execution queue with staggered start and retry mechanism
    command_queue: CommandQueue,
    /// History of apply attempts per change for context injection
    apply_history: ApplyHistory,
    /// History of archive attempts per change for context injection
    archive_history: ArchiveHistory,
    /// History of acceptance attempts per change for context injection
    acceptance_history: AcceptanceHistory,
    /// Tracks which changes have had acceptance tail injected (to prevent re-injection)
    acceptance_tail_injected: std::collections::HashMap<String, bool>,
}

impl AgentRunner {
    /// Create a new AgentRunner with the given configuration
    pub fn new(config: OrchestratorConfig) -> Self {
        // Build command queue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
        };

        Self {
            config,
            command_queue: CommandQueue::new(queue_config),
            apply_history: ApplyHistory::new(),
            archive_history: ArchiveHistory::new(),
            acceptance_history: AcceptanceHistory::new(),
            acceptance_tail_injected: std::collections::HashMap::new(),
        }
    }

    /// Create a new AgentRunner with shared stagger state.
    ///
    /// This allows multiple AgentRunner instances to coordinate their
    /// stagger delays through a shared last_execution timestamp.
    /// Useful for parallel execution modes where multiple runners
    /// need to avoid simultaneous command starts.
    ///
    /// # Arguments
    ///
    /// * `config` - Orchestrator configuration
    /// * `shared_state` - Shared last execution timestamp (Arc<Mutex<Option<Instant>>>)
    #[allow(dead_code)] // Infrastructure ready, integration pending (tasks 4.1-4.3)
    pub fn new_with_shared_state(
        config: OrchestratorConfig,
        shared_state: Arc<Mutex<Option<Instant>>>,
    ) -> Self {
        // Build command queue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
        };

        Self {
            config,
            command_queue: CommandQueue::new_with_shared_state(queue_config, shared_state),
            apply_history: ApplyHistory::new(),
            archive_history: ArchiveHistory::new(),
            acceptance_history: AcceptanceHistory::new(),
            acceptance_tail_injected: std::collections::HashMap::new(),
        }
    }

    /// Run apply command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_apply_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    #[allow(dead_code)] // Replaced by run_apply_streaming_with_runner
    pub async fn run_apply_streaming(
        &mut self,
        change_id: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        // Get acceptance tail first (requires &mut self)
        let acceptance_tail = self.get_acceptance_tail_context_for_apply(change_id);

        // Then get immutable data
        let template = self.config.get_apply_command()?;
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + acceptance_tail + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context, &acceptance_tail);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(
                    &command,
                    dir,
                    Some("apply"),
                    Some(change_id),
                )
                .await?
            }
            None => {
                self.execute_shell_command_streaming(&command, Some("apply"), Some(change_id))
                    .await?
            }
        };
        Ok((child, rx, start))
    }

    /// Run apply command using AiCommandRunner with streaming output.
    /// This ensures apply commands share stagger state with acceptance/archive/resolve.
    /// Returns a child process handle, a receiver for output lines, a start time, and the command string.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_apply_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    pub async fn run_apply_streaming_with_runner(
        &mut self,
        change_id: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
        cwd: Option<&Path>,
    ) -> Result<(
        StreamingChildHandle,
        mpsc::Receiver<OutputLine>,
        Instant,
        String,
    )> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let start = Instant::now();
        // Get acceptance tail first (requires &mut self)
        let acceptance_tail = self.get_acceptance_tail_context_for_apply(change_id);

        // Then get immutable data
        let template = self.config.get_apply_command()?;
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + acceptance_tail + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context, &acceptance_tail);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command via AiCommandRunner: {}", command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (child, ai_rx) = ai_runner
            .execute_streaming_with_retry(&command, cwd, Some("apply"), Some(change_id))
            .await?;

        // Convert AiCommandRunner output to AgentRunner output format
        let (tx, rx) = mpsc::channel::<OutputLine>(1024);
        tokio::spawn(async move {
            let mut ai_rx = ai_rx;
            while let Some(line) = ai_rx.recv().await {
                let converted = match line {
                    AiOutputLine::Stdout(s) => OutputLine::Stdout(s),
                    AiOutputLine::Stderr(s) => OutputLine::Stderr(s),
                };
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });

        Ok((child, rx, start, command))
    }

    /// Record an apply attempt after streaming execution completes.
    /// Call this after `run_apply_streaming()` child process finishes.
    pub fn record_apply_attempt(
        &mut self,
        change_id: &str,
        status: &ExitStatus,
        start: Instant,
        stdout_tail: Option<String>,
        stderr_tail: Option<String>,
    ) {
        history_ops::record_apply_attempt(
            &mut self.apply_history,
            change_id,
            status,
            start,
            stdout_tail,
            stderr_tail,
        );
    }

    /// Run archive command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_archive_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + history_context
    /// - user_prompt: from config.archive_prompt (user-customizable)
    /// - history_context: previous archive attempts (if any)
    pub async fn run_archive_streaming(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant)> {
        let start = Instant::now();
        let template = self.config.get_archive_command()?;
        let user_prompt = self.config.get_archive_prompt();
        let history_context = self.archive_history.format_context(change_id);

        // Build full prompt: user_prompt + history_context
        let full_prompt = build_archive_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running archive command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(
                    &command,
                    dir,
                    Some("archive"),
                    Some(change_id),
                )
                .await?
            }
            None => {
                self.execute_shell_command_streaming(&command, Some("archive"), Some(change_id))
                    .await?
            }
        };
        Ok((child, rx, start))
    }

    /// Run archive command using AiCommandRunner with streaming output.
    /// This ensures archive commands share stagger state with acceptance/apply/resolve.
    /// Returns a child process handle, a receiver for output lines, a start time, and the command string.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_archive_attempt()`.
    ///
    /// The prompt is constructed as: user_prompt + history_context
    /// - user_prompt: from config.archive_prompt (user-customizable)
    /// - history_context: previous archive attempts (if any)
    #[allow(dead_code)]
    pub async fn run_archive_streaming_with_runner(
        &self,
        change_id: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
        cwd: Option<&Path>,
    ) -> Result<(
        StreamingChildHandle,
        mpsc::Receiver<OutputLine>,
        Instant,
        String,
    )> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let start = Instant::now();
        let template = self.config.get_archive_command()?;
        let user_prompt = self.config.get_archive_prompt();
        let history_context = self.archive_history.format_context(change_id);

        // Build full prompt: user_prompt + history_context
        let full_prompt = build_archive_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running archive command via AiCommandRunner: {}", command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (child, ai_rx) = ai_runner
            .execute_streaming_with_retry(&command, cwd, Some("archive"), Some(change_id))
            .await?;

        // Convert AiCommandRunner output to AgentRunner output format
        let (tx, rx) = mpsc::channel::<OutputLine>(1024);
        tokio::spawn(async move {
            let mut ai_rx = ai_rx;
            while let Some(line) = ai_rx.recv().await {
                let converted = match line {
                    AiOutputLine::Stdout(s) => OutputLine::Stdout(s),
                    AiOutputLine::Stderr(s) => OutputLine::Stderr(s),
                };
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });

        Ok((child, rx, start, command))
    }

    /// Run apply command for the given change ID (blocking, no streaming)
    /// Records the attempt result in history for subsequent retries.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    #[allow(dead_code)] // Replaced by run_apply_with_runner in CLI/TUI flows
    pub async fn run_apply(&mut self, change_id: &str) -> Result<ExitStatus> {
        let start = Instant::now();

        // Get acceptance tail first (requires &mut self)
        let acceptance_tail = self.get_acceptance_tail_context_for_apply(change_id);

        // Then get immutable data
        let template = self.config.get_apply_command()?;
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + acceptance_tail + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context, &acceptance_tail);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command: {}", command
        );

        let status = self.execute_shell_command(&command).await?;

        // Record the attempt
        history_ops::record_apply_attempt(
            &mut self.apply_history,
            change_id,
            &status,
            start,
            None,
            None,
        );

        Ok(status)
    }

    /// Run apply command using AiCommandRunner (blocking, with output collection)
    /// This ensures apply commands share stagger state with acceptance/archive/resolve.
    ///
    /// The prompt is constructed as: user_prompt + system_prompt + history_context
    /// - user_prompt: from config.apply_prompt (user-customizable)
    /// - system_prompt: APPLY_SYSTEM_PROMPT constant (always included)
    /// - history_context: previous apply attempts (if any)
    pub async fn run_apply_with_runner(
        &mut self,
        change_id: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
    ) -> Result<ExitStatus> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let start = Instant::now();

        // Get acceptance tail first (requires &mut self)
        let acceptance_tail = self.get_acceptance_tail_context_for_apply(change_id);

        // Then get immutable data
        let template = self.config.get_apply_command()?;
        let user_prompt = self.config.get_apply_prompt();
        let history_context = self.apply_history.format_context(change_id);

        // Build full prompt: user_prompt + system_prompt + acceptance_tail + history_context
        let full_prompt = build_apply_prompt(user_prompt, &history_context, &acceptance_tail);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running apply command via AiCommandRunner: {}", command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (mut child, mut output_rx) = ai_runner
            .execute_streaming_with_retry(&command, None, Some("apply"), Some(change_id))
            .await?;

        // Collect output for history recording
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();

        while let Some(line) = output_rx.recv().await {
            match line {
                AiOutputLine::Stdout(s) => stdout_lines.push(s),
                AiOutputLine::Stderr(s) => stderr_lines.push(s),
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|e| OrchestratorError::AgentCommand(format!("Apply command failed: {}", e)))?;

        // Collect last N lines for history
        let stdout_tail = if stdout_lines.len() > 10 {
            Some(stdout_lines[stdout_lines.len() - 10..].join("\n"))
        } else if !stdout_lines.is_empty() {
            Some(stdout_lines.join("\n"))
        } else {
            None
        };

        let stderr_tail = if stderr_lines.len() > 10 {
            Some(stderr_lines[stderr_lines.len() - 10..].join("\n"))
        } else if !stderr_lines.is_empty() {
            Some(stderr_lines.join("\n"))
        } else {
            None
        };

        // Record the attempt
        history_ops::record_apply_attempt(
            &mut self.apply_history,
            change_id,
            &status,
            start,
            stdout_tail,
            stderr_tail,
        );

        Ok(status)
    }

    /// Record an archive attempt after streaming execution completes.
    /// Call this after `run_archive_streaming()` child process finishes.
    pub fn record_archive_attempt(
        &mut self,
        change_id: &str,
        status: &ExitStatus,
        start: Instant,
        verification_result: Option<String>,
        stdout_tail: Option<String>,
        stderr_tail: Option<String>,
    ) {
        history_ops::record_archive_attempt(
            &mut self.archive_history,
            change_id,
            status,
            start,
            verification_result,
            stdout_tail,
            stderr_tail,
        );
    }

    /// Clear apply history for a change (call after archiving)
    pub fn clear_apply_history(&mut self, change_id: &str) {
        history_ops::clear_apply_history(&mut self.apply_history, change_id);
    }

    pub fn clear_archive_history(&mut self, change_id: &str) {
        history_ops::clear_archive_history(&mut self.archive_history, change_id);
    }

    pub fn clear_acceptance_history(&mut self, change_id: &str) {
        history_ops::clear_acceptance_history(&mut self.acceptance_history, change_id);
    }

    pub fn format_acceptance_history(&self, change_id: &str) -> String {
        self.acceptance_history.format_context(change_id)
    }

    pub fn format_apply_history(&self, change_id: &str) -> String {
        self.apply_history.format_context(change_id)
    }

    #[allow(dead_code)]
    pub fn format_archive_history(&self, change_id: &str) -> String {
        self.archive_history.format_context(change_id)
    }

    /// Run acceptance command for the given change ID with output streaming.
    /// Returns a child process handle, a receiver for output lines, and a start time.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_acceptance_attempt()`.
    ///
    /// The prompt is constructed as: system_prompt + diff_context + user_prompt + history_context
    /// - system_prompt: ACCEPTANCE_SYSTEM_PROMPT constant (always included)
    /// - diff_context: changed files and previous findings (2nd+ attempts only)
    /// - user_prompt: from config.acceptance_prompt (user-customizable)
    /// - history_context: previous acceptance attempts (if any)
    #[allow(dead_code)] // Replaced by AiCommandRunner in acceptance_test_streaming
    pub async fn run_acceptance_streaming(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
        base_branch: Option<&str>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>, Instant, String)> {
        let start = Instant::now();
        let template = self.config.get_acceptance_command()?;
        let user_prompt = self.config.get_acceptance_prompt();
        let history_context = self.acceptance_history.format_context(change_id);

        // Build diff context for all attempts
        let diff_context = self
            .build_acceptance_diff_context(change_id, cwd, base_branch)
            .await?;

        // Build last acceptance output context for 2nd+ attempts
        use super::prompt::build_last_acceptance_output_context;
        let stdout_tail = self.acceptance_history.last_stdout_tail(change_id);
        let stderr_tail = self.acceptance_history.last_stderr_tail(change_id);
        let last_output_context =
            build_last_acceptance_output_context(stdout_tail.as_deref(), stderr_tail.as_deref());

        // Build prompt injected into `{prompt}`
        // NOTE: Full and ContextOnly modes now behave identically (no embedded system prompt).
        // The match is kept for clarity, but both branches produce the same result.
        let full_prompt = match self.config.get_acceptance_prompt_mode() {
            crate::config::AcceptancePromptMode::Full => build_acceptance_prompt(
                change_id,
                user_prompt,
                &history_context,
                &last_output_context,
                &diff_context,
            ),
            crate::config::AcceptancePromptMode::ContextOnly => {
                super::prompt::build_acceptance_prompt_context_only(
                    change_id,
                    user_prompt,
                    &history_context,
                    &last_output_context,
                    &diff_context,
                )
            }
        };

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running acceptance command: {}", command
        );
        let (child, rx) = match cwd {
            Some(dir) => {
                self.execute_shell_command_streaming_in_dir(
                    &command,
                    dir,
                    Some("acceptance"),
                    Some(change_id),
                )
                .await?
            }
            None => {
                self.execute_shell_command_streaming(&command, Some("acceptance"), Some(change_id))
                    .await?
            }
        };
        Ok((child, rx, start, command))
    }

    /// Run acceptance command using AiCommandRunner with streaming output.
    /// This ensures acceptance commands share stagger state with apply/archive/resolve.
    /// Returns a streaming handle, a receiver for output lines, a start time, and the command string.
    /// The caller is responsible for recording the attempt after the child completes
    /// by calling `record_acceptance_attempt()`.
    ///
    /// The prompt is constructed as: system_prompt + diff_context + user_prompt + history_context
    /// - system_prompt: ACCEPTANCE_SYSTEM_PROMPT constant (always included)
    /// - diff_context: changed files and previous findings (2nd+ attempts only)
    /// - user_prompt: from config.acceptance_prompt (user-customizable)
    /// - history_context: previous acceptance attempts (if any)
    pub async fn run_acceptance_streaming_with_runner(
        &self,
        change_id: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
        cwd: Option<&Path>,
        base_branch: Option<&str>,
    ) -> Result<(
        StreamingChildHandle,
        mpsc::Receiver<OutputLine>,
        Instant,
        String,
    )> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let start = Instant::now();
        let template = self.config.get_acceptance_command()?;
        let user_prompt = self.config.get_acceptance_prompt();
        let history_context = self.acceptance_history.format_context(change_id);

        // Build diff context for all attempts
        let diff_context = self
            .build_acceptance_diff_context(change_id, cwd, base_branch)
            .await?;

        // Build last acceptance output context for 2nd+ attempts
        use super::prompt::build_last_acceptance_output_context;
        let stdout_tail = self.acceptance_history.last_stdout_tail(change_id);
        let stderr_tail = self.acceptance_history.last_stderr_tail(change_id);
        let last_output_context =
            build_last_acceptance_output_context(stdout_tail.as_deref(), stderr_tail.as_deref());

        // Build prompt injected into `{prompt}`
        // NOTE: Full and ContextOnly modes now behave identically (no embedded system prompt).
        // The match is kept for clarity, but both branches produce the same result.
        let full_prompt = match self.config.get_acceptance_prompt_mode() {
            crate::config::AcceptancePromptMode::Full => build_acceptance_prompt(
                change_id,
                user_prompt,
                &history_context,
                &last_output_context,
                &diff_context,
            ),
            crate::config::AcceptancePromptMode::ContextOnly => {
                super::prompt::build_acceptance_prompt_context_only(
                    change_id,
                    user_prompt,
                    &history_context,
                    &last_output_context,
                    &diff_context,
                )
            }
        };

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running acceptance command via AiCommandRunner: {}", command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (child, ai_rx) = ai_runner
            .execute_streaming_with_retry(&command, cwd, Some("acceptance"), Some(change_id))
            .await?;

        // Convert AiCommandRunner output to AgentRunner output format
        let (tx, rx) = mpsc::channel::<OutputLine>(1024);
        tokio::spawn(async move {
            let mut ai_rx = ai_rx;
            while let Some(line) = ai_rx.recv().await {
                let converted = match line {
                    AiOutputLine::Stdout(s) => OutputLine::Stdout(s),
                    AiOutputLine::Stderr(s) => OutputLine::Stderr(s),
                };
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });

        Ok((child, rx, start, command))
    }

    /// Build acceptance diff context for all acceptance attempts.
    /// - 1st attempt: Shows files changed from base_branch to current commit
    /// - 2nd+ attempts: Shows files changed since last acceptance check
    async fn build_acceptance_diff_context(
        &self,
        change_id: &str,
        cwd: Option<&Path>,
        base_branch: Option<&str>,
    ) -> Result<String> {
        use super::prompt::build_acceptance_diff_context;

        // Get repository path
        let repo_path = cwd.unwrap_or_else(|| Path::new("."));

        // Get current commit hash
        let current_commit = crate::vcs::git::commands::get_current_commit(repo_path)
            .await
            .map_err(|e| {
                OrchestratorError::GitCommand(format!("Failed to get current commit hash: {}", e))
            })?;

        // Determine the base commit for diff
        let base_commit = if self.acceptance_history.count(change_id) == 0 {
            // First acceptance: use base branch if provided
            base_branch.map(|b| b.to_string())
        } else {
            // 2nd+ acceptance: use last acceptance commit
            self.acceptance_history.last_commit_hash(change_id)
        };

        // Get changed files between base and current commit
        let changed_files = if let Some(ref base) = base_commit {
            crate::vcs::git::commands::get_changed_files(repo_path, Some(base), &current_commit)
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to get changed files: {}", e))
                })?
        } else {
            // No base commit available (non-git repo or first attempt without base_branch)
            Vec::new()
        };

        // Get previous findings (only for 2nd+ attempts)
        let previous_findings = if self.acceptance_history.count(change_id) > 0 {
            self.acceptance_history.last_findings(change_id)
        } else {
            None
        };

        // Build diff context only if there are changed files or previous findings
        if changed_files.is_empty() && previous_findings.is_none() {
            return Ok(String::new());
        }

        let findings_slice = previous_findings.as_deref();
        Ok(build_acceptance_diff_context(
            &changed_files,
            findings_slice,
        ))
    }

    /// Record an acceptance attempt after streaming execution completes.
    /// Call this after `run_acceptance_streaming()` child process finishes.
    pub fn record_acceptance_attempt(&mut self, change_id: &str, attempt: AcceptanceAttempt) {
        history_ops::record_acceptance_attempt(&mut self.acceptance_history, change_id, attempt);
        // Reset acceptance tail injection flag so the next apply can receive the new output
        self.reset_acceptance_tail_injection(change_id);
    }

    /// Get the next acceptance attempt number for a change.
    pub fn next_acceptance_attempt_number(&self, change_id: &str) -> u32 {
        history_ops::next_acceptance_attempt_number(&self.acceptance_history, change_id)
    }

    /// Get the count of consecutive CONTINUE attempts for a change.
    pub fn count_consecutive_acceptance_continues(&self, change_id: &str) -> u32 {
        history_ops::count_consecutive_acceptance_continues(&self.acceptance_history, change_id)
    }

    /// Get the last acceptance attempt for a change.
    /// Returns None if there are no previous attempts.
    #[allow(dead_code)] // Reserved for future direct use
    pub fn get_last_acceptance_attempt(
        &self,
        change_id: &str,
    ) -> Option<&crate::history::AcceptanceAttempt> {
        self.acceptance_history.get_last_attempt(change_id)
    }

    /// Get the last stdout tail from the most recent acceptance attempt.
    /// Returns None if there are no previous attempts or the last attempt has no stdout tail.
    pub fn get_last_acceptance_stdout_tail(&self, change_id: &str) -> Option<String> {
        self.acceptance_history.last_stdout_tail(change_id)
    }

    /// Get the last stderr tail from the most recent acceptance attempt.
    /// Returns None if there are no previous attempts or the last attempt has no stderr tail.
    pub fn get_last_acceptance_stderr_tail(&self, change_id: &str) -> Option<String> {
        self.acceptance_history.last_stderr_tail(change_id)
    }

    /// Get acceptance tail context for apply prompt.
    /// Returns the formatted context block if:
    /// 1. There is a previous acceptance attempt with output
    /// 2. The tail has not been injected yet for this change
    ///
    /// This ensures the tail is only injected on the first apply retry after acceptance failure.
    pub fn get_acceptance_tail_context_for_apply(&mut self, change_id: &str) -> String {
        // Check if we've already injected the tail for this change
        if self
            .acceptance_tail_injected
            .get(change_id)
            .copied()
            .unwrap_or(false)
        {
            return String::new();
        }

        // Get stdout/stderr tails from the last acceptance attempt
        let stdout_tail = self.acceptance_history.last_stdout_tail(change_id);
        let stderr_tail = self.acceptance_history.last_stderr_tail(change_id);

        // If we have output, build the context and mark as injected
        use super::prompt::build_last_acceptance_output_context;
        let context =
            build_last_acceptance_output_context(stdout_tail.as_deref(), stderr_tail.as_deref());

        if !context.is_empty() {
            // Mark as injected
            self.acceptance_tail_injected
                .insert(change_id.to_string(), true);
        }

        context
    }

    /// Peek at the acceptance tail context without consuming the injection flag.
    /// This is useful for display purposes (e.g., TUI command logging) where we want
    /// to show what will be sent without affecting the actual execution.
    pub fn peek_acceptance_tail_context_for_apply(&self, change_id: &str) -> String {
        // Check if we've already injected the tail for this change
        if self
            .acceptance_tail_injected
            .get(change_id)
            .copied()
            .unwrap_or(false)
        {
            return String::new();
        }

        // Get stdout/stderr tails from the last acceptance attempt
        let stdout_tail = self.acceptance_history.last_stdout_tail(change_id);
        let stderr_tail = self.acceptance_history.last_stderr_tail(change_id);

        // Build the context without marking as injected
        use super::prompt::build_last_acceptance_output_context;
        build_last_acceptance_output_context(stdout_tail.as_deref(), stderr_tail.as_deref())
    }

    /// Reset acceptance tail injection flag for a change.
    /// This should be called when a new acceptance attempt is recorded,
    /// so the next apply retry can receive the new acceptance output.
    fn reset_acceptance_tail_injection(&mut self, change_id: &str) {
        self.acceptance_tail_injected.remove(change_id);
    }

    /// Run archive command for the given change ID (blocking, no streaming)
    #[allow(dead_code)] // Replaced by run_archive_with_runner in CLI/TUI flows
    pub async fn run_archive(&self, change_id: &str) -> Result<ExitStatus> {
        let template = self.config.get_archive_command()?;
        let prompt = self.config.get_archive_prompt();
        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, prompt);
        info!(
            module = module_path!(),
            "Running archive command: {}", command
        );
        self.execute_shell_command(&command).await
    }

    /// Run archive command using AiCommandRunner (blocking, with output collection)
    /// This ensures archive commands share stagger state with acceptance/apply/resolve.
    ///
    /// Note: This method does NOT record archive attempts in history.
    /// The caller is responsible for calling record_archive_attempt() after verification.
    pub async fn run_archive_with_runner(
        &self,
        change_id: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
    ) -> Result<ExitStatus> {
        let template = self.config.get_archive_command()?;
        let user_prompt = self.config.get_archive_prompt();
        let history_context = self.archive_history.format_context(change_id);

        // Build full prompt: user_prompt + history_context
        let full_prompt = build_archive_prompt(user_prompt, &history_context);

        let command = OrchestratorConfig::expand_change_id(template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);
        info!(
            module = module_path!(),
            "Running archive command via AiCommandRunner: {}", command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (mut child, mut output_rx) = ai_runner
            .execute_streaming_with_retry(&command, None, Some("archive"), Some(change_id))
            .await?;

        // Drain output (not needed for archive, but required to complete execution)
        while let Some(_line) = output_rx.recv().await {
            // Discard output - archive doesn't need it for history
        }

        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Archive command failed: {}", e))
        })?;

        Ok(status)
    }

    /// Analyze dependencies using the configured analyze command (blocking)
    pub async fn analyze_dependencies(&self, prompt: &str) -> Result<String> {
        let template = self.config.get_analyze_command()?;
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running analyze command: {}", template
        );
        info!("Expanded command length: {} chars", command.len());

        let output = self.execute_shell_command_with_output(&command).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)?;
        info!(
            "Analyze command completed, output length: {} chars",
            stdout.len()
        );
        debug!("Raw analysis output: {}", stdout);

        // Extract result from stream-json format if applicable
        let result = self.extract_stream_json_result(&stdout);
        info!("Extracted result length: {} chars", result.len());
        debug!("Extracted analysis result: {}", result);
        Ok(result)
    }

    /// Analyze dependencies using AiCommandRunner (blocking, with output collection)
    /// This ensures analyze commands share stagger state with acceptance/apply/archive/resolve.
    pub async fn analyze_dependencies_with_runner(
        &self,
        prompt: &str,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
    ) -> Result<String> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let template = self.config.get_analyze_command()?;
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running analyze command via AiCommandRunner: {}", template
        );
        info!("Expanded command length: {} chars", command.len());

        // Execute via AiCommandRunner (with shared stagger state)
        let (mut child, mut output_rx) = ai_runner
            .execute_streaming_with_retry(&command, None, Some("analyze"), None)
            .await?;

        // Collect stdout for result
        let mut stdout_lines = Vec::new();
        let mut stderr_lines = Vec::new();

        while let Some(line) = output_rx.recv().await {
            match line {
                AiOutputLine::Stdout(s) => stdout_lines.push(s),
                AiOutputLine::Stderr(s) => stderr_lines.push(s),
            }
        }

        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Analyze command failed: {}", e))
        })?;

        if !status.success() {
            let stderr = stderr_lines.join("\n");
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed: {}",
                stderr
            )));
        }

        let stdout = stdout_lines.join("\n");
        info!(
            "Analyze command completed, output length: {} chars",
            stdout.len()
        );
        debug!("Raw analysis output: {}", stdout);

        // Extract result from stream-json format if applicable
        let result = self.extract_stream_json_result(&stdout);
        info!("Extracted result length: {} chars", result.len());
        debug!("Extracted analysis result: {}", result);
        Ok(result)
    }

    /// Analyze dependencies using the configured analyze command with streaming output
    /// Returns a child process handle and a receiver for output lines
    #[allow(dead_code)] // Replaced by AiCommandRunner in ParallelizationAnalyzer
    pub async fn analyze_dependencies_streaming(
        &self,
        prompt: &str,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_analyze_command()?;
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running analyze command (streaming): {}", template
        );
        self.execute_shell_command_streaming(&command, Some("analyze"), None)
            .await
    }

    /// Execute resolve command with streaming output in a specific directory.
    /// Returns a child process handle and a receiver for output lines.
    #[allow(dead_code)] // Replaced by run_resolve_streaming_in_dir_with_runner in ensure_archive_commit
    pub async fn run_resolve_streaming_in_dir(
        &self,
        prompt: &str,
        cwd: &Path,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        let template = self.config.get_resolve_command()?;
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running resolve command (streaming) in {:?}: {}", cwd, command
        );
        self.execute_shell_command_streaming_in_dir(&command, cwd, Some("resolve"), None)
            .await
    }

    /// Execute resolve command using AiCommandRunner with streaming output in a specific directory.
    /// This ensures resolve commands share stagger state with acceptance/apply/archive/analyze.
    /// Returns a child process handle and a receiver for output lines.
    pub async fn run_resolve_streaming_in_dir_with_runner(
        &self,
        prompt: &str,
        cwd: &Path,
        ai_runner: &crate::ai_command_runner::AiCommandRunner,
    ) -> Result<(StreamingChildHandle, mpsc::Receiver<OutputLine>)> {
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let template = self.config.get_resolve_command()?;
        let command = OrchestratorConfig::expand_prompt(template, prompt);
        info!(
            module = module_path!(),
            "Running resolve command via AiCommandRunner (streaming) in {:?}: {}", cwd, command
        );

        // Execute via AiCommandRunner (with shared stagger state)
        let (child, ai_rx) = ai_runner
            .execute_streaming_with_retry(&command, Some(cwd), Some("resolve"), None)
            .await?;

        // Convert AiCommandRunner output to AgentRunner output format
        let (tx, rx) = mpsc::channel::<OutputLine>(1024);
        tokio::spawn(async move {
            let mut ai_rx = ai_rx;
            while let Some(line) = ai_rx.recv().await {
                let converted = match line {
                    AiOutputLine::Stdout(s) => OutputLine::Stdout(s),
                    AiOutputLine::Stderr(s) => OutputLine::Stderr(s),
                };
                if tx.send(converted).await.is_err() {
                    break;
                }
            }
        });

        Ok((child, rx))
    }

    /// Extract the result from stream-json output format
    /// stream-json outputs multiple JSON lines, the last one with type="result" contains the actual result
    fn extract_stream_json_result(&self, output: &str) -> String {
        // Try to find and parse the result line from stream-json output
        for line in output.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                // Check if this is a result message
                if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                    if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                        return result.to_string();
                    }
                }
                // Also check for assistant message content
                if json.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                    if let Some(message) = json.get("message") {
                        if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                            for item in content {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }

        // If not stream-json format, return as-is
        output.to_string()
    }

    /// Execute a shell command with output streaming and automatic retry
    /// Returns a child process handle and a receiver for output lines
    ///
    /// This function uses the command queue's retry logic to automatically retry
    /// transient failures. Retry notifications are sent through the output channel.
    async fn execute_shell_command_streaming(
        &self,
        command: &str,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        // Create output channel
        let (tx, rx) = mpsc::channel::<OutputLine>(100);

        // Clone tx for callback
        let tx_clone = tx.clone();
        let textify = self.config.get_stream_json_textify();
        let text_buf = Arc::new(Mutex::new(
            crate::stream_json_textifier::StreamJsonTextBuffer::new(),
        ));
        let text_buf_cb = text_buf.clone();

        // Create callback to forward streaming output
        let output_callback = move |line: StreamingOutputLine| {
            let tx = tx_clone.clone();
            let buf = text_buf_cb.clone();
            async move {
                match line {
                    StreamingOutputLine::Stdout(s) => {
                        if textify {
                            let mut buf = buf.lock().await;
                            for l in crate::stream_json_textifier::process_stdout_line(&s, &mut buf)
                            {
                                let _ = tx.send(OutputLine::Stdout(l)).await;
                            }
                        } else {
                            let _ = tx.send(OutputLine::Stdout(s)).await;
                        }
                    }
                    StreamingOutputLine::Stderr(s) => {
                        let _ = tx.send(OutputLine::Stderr(s)).await;
                    }
                }
            }
        };

        // Clone command queue and command string for background task
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();
        let operation_type_owned = operation_type.map(|s| s.to_string());
        let change_id_owned = change_id.map(|s| s.to_string());

        // Create oneshot channel to communicate final status
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<ExitStatus>();

        // Spawn background task to run retry logic
        tokio::spawn(async move {
            let result = command_queue
                .execute_with_retry_streaming(
                    || build_command(&command_str),
                    Some(output_callback),
                    operation_type_owned.as_deref(),
                    change_id_owned.as_deref(),
                )
                .await;

            // Flush any partial line remaining in the text buffer
            if textify {
                let mut buf = text_buf.lock().await;
                if let Some(tail) = buf.finalize() {
                    if !tail.is_empty() {
                        let _ = tx.send(OutputLine::Stdout(tail)).await;
                    }
                }
            }

            // Send final status
            match result {
                Ok((status, _stderr)) => {
                    let _ = status_tx.send(status);
                }
                Err(_) => {
                    // Send failure status
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        let _ = status_tx.send(ExitStatus::from_raw(1));
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, we can't create ExitStatus easily
                        // Just drop the channel
                    }
                }
            }

            // Close output channel
            drop(tx);
        });

        // Create a dummy child process that waits for stdin to close
        // When the background task completes, we'll close stdin which causes the process to exit
        let mut dummy_child = create_dummy_child()?;

        // Take stdin so we can close it later
        let dummy_stdin = dummy_child.stdin.take();

        // Wrap dummy child in ManagedChild
        let managed_child = ManagedChild::new(dummy_child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        // Spawn a task to close stdin when real command completes
        // This will cause the dummy process to exit cleanly
        tokio::spawn(async move {
            // Wait for status from background task
            let _ = status_rx.await;
            // Close stdin - this causes cat/findstr to exit cleanly
            drop(dummy_stdin);
        });

        Ok((managed_child, rx))
    }

    /// Execute a shell command with output streaming and automatic retry in a specific directory
    /// Returns a child process handle and a receiver for output lines
    ///
    /// This function uses the command queue's retry logic to automatically retry
    /// transient failures. Retry notifications are sent through the output channel.
    async fn execute_shell_command_streaming_in_dir(
        &self,
        command: &str,
        cwd: &Path,
        operation_type: Option<&str>,
        change_id: Option<&str>,
    ) -> Result<(ManagedChild, mpsc::Receiver<OutputLine>)> {
        // Create output channel
        let (tx, rx) = mpsc::channel::<OutputLine>(100);

        // Clone tx for callback
        let tx_clone = tx.clone();
        let textify = self.config.get_stream_json_textify();
        let text_buf = Arc::new(Mutex::new(
            crate::stream_json_textifier::StreamJsonTextBuffer::new(),
        ));
        let text_buf_cb = text_buf.clone();

        // Create callback to forward streaming output
        let output_callback = move |line: StreamingOutputLine| {
            let tx = tx_clone.clone();
            let buf = text_buf_cb.clone();
            async move {
                match line {
                    StreamingOutputLine::Stdout(s) => {
                        if textify {
                            let mut buf = buf.lock().await;
                            for l in crate::stream_json_textifier::process_stdout_line(&s, &mut buf)
                            {
                                let _ = tx.send(OutputLine::Stdout(l)).await;
                            }
                        } else {
                            let _ = tx.send(OutputLine::Stdout(s)).await;
                        }
                    }
                    StreamingOutputLine::Stderr(s) => {
                        let _ = tx.send(OutputLine::Stderr(s)).await;
                    }
                }
            }
        };

        // Clone command queue, command string, and cwd for background task
        let command_queue = self.command_queue.clone();
        let command_str = command.to_string();
        let cwd_path = cwd.to_path_buf();
        let operation_type_owned = operation_type.map(|s| s.to_string());
        let change_id_owned = change_id.map(|s| s.to_string());

        // Create oneshot channel to communicate final status
        let (status_tx, status_rx) = tokio::sync::oneshot::channel::<ExitStatus>();

        // Spawn background task to run retry logic
        tokio::spawn(async move {
            let result = command_queue
                .execute_with_retry_streaming(
                    || build_command_in_dir(&command_str, &cwd_path),
                    Some(output_callback),
                    operation_type_owned.as_deref(),
                    change_id_owned.as_deref(),
                )
                .await;

            // Flush any partial line remaining in the text buffer
            if textify {
                let mut buf = text_buf.lock().await;
                if let Some(tail) = buf.finalize() {
                    if !tail.is_empty() {
                        let _ = tx.send(OutputLine::Stdout(tail)).await;
                    }
                }
            }

            // Send final status
            match result {
                Ok((status, _stderr)) => {
                    let _ = status_tx.send(status);
                }
                Err(_) => {
                    // Send failure status
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        let _ = status_tx.send(ExitStatus::from_raw(1));
                    }
                    #[cfg(not(unix))]
                    {
                        // On Windows, we can't create ExitStatus easily
                        // Just drop the channel
                    }
                }
            }

            // Close output channel
            drop(tx);
        });

        // Create a dummy child process that waits for stdin to close
        // When the background task completes, we'll close stdin which causes the process to exit
        let mut dummy_child = create_dummy_child()?;

        // Take stdin so we can close it later
        let dummy_stdin = dummy_child.stdin.take();

        // Wrap dummy child in ManagedChild
        let managed_child = ManagedChild::new(dummy_child).map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to create managed child: {}", e))
        })?;

        // Spawn a task to close stdin when real command completes
        // This will cause the dummy process to exit cleanly
        tokio::spawn(async move {
            // Wait for status from background task
            let _ = status_rx.await;
            // Close stdin - this causes cat/findstr to exit cleanly
            drop(dummy_stdin);
        });

        Ok((managed_child, rx))
    }

    /// Execute a shell command and wait for completion (blocking, no streaming)
    /// Now uses CommandQueue for stagger delay and retry logic
    #[allow(dead_code)] // Used by run_apply/run_archive (legacy non-streaming methods)
    async fn execute_shell_command(&self, command: &str) -> Result<ExitStatus> {
        debug!(
            module = module_path!(),
            "Executing shell command with stagger/retry: {}", command
        );

        // Use command queue for stagger and retry
        let command_str = command.to_string();
        let (status, _stderr) = self
            .command_queue
            .execute_with_retry_streaming(
                || build_command(&command_str),
                None::<
                    fn(
                        StreamingOutputLine,
                    )
                        -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
                >,
                None,
                None,
            )
            .await?;

        debug!("Command exited with status: {:?}", status);
        Ok(status)
    }

    /// Execute a shell command and capture its output
    /// Now uses CommandQueue for stagger delay and retry logic
    async fn execute_shell_command_with_output(
        &self,
        command: &str,
    ) -> Result<std::process::Output> {
        debug!(
            module = module_path!(),
            "Executing shell command with output capture (stagger/retry): {}", command
        );

        // Collect stdout/stderr via callback
        let stdout_lines = Arc::new(Mutex::new(Vec::new()));
        let stderr_lines = Arc::new(Mutex::new(Vec::new()));

        let stdout_clone = stdout_lines.clone();
        let stderr_clone = stderr_lines.clone();

        let output_callback = move |line: StreamingOutputLine| {
            let stdout = stdout_clone.clone();
            let stderr = stderr_clone.clone();
            async move {
                match line {
                    StreamingOutputLine::Stdout(s) => {
                        stdout.lock().await.push(s);
                    }
                    StreamingOutputLine::Stderr(s) => {
                        stderr.lock().await.push(s);
                    }
                }
            }
        };

        // Use command queue for stagger and retry
        let command_str = command.to_string();
        let (status, _stderr_buf) = self
            .command_queue
            .execute_with_retry_streaming(
                || build_command(&command_str),
                Some(output_callback),
                None,
                None,
            )
            .await?;

        // Reconstruct Output from collected lines
        let stdout_vec = stdout_lines.lock().await;
        let stderr_vec = stderr_lines.lock().await;

        let stdout_bytes = stdout_vec.join("\n").into_bytes();
        let stderr_bytes = stderr_vec.join("\n").into_bytes();

        // Create a synthetic Output struct
        #[cfg(unix)]
        let output = std::process::Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(
                status.code().unwrap_or(1) << 8,
            ),
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        };

        #[cfg(windows)]
        let output = std::process::Output {
            status: std::os::windows::process::ExitStatusExt::from_raw(
                status.code().unwrap_or(1) as u32
            ),
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        };

        Ok(output)
    }

    /// Get the underlying configuration
    pub fn config(&self) -> &OrchestratorConfig {
        &self.config
    }
}

/// Build a command for execution (extracted for use with command queue)
fn build_command(command: &str) -> Command {
    if cfg!(target_os = "windows") {
        debug!("Building shell command: cmd /C {}", command);
        let mut cmd = Command::new("cmd");
        cmd.arg("/C")
            .arg(command)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            .env("CI", "true")
            // Disable pagers
            .env("PAGER", "type")
            .env("GIT_PAGER", "type")
            .env("LESS", "")
            .env("MORE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    } else {
        // Use login shell to load .zprofile/.profile for PATH and environment setup
        // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        debug!("Building shell command: {} -l -c {}", shell, command);
        let mut cmd = Command::new(&shell);
        cmd.arg("-l")
            .arg("-c")
            .arg(command)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            // Disable interactive features
            .env("CI", "true")
            .env("CONTINUOUS_INTEGRATION", "true")
            .env("NON_INTERACTIVE", "1")
            // Disable pagers completely
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("LESS", "-FX") // -F: quit if one screen, -X: no init
            .env("MORE", "-E") // -E: quit at EOF
            // Prevent any pager from being used
            .env("MANPAGER", "cat")
            .env("SYSTEMD_PAGER", "cat")
            // Disable git interactive features
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            // Detach from controlling TTY (setsid) to avoid job-control stops (STAT=T).
            // Fall back to process-group creation if setsid fails.
            crate::process_manager::configure_process_group(&mut cmd);
        }

        cmd
    }
}

/// Build a command for execution in a specific directory
fn build_command_in_dir(command: &str, cwd: &Path) -> Command {
    if cfg!(target_os = "windows") {
        debug!("Building shell command: cmd /C {}", command);
        let mut cmd = Command::new("cmd");
        cmd.arg("/C")
            .arg(command)
            .current_dir(cwd)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            .env("CI", "true")
            // Disable pagers
            .env("PAGER", "type")
            .env("GIT_PAGER", "type")
            .env("LESS", "")
            .env("MORE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    } else {
        // Use login shell to load .zprofile/.profile for PATH and environment setup
        // Note: -l (login) instead of -i (interactive) to avoid job control issues with TUI
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        debug!("Building shell command: {} -l -c {}", shell, command);
        let mut cmd = Command::new(&shell);
        cmd.arg("-l")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .env_clear()
            .envs(std::env::vars())
            // Disable terminal-related environment variables
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            // Disable interactive features
            .env("CI", "true")
            .env("CONTINUOUS_INTEGRATION", "true")
            .env("NON_INTERACTIVE", "1")
            // Disable pagers completely
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("LESS", "-FX") // -F: quit if one screen, -X: no init
            .env("MORE", "-E") // -E: quit at EOF
            // Prevent any pager from being used
            .env("MANPAGER", "cat")
            .env("SYSTEMD_PAGER", "cat")
            // Disable git interactive features
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        {
            // Detach from controlling TTY (setsid) to avoid job-control stops (STAT=T).
            // Fall back to process-group creation if setsid fails.
            crate::process_manager::configure_process_group(&mut cmd);
        }

        cmd
    }
}

/// Create a dummy child process that waits for stdin to close
fn create_dummy_child() -> Result<tokio::process::Child> {
    let child = if cfg!(target_os = "windows") {
        // Use findstr with stdin - will exit when stdin closes
        Command::new("findstr")
            .arg(".*")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(OrchestratorError::Io)?
    } else {
        // Use cat with no args - reads stdin until it closes
        Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(OrchestratorError::Io)?
    };
    Ok(child)
}
