//! Orchestrator execution logic for the TUI
//!
//! Contains the run_orchestrator function and archive operations.

use crate::agent::AgentRunner;
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::history::OutputCollector;
use crate::openspec::Change;
// Note: acceptance_test_streaming and related types are no longer imported here
// as they are handled by SerialRunService internally.
use crate::orchestration::output::{ChannelOutputHandler, ContextualOutputHandler, OutputMessage};
use crate::serial_run_service::SerialRunService;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use super::events::{LogEntry, OrchestratorEvent};
use super::queue::DynamicQueue;

/// Context for archive operations
/// Note: This is legacy code from Phase 1 archive processing.
/// It may be removed in the future as SerialRunService handles archiving.
#[allow(dead_code)]
pub struct ArchiveContext {
    pub changes_processed: usize,
    pub total_changes: usize,
    pub remaining_changes: usize,
    pub apply_count: u32,
}

/// Result of archive operation
/// Note: This is legacy code from Phase 1 archive processing.
/// It may be removed in the future as SerialRunService handles archiving.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ArchiveResult {
    Success,
    Failed,
    Cancelled,
}

fn apply_pending_removals(
    pending_changes: &mut HashSet<String>,
    processed_change_ids: &mut Vec<String>,
    apply_counts: &mut HashMap<String, u32>,
    removed_ids: Vec<String>,
) -> Vec<String> {
    if removed_ids.is_empty() {
        return Vec::new();
    }

    let mut removed_pending = Vec::new();
    for id in removed_ids {
        if pending_changes.remove(&id) {
            processed_change_ids.retain(|existing| existing != &id);
            apply_counts.remove(&id);
            removed_pending.push(id);
        }
    }

    removed_pending
}

