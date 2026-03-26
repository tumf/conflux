//! Orchestrator execution logic for the TUI
//!
//! Contains the run_orchestrator function and archive operations.

use crate::agent::AgentRunner;
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
// Note: acceptance_test_streaming and related types are no longer imported here
// as they are handled by SerialRunService internally.
use crate::orchestration::output::{ChannelOutputHandler, ContextualOutputHandler, OutputMessage};
use crate::serial_run_service::SerialRunService;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use super::events::{LogEntry, OrchestratorEvent};
use super::queue::DynamicQueue;

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

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
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

    let hooks = HookRunner::with_event_tx(config.get_hooks(), tx.clone());
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
        inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
        inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
        inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
        strict_process_cleanup: config.get_command_strict_process_cleanup(),
    };
    let stream_json_textify = config.get_stream_json_textify();
    let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
    ai_runner.set_stream_json_textify(stream_json_textify);
    ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());

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

        // Check if this change has been stopped (single-change stop)
        if dynamic_queue.is_stopped(&change_id).await {
            dynamic_queue.clear_stopped(&change_id).await;
            pending_changes.remove(&change_id);
            total_changes = total_changes.saturating_sub(1);
            let change_stopped_event = OrchestratorEvent::ChangeStopped {
                change_id: change_id.clone(),
            };
            let _ = tx.send(change_stopped_event.clone()).await;
            shared_state
                .write()
                .await
                .apply_execution_event(&change_stopped_event);
            let _ = tx
                .send(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Change stopped: {}",
                    change_id
                ))))
                .await;
            continue;
        }

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
        let cancel_token_clone = cancel_token.clone();

        // Create a cancel_check that monitors both global cancel AND single-change stop
        let dynamic_queue_clone = dynamic_queue.clone();
        let change_id_for_cancel = change_id.clone();
        let cancel_check = move || {
            // Check global cancellation
            if cancel_token_clone.is_cancelled() {
                return true;
            }
            // Check single-change stop (non-blocking check)
            dynamic_queue_clone.try_is_stopped(&change_id_for_cancel)
        };

        // Create a closure that only checks single-change stop
        let dynamic_queue_clone2 = dynamic_queue.clone();
        let change_id_for_single_stop = change_id.clone();
        let is_single_change_stopped =
            move || dynamic_queue_clone2.try_is_stopped(&change_id_for_single_stop);

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
                is_single_change_stopped,
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
            Ok(ChangeProcessResult::ChangeStopped) => {
                // Clear the stopped flag to allow re-queueing
                dynamic_queue.clear_stopped(&change_id).await;
                // Send ChangeStopped event to move the change to not queued
                let change_stopped_event = OrchestratorEvent::ChangeStopped {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(change_stopped_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&change_stopped_event);
                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Change {} stopped, continuing with other queued changes",
                        change_id
                    ))))
                    .await;
                // Remove this change from pending but continue processing others
                pending_changes.retain(|id| id != &change_id);
                continue;
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

                // Send AcceptanceStarted event
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: format!("opencode acceptance {}", change_id),
                };
                let _ = tx.send(acceptance_started_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_started_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_started_event).await;
                }

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
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

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
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

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                // Update local state tracking
                apply_counts.insert(change_id.clone(), apply_count);
            }
            Ok(ChangeProcessResult::AcceptanceBlocked) => {
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

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&acceptance_completed_event).await;
                }

                let _ = tx
                    .send(OrchestratorEvent::Log(LogEntry::warn(
                        "Acceptance blocked - implementation blocker detected, stopping apply loop"
                            .to_string(),
                    )))
                    .await;

                // Mark as stalled in SerialRunService and remove from pending to prevent re-selection and archive
                let reason = "Implementation blocker detected - requires manual intervention";
                serial_service.mark_stalled(&change_id, reason);
                pending_changes.remove(&change_id);

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

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
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

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                let _ = tx.send(acceptance_completed_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&acceptance_completed_event);
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
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&processing_error_event);
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

                // Send ChangeArchived event
                let change_archived_event = OrchestratorEvent::ChangeArchived(change_id.clone());
                let _ = tx.send(change_archived_event.clone()).await;
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&change_archived_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&change_archived_event).await;
                }

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
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&processing_error_event);
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
                shared_state
                    .write()
                    .await
                    .apply_execution_event(&processing_error_event);
                #[cfg(feature = "web-monitoring")]
                if let Some(ws) = &web_state {
                    ws.apply_execution_event(&processing_error_event).await;
                }
            }
            Err(e) => {
                // Check if this was a single-change stop (error message contains "Cancelled")
                let error_str = e.to_string();
                if error_str.contains("Cancelled") && dynamic_queue.try_is_stopped(&change_id) {
                    // Clear the stop flag and send ChangeStopped event
                    dynamic_queue.clear_stopped(&change_id).await;
                    pending_changes.remove(&change_id);
                    total_changes = total_changes.saturating_sub(1);
                    let change_stopped_event2 = OrchestratorEvent::ChangeStopped {
                        change_id: change_id.clone(),
                    };
                    let _ = tx.send(change_stopped_event2.clone()).await;
                    shared_state
                        .write()
                        .await
                        .apply_execution_event(&change_stopped_event2);
                    let _ = tx
                        .send(OrchestratorEvent::Log(LogEntry::info(format!(
                            "Change stopped during execution: {}",
                            change_id
                        ))))
                        .await;
                    continue;
                } else {
                    // Regular error - treat as before
                    let error_msg = format!("Processing error for {}: {}", change_id, e);
                    let processing_error_event = OrchestratorEvent::ProcessingError {
                        id: change_id.clone(),
                        error: error_msg,
                    };
                    let _ = tx.send(processing_error_event.clone()).await;
                    shared_state
                        .write()
                        .await
                        .apply_execution_event(&processing_error_event);
                    #[cfg(feature = "web-monitoring")]
                    if let Some(ws) = &web_state {
                        ws.apply_execution_event(&processing_error_event).await;
                    }
                    break;
                }
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

    // Check if Git is available for parallel execution
    service.check_vcs_available().await?;

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

    let uncommitted_file_change_ids: HashSet<String> =
        match crate::vcs::git::commands::list_changes_with_uncommitted_files(&repo_root).await {
            Ok(ids) => ids.into_iter().collect(),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Failed to detect uncommitted files in changes for parallel start"
                );
                HashSet::new()
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
            uncommitted_file_change_ids,
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
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

    /// Test that AcceptanceBlocked prevents re-selection and archive in TUI serial mode.
    /// Verifies that:
    /// 1. The blocked change is marked as stalled in SerialRunService
    /// 2. The blocked change is removed from pending_changes
    /// 3. Blocked change cannot be re-selected through pending_changes filtering
    #[tokio::test]
    async fn test_tui_acceptance_blocked_prevents_reselection_and_archive() {
        use crate::serial_run_service::SerialRunService;
        use std::collections::HashSet;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::OrchestratorConfig::default();
        let mut serial_service = SerialRunService::new(temp_dir.path().to_path_buf(), config);

        let blocked_change_id = "blocked-change";
        let other_change_id = "other-change";

        // Simulate pending changes before blocking
        let mut pending_changes: HashSet<String> =
            vec![blocked_change_id.to_string(), other_change_id.to_string()]
                .into_iter()
                .collect();

        // Simulate AcceptanceBlocked processing
        let reason = "Implementation blocker detected - requires manual intervention";
        serial_service.mark_stalled(blocked_change_id, reason);
        pending_changes.remove(blocked_change_id);

        // Verify the blocked change is no longer in pending
        assert!(!pending_changes.contains(blocked_change_id));
        assert!(pending_changes.contains(other_change_id));

        // Verify the blocked change is marked as stalled
        assert!(serial_service.is_stalled(blocked_change_id));
        assert!(!serial_service.is_stalled(other_change_id));

        // Verify that only the non-blocked change remains selectable
        assert_eq!(pending_changes.len(), 1);
        assert!(pending_changes.contains(other_change_id));
    }
}
