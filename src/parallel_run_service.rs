//! Shared parallel execution service for CLI and TUI modes.
//!
//! This module provides a unified service for running parallel execution
//! that can be used by both CLI and TUI orchestrators, eliminating
//! code duplication between the two modes.

use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::analyzer::{ParallelGroup, ParallelizationAnalyzer};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::hooks::HookRunner;
use crate::openspec::Change;
use crate::parallel::{ParallelEvent, ParallelExecutor};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Service for parallel execution of changes.
///
/// This service encapsulates the shared logic between CLI and TUI
/// parallel execution modes, including:
/// - Git availability checking
/// - Dependency-based analysis
/// - ParallelExecutor coordination
pub struct ParallelRunService {
    /// Configuration for the orchestrator
    config: OrchestratorConfig,
    /// Repository root directory
    repo_root: PathBuf,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
    /// Shared stagger state for coordinating AI command execution delays
    shared_stagger_state: SharedStaggerState,
    /// AI command runner for analyze commands
    ai_runner: AiCommandRunner,
}

impl ParallelRunService {
    /// Create a new parallel run service
    pub fn new(repo_root: PathBuf, config: OrchestratorConfig) -> Self {
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
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
        };
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        Self {
            config,
            repo_root,
            no_resume: false,
            shared_stagger_state,
            ai_runner,
        }
    }

    /// Create a new parallel run service with a shared stagger state
    pub fn new_with_shared_state(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        shared_stagger_state: SharedStaggerState,
    ) -> Self {
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
        };
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        Self {
            config,
            repo_root,
            no_resume: false,
            shared_stagger_state,
            ai_runner,
        }
    }

    /// Set whether to disable automatic workspace resume.
    ///
    /// When `no_resume` is true, existing workspaces are always deleted
    /// and new ones are created. When false (default), existing workspaces
    /// are reused to resume interrupted work.
    pub fn set_no_resume(&mut self, no_resume: bool) {
        self.no_resume = no_resume;
    }

    /// Check if git is available for parallel execution
    ///
    /// Returns an error if git repository is not available for parallel execution.
    pub async fn check_vcs_available(&self) -> Result<()> {
        if !crate::cli::check_parallel_available() {
            return Err(crate::error::OrchestratorError::GitCommand(
                "Git repository not available for parallel execution".to_string(),
            ));
        }
        Ok(())
    }

    /// Create a configured ParallelExecutor instance with optional shared queue change state.
    ///
    /// This allows external callers to share queue change timestamps across multiple executors,
    /// enabling debounce logic to work across re-analysis iterations.
    pub fn create_executor_with_queue_state(
        &self,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        cancel_token: Option<CancellationToken>,
        shared_queue_change: Option<std::sync::Arc<tokio::sync::Mutex<Option<std::time::Instant>>>>,
        dynamic_queue: Option<std::sync::Arc<crate::tui::queue::DynamicQueue>>,
        manual_resolve_counter: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    ) -> ParallelExecutor {
        let vcs_backend = self.config.get_vcs_backend();
        let mut executor = ParallelExecutor::with_backend_and_queue_and_stagger(
            self.repo_root.clone(),
            self.config.clone(),
            event_tx,
            vcs_backend,
            shared_queue_change,
            Some(self.shared_stagger_state.clone()),
        );
        executor.set_no_resume(self.no_resume);

        // Set hooks from config
        let hooks = HookRunner::new(self.config.get_hooks());
        executor.set_hooks(hooks);

        if let Some(token) = cancel_token {
            executor.set_cancel_token(token);
        }
        if let Some(queue) = dynamic_queue {
            executor.set_dynamic_queue(queue);
        }
        if let Some(counter) = manual_resolve_counter {
            executor.set_manual_resolve_counter(counter);
        }
        executor
    }

    async fn filter_committed_changes(
        &self,
        changes: Vec<Change>,
    ) -> Result<(Vec<Change>, Vec<String>)> {
        let committed_change_ids: HashSet<String> =
            match crate::vcs::git::commands::list_changes_in_head(&self.repo_root).await {
                Ok(ids) => ids.into_iter().collect(),
                Err(err) => {
                    warn!(
                        "Failed to load committed change snapshot; assuming all changes are committed: {}",
                        err
                    );
                    return Ok((changes, Vec::new()));
                }
            };

        let mut committed = Vec::new();
        let mut skipped = Vec::new();

        for change in changes {
            if committed_change_ids.contains(&change.id) {
                committed.push(change);
            } else {
                skipped.push(change.id);
            }
        }

        skipped.sort();
        Ok((committed, skipped))
    }

    /// Prepare changes for parallel execution: filter committed changes and send warning event if needed.
    ///
    /// This helper consolidates the preparation logic shared across multiple execution paths:
    /// 1. Filters changes to only include those committed to the repository
    /// 2. Sends a warning event if uncommitted changes are skipped (before any state update)
    /// 3. Returns the filtered changes, or None if no committed changes remain
    ///
    /// The event is sent synchronously before returning to maintain proper event ordering.
    async fn prepare_parallel_execution(
        &self,
        changes: Vec<Change>,
        event_tx: &mpsc::Sender<ParallelEvent>,
    ) -> Result<Option<Vec<Change>>> {
        let (changes, skipped) = self.filter_committed_changes(changes).await?;

        // Send warning event BEFORE any state update to maintain event order
        if !skipped.is_empty() {
            let message = format!(
                "Skipping uncommitted changes in parallel mode: {}",
                skipped.join(", ")
            );
            warn!("{}", message);
            let _ = event_tx
                .send(ParallelEvent::Warning {
                    title: "Uncommitted changes skipped".to_string(),
                    message,
                })
                .await;
        }

        if changes.is_empty() {
            info!("No committed changes available for parallel execution");
            return Ok(None);
        }

        Ok(Some(changes))
    }

    /// Run parallel execution with an event callback
    ///
    /// The event_handler receives ParallelEvents as they occur during execution.
    /// Returns the execution result.
    ///
    /// This method now uses `execute_with_reanalysis` for dynamic re-analysis,
    /// matching the TUI behavior and aligning with the spec requirement for
    /// unified CLI/TUI execution paths.
    pub async fn run_parallel<F>(
        &self,
        changes: Vec<Change>,
        cancel_token: Option<CancellationToken>,
        event_handler: F,
    ) -> Result<()>
    where
        F: Fn(ParallelEvent) + Send + Sync + 'static,
    {
        // Create event channel
        let (event_tx, mut event_rx) = mpsc::channel::<ParallelEvent>(100);

        // Prepare changes using the common helper (sends warning event if needed)
        let changes = match self.prepare_parallel_execution(changes, &event_tx).await? {
            Some(changes) => changes,
            None => return Ok(()),
        };

        info!(
            "Starting parallel execution with re-analysis for {} changes",
            changes.len()
        );

        // Spawn event forwarding task
        let forward_handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let is_completed =
                    matches!(event, ParallelEvent::AllCompleted | ParallelEvent::Stopped);
                event_handler(event);
                if is_completed {
                    break;
                }
            }
        });

        // Create and run executor with re-analysis (same as TUI), passing shared stagger state
        let mut executor = ParallelExecutor::with_backend_and_queue_and_stagger(
            self.repo_root.clone(),
            self.config.clone(),
            Some(event_tx.clone()),
            self.config.get_vcs_backend(),
            None,
            Some(self.shared_stagger_state.clone()),
        );
        executor.set_no_resume(self.no_resume);

        // Set hooks from config
        let hooks = HookRunner::new(self.config.get_hooks());
        executor.set_hooks(hooks);

        // Set cancel token if provided
        if let Some(token) = cancel_token {
            executor.set_cancel_token(token);
        }

        // Clone config and shared stagger state for the analyzer closure
        let config = self.config.clone();
        let repo_root = self.repo_root.clone();
        let shared_stagger_state = self.shared_stagger_state.clone();

        // Use order-based execution (aligned with spec)
        let result = executor
            .execute_with_order_based_reanalysis(
                changes,
                move |remaining, in_flight_ids, iteration| {
                    let config = config.clone();
                    let repo_root = repo_root.clone();
                    let event_tx = event_tx.clone();
                    let shared_stagger_state = shared_stagger_state.clone();
                    Box::pin(async move {
                        let service = ParallelRunService::new_with_shared_state(
                            repo_root,
                            config,
                            shared_stagger_state,
                        );
                        service
                            .analyze_order_with_sender(
                                remaining,
                                in_flight_ids,
                                Some(&event_tx),
                                iteration,
                            )
                            .await
                    })
                },
            )
            .await;

        // Wait for event forwarding to complete
        let _ = forward_handle.await;

        result
    }

    /// Run parallel execution with an mpsc sender for events and optional shared queue change state.
    ///
    /// This variant is useful when integrating with existing channel-based
    /// event systems (e.g., TUI).
    ///
    /// Uses dynamic re-analysis: after each dispatch iteration completes, the remaining changes
    /// are re-analyzed to determine the next dispatch.
    ///
    /// The `shared_queue_change` parameter allows tracking queue changes across multiple
    /// re-analysis iterations for proper debouncing behavior.
    pub async fn run_parallel_with_channel_and_queue_state(
        &self,
        changes: Vec<Change>,
        event_tx: mpsc::Sender<ParallelEvent>,
        cancel_token: Option<CancellationToken>,
        shared_queue_change: Option<std::sync::Arc<tokio::sync::Mutex<Option<std::time::Instant>>>>,
        dynamic_queue: Option<std::sync::Arc<crate::tui::queue::DynamicQueue>>,
        manual_resolve_counter: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    ) -> Result<()> {
        let executor = self.create_executor_with_queue_state(
            Some(event_tx.clone()),
            cancel_token,
            shared_queue_change,
            dynamic_queue,
            manual_resolve_counter,
        );
        // Use order-based execution (aligned with spec)
        self.run_parallel_order_based_with_executor(executor, changes, event_tx)
            .await
    }

    /// Run parallel execution with order-based analysis using a pre-configured executor.
    ///
    /// This is the preferred execution method that aligns with the parallel-execution spec.
    /// Uses `order` directly to select changes based on available slots.
    pub async fn run_parallel_order_based_with_executor(
        &self,
        mut executor: ParallelExecutor,
        changes: Vec<Change>,
        event_tx: mpsc::Sender<ParallelEvent>,
    ) -> Result<()> {
        // Prepare changes using the common helper (sends warning event if needed)
        let changes = match self.prepare_parallel_execution(changes, &event_tx).await? {
            Some(changes) => changes,
            None => return Ok(()),
        };

        info!(
            "Starting order-based parallel execution with re-analysis for {} changes",
            changes.len()
        );

        let config = self.config.clone();
        let repo_root = self.repo_root.clone();
        let shared_stagger_state = self.shared_stagger_state.clone();

        // Use order-based execution
        executor
            .execute_with_order_based_reanalysis(
                changes,
                move |remaining, in_flight_ids, iteration| {
                    let config = config.clone();
                    let repo_root = repo_root.clone();
                    let event_tx = event_tx.clone();
                    let shared_stagger_state = shared_stagger_state.clone();
                    Box::pin(async move {
                        let service = ParallelRunService::new_with_shared_state(
                            repo_root,
                            config,
                            shared_stagger_state,
                        );
                        service
                            .analyze_order_with_sender(
                                remaining,
                                in_flight_ids,
                                Some(&event_tx),
                                iteration,
                            )
                            .await
                    })
                },
            )
            .await
    }

    /// Analyze changes and group them for parallel execution (public API).
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    pub async fn analyze_and_group_public(&self, changes: &[Change]) -> Vec<ParallelGroup> {
        self.analyze_and_group(changes).await
    }

    /// Analyze changes and group them for parallel execution.
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    async fn analyze_and_group(&self, changes: &[Change]) -> Vec<ParallelGroup> {
        self.analyze_and_group_with_sender(changes, None, 1).await
    }

    /// Analyze changes and return order-based result with optional event sender.
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, returns all changes in a single order (no dependency inference).
    /// When a sender is provided, AnalysisOutput events are sent for streaming output.
    ///
    /// # Arguments
    /// * `changes` - Changes to analyze for execution order
    /// * `in_flight_ids` - Currently executing change IDs (not selectable, but available as dependencies)
    /// * `event_tx` - Optional event sender for streaming output
    /// * `iteration` - Current iteration number
    async fn analyze_order_with_sender(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
        iteration: u32,
    ) -> crate::analyzer::AnalysisResult {
        // Check if LLM analysis is enabled (default: true)
        if self.config.use_llm_analysis() {
            info!("Using LLM analysis for parallelization (analyze_command)");
            match self
                .analyze_order_with_llm_streaming(changes, in_flight_ids, event_tx, iteration)
                .await
            {
                Ok(result) => {
                    info!(
                        "LLM analysis successful: {} changes in order",
                        result.order.len()
                    );
                    return result;
                }
                Err(e) => {
                    error!("LLM analysis failed: {}", e);
                    warn!(
                        "Falling back to running all changes in parallel (no dependency analysis)"
                    );
                }
            }
        } else {
            info!("LLM analysis disabled, running all changes in parallel");
        }

        // Fallback: all changes in order with no dependencies
        crate::analyzer::AnalysisResult {
            order: changes.iter().map(|c| c.id.clone()).collect(),
            dependencies: HashMap::new(),
            groups: None,
        }
    }

    /// Analyze changes and group them for parallel execution with optional event sender.
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    /// When a sender is provided, AnalysisOutput events are sent for streaming output.
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze_order_with_sender()` for order-based execution.
    async fn analyze_and_group_with_sender(
        &self,
        changes: &[Change],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
        iteration: u32,
    ) -> Vec<ParallelGroup> {
        // Check if LLM analysis is enabled (default: true)
        if self.config.use_llm_analysis() {
            info!("Using LLM analysis for parallelization (analyze_command)");
            match self
                .analyze_with_llm_streaming(changes, event_tx, iteration)
                .await
            {
                Ok(groups) => {
                    info!("LLM analysis successful: {} groups", groups.len());
                    return groups;
                }
                Err(e) => {
                    error!("LLM analysis failed: {}", e);
                    warn!(
                        "Falling back to running all changes in parallel (no dependency analysis)"
                    );
                }
            }
        } else {
            info!("LLM analysis disabled, running all changes in parallel");
        }

        // Fall back: run all changes in a single parallel group
        Self::all_parallel(changes)
    }

    /// Create a single group with all changes (no dependencies, full parallelism)
    fn all_parallel(changes: &[Change]) -> Vec<ParallelGroup> {
        if changes.is_empty() {
            return Vec::new();
        }

        vec![ParallelGroup {
            id: 1,
            changes: changes.iter().map(|c| c.id.clone()).collect(),
            depends_on: Vec::new(),
        }]
    }

    /// Analyze changes using LLM and return raw analysis result (order + dependencies)
    ///
    /// # Arguments
    /// * `changes` - Changes to analyze for execution order
    /// * `in_flight_ids` - Currently executing change IDs (not selectable, but available as dependencies)
    /// * `event_tx` - Optional event sender for streaming output
    /// * `iteration` - Current iteration number
    async fn analyze_order_with_llm_streaming(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
        iteration: u32,
    ) -> Result<crate::analyzer::AnalysisResult> {
        let analyzer = ParallelizationAnalyzer::new(self.ai_runner.clone(), self.config.clone());

        if let Some(tx) = event_tx {
            let tx = tx.clone();
            analyzer
                .analyze_with_callback(changes, in_flight_ids, move |output| {
                    let _ = tx.try_send(ParallelEvent::AnalysisOutput {
                        output: output.clone(),
                        iteration,
                    });
                })
                .await
        } else {
            analyzer.analyze_with_inflight(changes, in_flight_ids).await
        }
    }

    /// Analyze changes using LLM (analyze_command) with streaming output
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze_order_with_llm_streaming()` for order-based execution.
    async fn analyze_with_llm_streaming(
        &self,
        changes: &[Change],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
        iteration: u32,
    ) -> Result<Vec<ParallelGroup>> {
        let analyzer = ParallelizationAnalyzer::new(self.ai_runner.clone(), self.config.clone());

        if let Some(tx) = event_tx {
            let tx = tx.clone();
            analyzer
                .analyze_groups_with_callback(changes, move |output| {
                    let _ = tx.try_send(ParallelEvent::AnalysisOutput {
                        output: output.clone(),
                        iteration,
                    });
                })
                .await
        } else {
            analyzer.analyze_groups(changes).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

    fn create_test_change(id: &str, dependencies: Vec<&str>) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "1m ago".to_string(),
            is_approved: true,
            dependencies: dependencies.into_iter().map(String::from).collect(),
        }
    }

    async fn init_git_repo(temp_dir: &TempDir) -> bool {
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let init_ok = init_result
            .as_ref()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !init_ok {
            return false;
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        true
    }

    #[tokio::test]
    async fn test_filter_committed_changes_skips_uncommitted() {
        let temp_dir = TempDir::new().expect("tempdir");
        if !init_git_repo(&temp_dir).await {
            return;
        }

        let base_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a/proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b/proposal.md"), "test").unwrap();

        let service =
            ParallelRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let changes = vec![
            create_test_change("change-a", vec![]),
            create_test_change("change-b", vec![]),
        ];

        let (committed, skipped) = service
            .filter_committed_changes(changes)
            .await
            .expect("filter changes");

        let committed_ids: Vec<String> = committed.into_iter().map(|change| change.id).collect();
        assert_eq!(committed_ids, vec!["change-a".to_string()]);
        assert_eq!(skipped, vec!["change-b".to_string()]);
    }
}