/// Archive a single completed change
/// Returns Ok(ArchiveResult) indicating success, failure, or cancellation
/// Note: This is legacy code from Phase 1 archive processing.
/// It may be removed in the future as SerialRunService handles archiving.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub async fn archive_single_change(
    change_id: &str,
    change: &Change,
    agent: &mut AgentRunner,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    context: &ArchiveContext,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    #[cfg(feature = "web-monitoring")] web_state: &Option<Arc<crate::web::WebState>>,
) -> Result<ArchiveResult> {
    use crate::agent::OutputLine;
    use crate::hooks::{HookContext, HookType};

    // Run on_change_complete hook
    let complete_context = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(change_id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);
    if let Err(e) = hooks
        .run_hook(HookType::OnChangeComplete, &complete_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_change_complete hook failed: {}",
                e
            ))))
            .await;
    }

    // Run pre_archive hook
    let pre_archive_context = HookContext::new(
        context.changes_processed,
        context.total_changes,
        context.remaining_changes,
        false,
    )
    .with_change(change_id, change.completed_tasks, change.total_tasks)
    .with_apply_count(context.apply_count);
    if let Err(e) = hooks
        .run_hook(HookType::PreArchive, &pre_archive_context)
        .await
    {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "pre_archive hook failed: {}",
                e
            ))))
            .await;
    }

    // Build expanded archive command for ArchiveStarted event
    // This mirrors the logic in AgentRunner::run_archive_streaming_with_runner
    let archive_template = agent.config().get_archive_command()?;
    let archive_user_prompt = agent.config().get_archive_prompt();
    let archive_history_context = agent.format_archive_history(change_id);
    let archive_full_prompt =
        crate::agent::build_archive_prompt(archive_user_prompt, &archive_history_context);
    let archive_expanded_command =
        OrchestratorConfig::expand_change_id(archive_template, change_id);
    let archive_expanded_command =
        OrchestratorConfig::expand_prompt(&archive_expanded_command, &archive_full_prompt);

    // Archive the change - send ArchiveStarted event with expanded command
    let archive_started_event = OrchestratorEvent::ArchiveStarted {
        change_id: change_id.to_string(),
        command: archive_expanded_command,
    };
    let _ = tx.send(archive_started_event.clone()).await;
    #[cfg(feature = "web-monitoring")]
    if let Some(ws) = web_state {
        ws.apply_execution_event(&archive_started_event).await;
    }

    use crate::execution::archive::{
        build_archive_error_message, ensure_archive_commit, verify_archive_completion,
        ArchiveVerificationResult, ARCHIVE_COMMAND_MAX_RETRIES,
    };

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;

        // Run archive command via AiCommandRunner with streaming output (shared stagger state)
        let (mut child, mut output_rx, start, _command) = agent
            .run_archive_streaming_with_runner(change_id, ai_runner, None)
            .await?;

        // Create output collector for history
        let mut output_collector = OutputCollector::new();

        // Stream output to TUI log, with cancellation support
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    let _ = child.terminate();
                    let _ = child.kill().await;
                    let _ = tx
                        .send(OrchestratorEvent::Log(LogEntry::warn(
                            "Process killed due to cancellation".to_string(),
                        )))
                        .await;
                    return Ok(ArchiveResult::Cancelled);
                }
                line = output_rx.recv() => {
                    match line {
                        Some(OutputLine::Stdout(s)) => {
                            tracing::debug!("Archive stdout: {}", s);
                            output_collector.add_stdout(&s);
                            let _ = tx.send(OrchestratorEvent::Log(
                                LogEntry::info(s)
                                    .with_change_id(change_id)
                                    .with_operation("archive")
                                    .with_iteration(attempt)
                            )).await;
                        }
                        Some(OutputLine::Stderr(s)) => {
                            tracing::debug!("Archive stderr: {}", s);
                            output_collector.add_stderr(&s);
                            let _ = tx.send(OrchestratorEvent::Log(
                                LogEntry::warn(s)
                                    .with_change_id(change_id)
                                    .with_operation("archive")
                                    .with_iteration(attempt)
                            )).await;
                        }
                        None => {
                            tracing::debug!("Archive output stream closed");
                            break;
                        }
                    }
                }
            }
        }

        // Wait for child process to complete
        let status = child.wait().await.map_err(|e| {
            crate::error::OrchestratorError::AgentCommand(format!(
                "Failed to wait for archive command for change '{}': {}",
                change_id, e
            ))
        })?;

        if !status.success() {
            let error_msg = format!("Archive failed with exit code: {:?}", status.code());

            // Record the failed attempt
            agent.record_archive_attempt(
                change_id,
                &status,
                start,
                Some(error_msg.clone()),
                output_collector.stdout_tail(),
                output_collector.stderr_tail(),
            );

            // Run on_error hook
            let error_context = HookContext::new(
                context.changes_processed,
                context.total_changes,
                context.remaining_changes,
                false,
            )
            .with_change(change_id, change.completed_tasks, change.total_tasks)
            .with_apply_count(context.apply_count)
            .with_error(&error_msg);
            let _ = hooks.run_hook(HookType::OnError, &error_context).await;

            let processing_error_event = OrchestratorEvent::ProcessingError {
                id: change_id.to_string(),
                error: error_msg.clone(),
            };
            let _ = tx.send(processing_error_event.clone()).await;
            #[cfg(feature = "web-monitoring")]
            if let Some(ws) = web_state {
                ws.apply_execution_event(&processing_error_event).await;
            }
            return Ok(ArchiveResult::Failed);
        }

        let verification = verify_archive_completion(change_id, None);
        if verification.is_success() {
            // Record successful archive attempt
            agent.record_archive_attempt(
                change_id,
                &status,
                start,
                None,
                output_collector.stdout_tail(),
                output_collector.stderr_tail(),
            );
            let log_tx = tx.clone();
            let commit_result = ensure_archive_commit(
                change_id,
                Path::new("."),
                &*agent,
                ai_runner,
                crate::vcs::VcsBackend::Auto,
                move |line| {
                    let log_tx = log_tx.clone();
                    async move {
                        match line {
                            OutputLine::Stdout(text) => {
                                let _ = log_tx
                                    .send(OrchestratorEvent::Log(LogEntry::info(text)))
                                    .await;
                            }
                            OutputLine::Stderr(text) => {
                                let _ = log_tx
                                    .send(OrchestratorEvent::Log(LogEntry::warn(text)))
                                    .await;
                            }
                        }
                    }
                },
            )
            .await;

            if let Err(e) = commit_result {
                let error_msg = format!("Archive commit failed for {}: {}", change_id, e);
                let error_context = HookContext::new(
                    context.changes_processed,
                    context.total_changes,
                    context.remaining_changes,
                    false,
                )
                .with_change(change_id, change.completed_tasks, change.total_tasks)
                .with_apply_count(context.apply_count)
                .with_error(&error_msg);
                let _ = hooks.run_hook(HookType::OnError, &error_context).await;

                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.to_string(),
                    error: error_msg,
                };
                let _ = tx.send(processing_error_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }
                return Ok(ArchiveResult::Failed);
            }

            // Clear apply and archive history for the archived change
            agent.clear_apply_history(change_id);
            agent.clear_archive_history(change_id);

            // Run post_archive hook
            let post_archive_context = HookContext::new(
                context.changes_processed + 1,
                context.total_changes,
                context.remaining_changes.saturating_sub(1),
                false,
            )
            .with_change(change_id, change.completed_tasks, change.total_tasks)
            .with_apply_count(context.apply_count);
            if let Err(e) = hooks
                .run_hook(HookType::PostArchive, &post_archive_context)
                .await
            {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "post_archive hook failed: {}",
                        e
                    ))))
                    .await;
            }

            let change_archived_event = OrchestratorEvent::ChangeArchived(change_id.to_string());
            let _ = tx.send(change_archived_event.clone()).await;
            // Update shared orchestration state (marks as archived, removes from pending)
            shared_state
                .write()
                .await
                .apply_execution_event(&change_archived_event);
            #[cfg(feature = "web-monitoring")]
            if let Some(ws) = web_state {
                ws.apply_execution_event(&change_archived_event).await;
            }
            return Ok(ArchiveResult::Success);
        }

        // Verification failed - record with reason
        let verification_reason = match verification {
            ArchiveVerificationResult::NotArchived { ref change_id } => {
                format!("Change still exists at openspec/changes/{}", change_id)
            }
            _ => "Archive verification failed".to_string(),
        };
        agent.record_archive_attempt(
            change_id,
            &status,
            start,
            Some(verification_reason.clone()),
            output_collector.stdout_tail(),
            output_collector.stderr_tail(),
        );

        if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Archive verification failed for {} (attempt {}/{}): {}; retrying archive command",
                    change_id, attempt, max_attempts, verification_reason
                ))))
                .await;
            tracing::warn!(
                change_id = %change_id,
                attempt = attempt,
                max_attempts = max_attempts,
                reason = %verification_reason,
                "Archive verification failed; retrying archive command"
            );
            continue;
        }

        let error_msg = build_archive_error_message(change_id, None);
        let processing_error_event = OrchestratorEvent::ProcessingError {
            id: change_id.to_string(),
            error: error_msg,
        };
        let _ = tx.send(processing_error_event.clone()).await;
        #[cfg(feature = "web-monitoring")]
        if let Some(ws) = web_state {
            ws.apply_execution_event(&processing_error_event).await;
        }
        return Ok(ArchiveResult::Failed);
    }
}

/// Archive all complete changes from the pending set
/// Returns the number of successfully archived changes
/// Note: This is legacy code from Phase 1 archive processing.
/// It may be removed in the future as SerialRunService handles archiving.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub async fn archive_all_complete_changes(
    pending_ids: &HashSet<String>,
    agent: &mut AgentRunner,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    hooks: &crate::hooks::HookRunner,
    tx: &mpsc::Sender<OrchestratorEvent>,
    cancel_token: &CancellationToken,
    archived_set: &mut HashSet<String>,
    total_changes: usize,
    changes_processed: &mut usize,
    apply_counts: &HashMap<String, u32>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    #[cfg(feature = "web-monitoring")] web_state: &Option<Arc<crate::web::WebState>>,
) -> Result<usize> {
    use crate::openspec;

    // Log entry point with pending count
    tracing::debug!(
        pending_count = pending_ids.len(),
        "archive_all_complete_changes: checking for complete changes to archive"
    );

    // Fetch current state of all changes using native implementation
    let changes = openspec::list_changes_native()?;

    // Find complete changes that are still in pending set
    let complete_changes: Vec<Change> = changes
        .into_iter()
        .filter(|c| pending_ids.contains(&c.id) && !archived_set.contains(&c.id) && c.is_complete())
        .collect();

    tracing::debug!(
        complete_count = complete_changes.len(),
        complete_ids = ?complete_changes.iter().map(|c| &c.id).collect::<Vec<_>>(),
        "archive_all_complete_changes: found complete changes to archive"
    );

    let mut archived_count = 0;

    for change in complete_changes {
        tracing::debug!(
            change_id = %change.id,
            "archive_all_complete_changes: starting archive for change"
        );
        if cancel_token.is_cancelled() {
            break;
        }

        let remaining_changes = pending_ids.len().saturating_sub(archived_count);
        let apply_count = *apply_counts.get(&change.id).unwrap_or(&0);
        let context = ArchiveContext {
            changes_processed: *changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
        };

        // Notify processing started for this change
        let _ = tx
            .send(OrchestratorEvent::ProcessingStarted(change.id.clone()))
            .await;

        // Send ProcessingCompleted before archiving
        let _ = tx
            .send(OrchestratorEvent::ProcessingCompleted(change.id.clone()))
            .await;

        let result = archive_single_change(
            &change.id,
            &change,
            agent,
            ai_runner,
            hooks,
            tx,
            cancel_token,
            &context,
            shared_state,
            #[cfg(feature = "web-monitoring")]
            web_state,
        )
        .await?;

        tracing::debug!(
            change_id = %change.id,
            result = ?result,
            "archive_all_complete_changes: archive result for change"
        );

        match result {
            ArchiveResult::Success => {
                archived_set.insert(change.id.clone());
                archived_count += 1;
                *changes_processed += 1;
            }
            ArchiveResult::Failed => {
                // Error already logged and sent, continue to next
            }
            ArchiveResult::Cancelled => {
                break;
            }
        }
    }

    tracing::debug!(
        archived_count = archived_count,
        "archive_all_complete_changes: completed archiving loop"
    );

    Ok(archived_count)
}

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
///
/// The orchestrator uses a two-phase loop:
/// - Phase 1: Archive all complete changes before doing any apply
/// - Phase 2: Apply one incomplete change
///
/// This ensures complete changes are never skipped.
#[allow(clippy::too_many_arguments)]
pub async fn run_orchestrator(
    change_ids: Vec<String>,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
    _graceful_stop_flag: Arc<AtomicBool>,
    shared_state: Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
) -> Result<()> {
    // Note: OutputLine is no longer needed as output is handled by ChannelOutputHandler
    use crate::hooks::{HookContext, HookRunner, HookType};
    use crate::openspec;

    let hooks = HookRunner::new(config.get_hooks());
    let max_iterations = config.get_max_iterations();
    // Note: acceptance_max_continues is now handled by SerialRunService
    let mut agent = AgentRunner::new(config.clone());

    // Create AiCommandRunner for serial mode execution
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
    let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);

    // Create serial run service for shared state and helpers
    let repo_root = std::env::current_dir()?;
    let mut serial_service = SerialRunService::new(repo_root, config);

    let mut total_changes = change_ids.len();
    let mut changes_processed: usize = 0;
    // Note: current_change_id is now tracked by SerialRunService
    let mut apply_counts: HashMap<String, u32> = HashMap::new();
    let mut archived_changes: HashSet<String> = HashSet::new();
    let mut pending_changes: HashSet<String> = change_ids.iter().cloned().collect();
    let mut processed_change_ids: Vec<String> = change_ids.clone();

    // Run on_start hook
    let start_context = HookContext::new(0, total_changes, total_changes, false);
    if let Err(e) = hooks.run_hook(HookType::OnStart, &start_context).await {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_start hook failed: {}",
                e
            ))))
            .await;
    }

    // Main two-phase loop
    loop {
        // Check for cancellation before each iteration
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Check for graceful stop flag (stop after current change completes)
        if _graceful_stop_flag.load(Ordering::SeqCst) {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(
                    "Graceful stop: stopping after current change".to_string(),
                )))
                .await;
            let _ = tx.send(OrchestratorEvent::Stopped).await;
            break;
        }

        // Check max iterations limit (0 = no limit)
        let current_iteration = serial_service.iteration();
        if max_iterations > 0 && current_iteration >= max_iterations {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Max iterations ({}) reached, stopping orchestration",
                    max_iterations
                ))))
                .await;
            // Send completion event
            let _ = tx.send(OrchestratorEvent::AllCompleted).await;
            break;
        }

        // Log warning when approaching limit (80%)
        if max_iterations > 0 {
            let warning_threshold = (max_iterations as f32 * 0.8) as u32;
            if current_iteration == warning_threshold {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Approaching max iterations: {}/{}",
                        current_iteration, max_iterations
                    ))))
                    .await;
            }
        }

        // Check dynamic queue for new changes before checking if we're done
        while let Some(dynamic_id) = dynamic_queue.pop().await {
            // Skip if already archived or in pending
            if !archived_changes.contains(&dynamic_id) && !pending_changes.contains(&dynamic_id) {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Processing dynamically added: {}",
                        dynamic_id
                    ))))
                    .await;
                pending_changes.insert(dynamic_id.clone());
                processed_change_ids.push(dynamic_id);
                total_changes += 1;
            }
        }

        let removed_pending = apply_pending_removals(
            &mut pending_changes,
            &mut processed_change_ids,
            &mut apply_counts,
            dynamic_queue.drain_removed().await,
        );
        if !removed_pending.is_empty() {
            total_changes = total_changes.saturating_sub(removed_pending.len());
            for id in removed_pending {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Removed from pending queue: {}",
                        id
                    ))))
                    .await;
            }
        }

        // Check if all pending changes are done
        if pending_changes.is_empty() {
            break;
        }

        // Note: Phase 1 archive processing has been removed.
        // SerialRunService::process_change() now handles archiving automatically
        // for completed changes. Archive results are handled in Phase 2 below.

        // Check for cancellation
        if cancel_token.is_cancelled() {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Phase 2: Select and apply next change (including completed ones for archiving)
        // Fetch current state to find best candidate using native implementation
        let changes = openspec::list_changes_native()?;

        // Filter to changes in pending set (include completed changes so they can be archived)
        let eligible_changes: Vec<_> = changes
            .iter()
            .filter(|c| pending_changes.contains(&c.id))
            .cloned()
            .collect();

        // Use serial service for change selection
        let next_change = serial_service.select_next_change(&eligible_changes);

        let Some(change) = next_change else {
            // No incomplete changes found - might all be complete now
            // Loop will re-check in Phase 1
            continue;
        };

        let change_id = change.id.clone();
        let change = change.clone();

        // Notify processing started
        let processing_started_event = OrchestratorEvent::ProcessingStarted(change_id.clone());
        let _ = tx.send(processing_started_event.clone()).await;
        // Update shared orchestration state
        shared_state
            .write()
            .await
            .apply_execution_event(&processing_started_event);
        #[cfg(feature = "web-monitoring")]
        if let Some(ws) = &web_state {
            ws.apply_execution_event(&processing_started_event).await;
        }

        let remaining_changes = pending_changes.len();

        // Get current apply count for this change (before processing)
        let apply_count_before = *apply_counts.get(&change_id).unwrap_or(&0);

        // Create output handler that forwards to TUI events
        // Use Arc<RwLock<String>> to track current operation (apply/acceptance/archive/resolve)
        let tx_clone = tx.clone();
        let change_id_clone = change_id.clone();
        let apply_count_for_output = apply_count_before + 1; // Will be incremented in process_change
        let current_operation = std::sync::Arc::new(std::sync::RwLock::new("apply".to_string()));
        let current_operation_clone = current_operation.clone();
        let output = ChannelOutputHandler::new(move |msg: OutputMessage| {
            let tx = tx_clone.clone();
            let change_id = change_id_clone.clone();
            let apply_count = apply_count_for_output;
            let operation = current_operation_clone.read().unwrap().clone();
            tokio::spawn(async move {
                match msg {
                    OutputMessage::Stdout(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::info(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                    OutputMessage::Stderr(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::warn(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                    OutputMessage::Info(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::info(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                    OutputMessage::Warn(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::warn(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                    OutputMessage::Error(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::error(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                    OutputMessage::Success(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::success(s)
                                    .with_change_id(&change_id)
                                    .with_operation(&operation)
                                    .with_iteration(apply_count),
                            ))
                            .await;
                    }
                }
            });
        });

        // Wrap output handler with ContextualOutputHandler to track operation
        let output = ContextualOutputHandler::new(output, current_operation.clone());

        // Build expanded apply command for ApplyStarted event
        // This mirrors the logic in AgentRunner::run_apply_streaming_with_runner
        // Use peek method to avoid consuming the acceptance_tail_injected flag
        let acceptance_tail = agent.peek_acceptance_tail_context_for_apply(&change_id);
        let apply_template = agent.config().get_apply_command()?;
        let apply_user_prompt = agent.config().get_apply_prompt();
        let apply_history_context = agent.format_apply_history(&change_id);
        let apply_full_prompt = crate::agent::build_apply_prompt(
            apply_user_prompt,
            &apply_history_context,
            &acceptance_tail,
        );
        let apply_expanded_command =
            OrchestratorConfig::expand_change_id(apply_template, &change_id);
        let apply_expanded_command =
            OrchestratorConfig::expand_prompt(&apply_expanded_command, &apply_full_prompt);

        // Send ApplyStarted event with expanded command
        let apply_started_event = OrchestratorEvent::ApplyStarted {
            change_id: change_id.to_string(),
            command: apply_expanded_command,
        };
        let _ = tx.send(apply_started_event.clone()).await;
        shared_state
            .write()
            .await
            .apply_execution_event(&apply_started_event);
        #[cfg(feature = "web-monitoring")]
        if let Some(ws) = &web_state {
            ws.apply_execution_event(&apply_started_event).await;
        }

        // Process the change using SerialRunService
        use crate::serial_run_service::ChangeProcessResult;
        let cancel_check = || cancel_token.is_cancelled();
        let result = serial_service
            .process_change(
                &change,
                &mut agent,
                &ai_runner,
                &hooks,
                &output,
                total_changes,
                remaining_changes,
                cancel_check,
                Some(current_operation.clone()),
            )
            .await;

        // Get the apply count after processing
        let apply_count = serial_service.apply_count(&change_id);

        // Send ApplyOutput event to update iteration number
        let apply_output_event = OrchestratorEvent::ApplyOutput {
            change_id: change_id.clone(),
            output: String::new(),
            iteration: Some(apply_count),
        };
        let _ = tx.send(apply_output_event.clone()).await;
        #[cfg(feature = "web-monitoring")]
        if let Some(ws) = &web_state {
            ws.apply_execution_event(&apply_output_event).await;
        }

        match result {
            Ok(ChangeProcessResult::Cancelled) => {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(
                        "Processing cancelled".to_string(),
                    )))
                    .await;
                pending_changes.clear();
                break;
            }
            Ok(ChangeProcessResult::AcceptancePassed) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Send AcceptanceStarted event (acceptance ran and passed)
                // Build acceptance command (simplified - actual command includes diff context)
                let acceptance_command = {
                    let template = agent
                        .config()
                        .get_acceptance_command()
                        .unwrap_or("acceptance");
                    let user_prompt = agent.config().get_acceptance_prompt();
                    let history = agent.format_acceptance_history(&change_id);
                    let prompt = format!("{}\n{}", user_prompt, history);
                    let cmd = OrchestratorConfig::expand_change_id(template, &change_id);
                    OrchestratorConfig::expand_prompt(&cmd, &prompt)
                };

                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: acceptance_command,
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                // Send ProcessingCompleted event
                let processing_completed_event =
                    OrchestratorEvent::ProcessingCompleted(change_id.clone());
                let _ = tx.send(processing_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&processing_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::ApplySuccessIncomplete) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::AcceptanceContinue) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Send AcceptanceStarted event (acceptance ran and returned Continue)
                let acceptance_command = {
                    let template = agent
                        .config()
                        .get_acceptance_command()
                        .unwrap_or("acceptance");
                    let user_prompt = agent.config().get_acceptance_prompt();
                    let history = agent.format_acceptance_history(&change_id);
                    let prompt = format!("{}\n{}", user_prompt, history);
                    let cmd = OrchestratorConfig::expand_change_id(template, &change_id);
                    OrchestratorConfig::expand_prompt(&cmd, &prompt)
                };
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: acceptance_command,
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::AcceptanceContinueExceeded) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Send AcceptanceStarted event (acceptance ran and exceeded continue limit)
                let acceptance_command = {
                    let template = agent
                        .config()
                        .get_acceptance_command()
                        .unwrap_or("acceptance");
                    let user_prompt = agent.config().get_acceptance_prompt();
                    let history = agent.format_acceptance_history(&change_id);
                    let prompt = format!("{}\n{}", user_prompt, history);
                    let cmd = OrchestratorConfig::expand_change_id(template, &change_id);
                    OrchestratorConfig::expand_prompt(&cmd, &prompt)
                };
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: acceptance_command,
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::AcceptanceFailed { .. }) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Send AcceptanceStarted event (acceptance ran and failed)
                let acceptance_command = {
                    let template = agent
                        .config()
                        .get_acceptance_command()
                        .unwrap_or("acceptance");
                    let user_prompt = agent.config().get_acceptance_prompt();
                    let history = agent.format_acceptance_history(&change_id);
                    let prompt = format!("{}\n{}", user_prompt, history);
                    let cmd = OrchestratorConfig::expand_change_id(template, &change_id);
                    OrchestratorConfig::expand_prompt(&cmd, &prompt)
                };
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: acceptance_command,
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::AcceptanceCommandFailed { error }) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                let _ = tx.send(apply_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&apply_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&apply_completed_event).await;
                }

                // Send AcceptanceStarted event (acceptance ran but command failed)
                let acceptance_command = {
                    let template = agent
                        .config()
                        .get_acceptance_command()
                        .unwrap_or("acceptance");
                    let user_prompt = agent.config().get_acceptance_prompt();
                    let history = agent.format_acceptance_history(&change_id);
                    let prompt = format!("{}\n{}", user_prompt, history);
                    let cmd = OrchestratorConfig::expand_change_id(template, &change_id);
                    OrchestratorConfig::expand_prompt(&cmd, &prompt)
                };
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: acceptance_command,
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::error(format!(
                        "Acceptance command failed: {}",
                        error
                    ))))
                    .await;

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::ApplyFailed { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                let _ = tx.send(processing_error_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::Archived) => {
                // Change was complete and successfully archived
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::success(format!(
                        "Change {} archived successfully",
                        change_id
                    ))))
                    .await;

                // Update local state tracking
                archived_changes.insert(change_id.clone());
                pending_changes.remove(&change_id);
                changes_processed += 1;
                apply_counts.remove(&change_id);
            }
            Ok(ChangeProcessResult::Stalled { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                let _ = tx.send(processing_error_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }

                // Remove stalled change from pending
                pending_changes.remove(&change_id);
            }
            Ok(ChangeProcessResult::Failed { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                let _ = tx.send(processing_error_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }
            }
            Err(e) => {
                let error_msg = format!("Processing error for {}: {}", change_id, e);
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error_msg,
                };
                let _ = tx.send(processing_error_event.clone()).await;
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }
                break;
            }
        }
    }

    // Run on_finish hook after all changes processed or stopped
    let complete_context = HookContext::new(changes_processed, total_changes, 0, false);
    if let Err(e) = hooks.run_hook(HookType::OnFinish, &complete_context).await {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_finish hook failed: {}",
                e
            ))))
            .await;
    }

    // Send completion event
    let _ = tx.send(OrchestratorEvent::AllCompleted).await;

    Ok(())
}

/// Run the orchestrator in parallel mode
///
/// Executes multiple changes concurrently using git worktrees, with dependency analysis
/// and automatic workspace management.
///
/// Supports dynamic queue: continuously processes changes as slots become available,
/// without waiting for batch boundaries.
#[allow(clippy::too_many_arguments)]
pub async fn run_orchestrator_parallel(
    change_ids: Vec<String>,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
    _graceful_stop_flag: Arc<AtomicBool>,
    shared_state: Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
) -> Result<()> {
    use crate::openspec::list_changes_native;
    use crate::parallel::ParallelEvent;
    use crate::parallel_run_service::ParallelRunService;
    use std::collections::HashSet;

    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(format!(
            "Starting parallel processing of {} change(s)",
            change_ids.len()
        ))))
        .await;

    // Get repo root
    let repo_root = std::env::current_dir()?;

    // Create ParallelRunService
    let service = ParallelRunService::new(repo_root.clone(), config.clone());

    // Create shared queue change timestamp for debouncing
    let shared_queue_change = Arc::new(tokio::sync::Mutex::new(None::<std::time::Instant>));

    let mut stopped_or_cancelled = false;
    let mut had_errors = false;

    // Fetch all changes for UI refresh
    let all_changes = list_changes_native()?;

    let committed_change_ids: HashSet<String> =
        match crate::vcs::git::commands::list_changes_in_head(&repo_root).await {
            Ok(ids) => ids.into_iter().collect(),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Failed to load committed change snapshot for parallel start"
                );
                all_changes.iter().map(|change| change.id.clone()).collect()
            }
        };

    // Filter to get only changes to process
    let changes_to_process: Vec<Change> = all_changes
        .iter()
        .filter(|c| change_ids.contains(&c.id))
        .cloned()
        .collect();

    // Send initial ChangesRefreshed event with empty worktree data
    // (Worktree data will be populated during parallel execution)
    let _ = tx
        .send(OrchestratorEvent::ChangesRefreshed {
            changes: all_changes,
            committed_change_ids,
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            resolve_wait_ids: HashSet::new(),
        })
        .await;

    // Create WebState event forwarding channel and task
    #[cfg(feature = "web-monitoring")]
    let (web_event_tx, web_event_handle) = if let Some(web_state) = web_state.clone() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                web_state.apply_execution_event(&event).await;
                if matches!(
                    event,
                    crate::events::ExecutionEvent::AllCompleted
                        | crate::events::ExecutionEvent::Stopped
                ) {
                    break;
                }
            }
        });
        (Some(tx), Some(handle))
    } else {
        (None, None)
    };

    // Create event channel for forwarding to TUI
    let (parallel_tx, mut parallel_rx) = mpsc::channel::<ParallelEvent>(100);

    // Spawn event forwarding task
    let forward_tx = tx.clone();
    let forward_cancel = cancel_token.clone();
    let merge_deferred_stop = Arc::new(AtomicBool::new(false));
    let forward_merge_stop = merge_deferred_stop.clone();
    let forward_shared_state = shared_state.clone();
    #[cfg(feature = "web-monitoring")]
    let forward_web_tx = web_event_tx.clone();
    let forward_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = forward_cancel.cancelled() => {
                    break;
                }
                event = parallel_rx.recv() => {
                    match event {
                        Some(ParallelEvent::AllCompleted) => {
                            // AllCompleted signals execution completion
                            #[cfg(feature = "web-monitoring")]
                            if let Some(tx) = &forward_web_tx {
                                let _ = tx.send(ParallelEvent::AllCompleted);
                            }
                            break;
                        }
                        Some(ParallelEvent::Stopped) => {
                            forward_merge_stop.store(true, Ordering::SeqCst);
                            let _ = forward_tx.send(ParallelEvent::Stopped).await;
                            #[cfg(feature = "web-monitoring")]
                            if let Some(tx) = &forward_web_tx {
                                let _ = tx.send(ParallelEvent::Stopped);
                            }
                            break;
                        }
                        Some(parallel_event) => {
                            // Forward to TUI first (before acquiring write lock)
                            // This prevents TUI updates from being blocked when acceptance tests run for a long time
                            let _ = forward_tx.send(parallel_event.clone()).await;
                            // Apply to shared orchestration state
                            forward_shared_state
                                .write()
                                .await
                                .apply_execution_event(&parallel_event);
                            // Forward to WebState
                            #[cfg(feature = "web-monitoring")]
                            if let Some(tx) = &forward_web_tx {
                                let _ = tx.send(parallel_event);
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Execute all changes using slot-driven continuous dispatch
    let result = tokio::select! {
        _ = cancel_token.cancelled() => {
            let change_ids: Vec<String> = changes_to_process.iter().map(|c| c.id.clone()).collect();
            let cancel_msg = format!(
                "Cancelled parallel execution ({} changes: {})",
                change_ids.len(),
                change_ids.join(", ")
            );
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    cancel_msg.clone(),
                )))
                .await;
            Err(crate::error::OrchestratorError::AgentCommand(cancel_msg))
        }
        result = service.run_parallel_with_channel_and_queue_state(
            changes_to_process.clone(),
            parallel_tx,
            Some(cancel_token.clone()),
            Some(shared_queue_change.clone()),
            Some(Arc::new(dynamic_queue.clone())),
            Some(manual_resolve_counter.clone()),
        ) => {
            result
        }
    };

    // Wait for forward task to complete
    let _ = forward_handle.await;
    if merge_deferred_stop.load(Ordering::SeqCst) {
        stopped_or_cancelled = true;
    }

    match result {
        Ok(_) => {
            if merge_deferred_stop.load(Ordering::SeqCst) {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Execution stopped with deferred merges ({} changes processed)",
                        changes_to_process.len()
                    ))))
                    .await;
            } else {
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::success(format!(
                        "Execution completed ({} changes processed)",
                        changes_to_process.len()
                    ))))
                    .await;
            }
        }
        Err(e) => {
            had_errors = true;
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::error(format!(
                    "Execution failed: {}",
                    e
                ))))
                .await;
        }
    }

    // Cleanup WebState event forwarding task
    #[cfg(feature = "web-monitoring")]
    if let Some(handle) = web_event_handle {
        drop(web_event_tx);
        let _ = handle.await;
    }

    // Only send completion message and AllCompleted event if not stopped/cancelled
    if !stopped_or_cancelled {
        if had_errors {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing completed with errors".to_string(),
                )))
                .await;
        } else {
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::success(
                    "All parallel changes completed".to_string(),
                )))
                .await;
        }
        let _ = tx.send(OrchestratorEvent::AllCompleted).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Test that the archive path uses the correct directory structure.
    /// The archive path should be `openspec/changes/archive/<change_id>`,
    /// not `openspec/archive/<change_id>`.
    #[test]
    fn test_archive_path_structure() {
        let change_id = "test-change";

        // This is the correct path structure used in archive_single_change
        let change_path = Path::new("openspec/changes").join(change_id);
        let archive_path = Path::new("openspec/changes/archive").join(change_id);

        // Verify the path structure is correct
        assert_eq!(
            change_path.to_str().unwrap(),
            "openspec/changes/test-change"
        );
        assert_eq!(
            archive_path.to_str().unwrap(),
            "openspec/changes/archive/test-change"
        );

        // The archive path should be under openspec/changes/archive, not openspec/archive
        assert!(archive_path.starts_with("openspec/changes/archive"));
        assert!(!archive_path.starts_with("openspec/archive/"));
    }

    /// Test archive verification logic: when change still exists and archive doesn't,
    /// it should be considered a failed archive.
    #[test]
    fn test_archive_verification_logic() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create the directory structure
        let changes_dir = base.join("openspec/changes");
        let archive_dir = base.join("openspec/changes/archive");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::create_dir_all(&archive_dir).unwrap();

        let change_id = "my-change";

        // Scenario 1: Change exists, archive doesn't exist -> archive failed
        let change_path = changes_dir.join(change_id);
        let archive_path = archive_dir.join(change_id);
        fs::create_dir(&change_path).unwrap();

        assert!(change_path.exists());
        assert!(!archive_path.exists());
        // This condition triggers the "archive failed" error in archive_single_change
        let archive_failed = change_path.exists() && !archive_path.exists();
        assert!(archive_failed);

        // Scenario 2: Change doesn't exist (moved to archive) -> archive succeeded
        fs::remove_dir(&change_path).unwrap();
        fs::create_dir(&archive_path).unwrap();

        assert!(!change_path.exists());
        assert!(archive_path.exists());
        let archive_succeeded = !change_path.exists() || archive_path.exists();
        assert!(archive_succeeded);

        // Scenario 3: Both paths exist (edge case, shouldn't happen normally)
        fs::create_dir(&change_path).unwrap();
        assert!(change_path.exists());
        assert!(archive_path.exists());
        // If archive exists, the archive is considered successful
        let archive_ok = archive_path.exists();
        assert!(archive_ok);
    }
}
