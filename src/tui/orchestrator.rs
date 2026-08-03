//! Orchestrator execution logic for the TUI
//!
//! Contains the run_orchestrator function and archive operations.

use crate::agent::AgentRunner;
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
// Note: acceptance_test_streaming and related types are no longer imported here
// as they are handled by SerialRunService internally.
use crate::events::{EventDispatcher, EventSink};
use crate::orchestration::output::{ChannelOutputHandler, ContextualOutputHandler, OutputMessage};
use crate::parallel::PostArchiveAction;
use crate::serial_run_service::SerialRunService;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use super::events::{LogEntry, OrchestratorEvent, TuiEventSink};
use super::queue::DynamicQueue;

fn configure_parallel_post_archive_action(
    service: &mut crate::parallel_run_service::ParallelRunService,
    action: PostArchiveAction,
) {
    service.set_post_archive_action(action);
}

/// Install the invocation-scoped upstream publication runtime, if any.
///
/// A `None` runtime leaves the service untouched, which is the hard default-off
/// boundary: no coordinator is constructed, so nothing fetches, verifies,
/// pushes, or confirms.
fn configure_parallel_upstream_integration(
    service: &mut crate::parallel_run_service::ParallelRunService,
    runtime: Option<crate::upstream::UpstreamRuntime>,
) {
    if let Some(runtime) = runtime {
        service.set_upstream_integration(runtime);
    }
}

/// Bounded deadline for the scheduler's cancellation cleanup barrier.
///
/// Operator cancellation keeps polling the running scheduler future so inner
/// task abort, join draining, execution-handle release, pending merge/base-lane
/// result handling, and workspace-guard drop can complete. Exceeding this
/// deadline escalates to managed cleanup; it never reclassifies the run as an
/// execution failure.
const PARALLEL_CANCELLATION_CLEANUP_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(120);

/// How one parallel orchestration boundary run reached its terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParallelTermination {
    /// The scheduler future returned without operator cancellation.
    SchedulerReturned,
    /// Operator cancellation; the scheduler reached its cleanup barrier.
    CancelledAfterCleanup,
    /// Operator cancellation; the cleanup barrier hit its bounded deadline.
    CancelledAfterCleanupTimeout,
}

impl ParallelTermination {
    /// True when the run ended through operator cancellation.
    pub(crate) fn is_operator_cancellation(self) -> bool {
        matches!(
            self,
            ParallelTermination::CancelledAfterCleanup
                | ParallelTermination::CancelledAfterCleanupTimeout
        )
    }
}

/// Keep polling an already-running scheduler future after cancellation was
/// requested, under a bounded cleanup deadline.
///
/// The scheduler future is deliberately not dropped on cancellation: dropping it
/// would abandon inner abort/drain, execution-handle release, pending merge
/// result handling, and workspace-guard drop. A deadline overrun returns without
/// a scheduler result so the caller can escalate managed cleanup, and the
/// outcome stays operator cancellation either way.
pub(crate) async fn drain_cancelled_scheduler<T, F>(
    scheduler: F,
    cleanup_deadline: std::time::Duration,
) -> (ParallelTermination, Option<T>)
where
    F: std::future::Future<Output = T>,
{
    match tokio::time::timeout(cleanup_deadline, scheduler).await {
        Ok(result) => (ParallelTermination::CancelledAfterCleanup, Some(result)),
        Err(_) => (ParallelTermination::CancelledAfterCleanupTimeout, None),
    }
}

/// Terminal reporting class for one parallel orchestration boundary run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParallelTerminalReport {
    /// Normal completion: success log and `AllCompleted`.
    Completed,
    /// The run drained, but one or more changes ended in a change-local failure
    /// whose evidence is preserved for explicit retry: warning and
    /// `AllCompleted`, no success message and no Error.
    CompletedWithErrors,
    /// Genuine execution error: failure log, completion-with-errors, `AllCompleted`.
    Failed,
    /// Operator stop or scheduler-reported stop: one stop diagnostic only, with
    /// no execution-failure, completion, or `AllCompleted` output.
    Stopped,
}

/// Classify terminal reporting for one parallel orchestration boundary run.
///
/// Operator cancellation is a stopped outcome, never an agent-command failure,
/// even when the bounded cleanup barrier had to escalate. Only a scheduler that
/// returned on its own may report a genuine execution error.
///
/// `scheduler_completed_with_errors` is the scheduler's own typed report, not an
/// inference from diagnostics: a run that drained while changes remain in manual
/// `MergeWait` is neither a success nor a failure, and reporting it as either
/// would be untruthful about work the operator still owns.
pub(crate) fn classify_parallel_terminal_report(
    termination: ParallelTermination,
    scheduler_failed: bool,
    scheduler_reported_stop: bool,
    reducer_owned_lane_wait_or_active: bool,
    scheduler_completed_with_errors: bool,
) -> ParallelTerminalReport {
    if termination.is_operator_cancellation() || scheduler_reported_stop {
        return ParallelTerminalReport::Stopped;
    }
    if scheduler_failed {
        return ParallelTerminalReport::Failed;
    }
    if reducer_owned_lane_wait_or_active {
        return ParallelTerminalReport::Stopped;
    }
    if scheduler_completed_with_errors {
        return ParallelTerminalReport::CompletedWithErrors;
    }
    ParallelTerminalReport::Completed
}

fn post_archive_dispatch_event(
    state: &crate::orchestration::state::OrchestratorState,
    change_id: &str,
) -> Option<OrchestratorEvent> {
    let has_resolve_lane_blocker = state.has_other_post_archive_lane_blocker(change_id);

    if has_resolve_lane_blocker {
        return Some(OrchestratorEvent::MergeDeferred {
            change_id: change_id.to_string(),
            reason: "Resolve lane occupied by active resolving/rejecting change; auto-queue archived change".to_string(),
            auto_resumable: true,
        });
    }

    None
}

/// Buffer for the dispatch bridge handed to `mpsc`-only producers.
///
/// Matches the TUI event channel it ultimately feeds, so bridging cannot make a
/// producer block earlier than emitting straight to the frontend would have.
const EVENT_BRIDGE_BUFFER: usize = 100;

/// How long a boundary run waits for bridged producers to drain before it
/// publishes its terminal event.
const EVENT_BRIDGE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Build the frontend sinks attached to one orchestration boundary run.
///
/// The TUI channel and the web monitoring state are peers here: both are
/// frontends of the same dispatch owner, neither reapplies the event to the
/// reducer, and adding or removing one cannot change how many times a counter
/// advances.
fn boundary_event_sinks(
    tx: &mpsc::Sender<OrchestratorEvent>,
    #[cfg(feature = "web-monitoring")] web_state: Option<&Arc<crate::web::WebState>>,
) -> Vec<Arc<dyn EventSink>> {
    // Without `web-monitoring` the TUI is the only frontend, so nothing pushes.
    #[allow(unused_mut)]
    let mut sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(TuiEventSink::new(tx.clone()))];
    #[cfg(feature = "web-monitoring")]
    if let Some(ws) = web_state {
        sinks.push(Arc::new(crate::web::state::WebEventSink::new(ws.clone())));
    }
    sinks
}

async fn initialize_parallel_shared_state(
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    change_ids: &[String],
    max_iterations: u32,
) -> bool {
    let mut state = shared_state.write().await;
    let resolve_wait_ids = state.resolve_wait_change_ids();
    let preserve_manual_resolve_startup = change_ids.is_empty() && !resolve_wait_ids.is_empty();

    if preserve_manual_resolve_startup {
        tracing::info!(
            resolve_wait_ids = ?resolve_wait_ids,
            "Preserving reducer-owned ResolveWait during empty manual resolve scheduler startup"
        );
        // Do not replace the reducer state here: TuiCommand::ResolveMerge just recorded
        // scheduler-owned ResolveWait intent, and ParallelRunService must observe it via
        // executor.has_resolve_wait() to avoid a zero-change no-op. Only switch the mode so
        // any subsequent execution events use parallel semantics.
        state.set_execution_mode(crate::orchestration::state::ExecutionMode::Parallel);
        true
    } else {
        *state = crate::orchestration::state::OrchestratorState::with_mode(
            change_ids.to_vec(),
            max_iterations,
            crate::orchestration::state::ExecutionMode::Parallel,
        );
        // Re-apply queue intent for each selected change so that the initial
        // ChangesRefreshed display sync (apply_display_statuses_from_reducer) does
        // not regress these rows from Queued back to NotQueued before analysis starts.
        for id in change_ids {
            state.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                id.clone(),
            ));
        }
        false
    }
}

/// Run the orchestrator for selected changes
/// Uses streaming output to send log entries in real-time
/// Supports cancellation via CancellationToken for graceful shutdown
#[allow(clippy::too_many_arguments)]
pub async fn run_orchestrator(
    change_ids: Vec<String>,
    explicit_retry: bool,
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

    let repo_root = std::env::current_dir()?;

    // One dispatch owner for the whole boundary run. Every producer below emits
    // through it — directly, or through `event_bridge` when it can only speak
    // `mpsc::Sender` — so no producer can reach one frontend while bypassing
    // another.
    let dispatcher = Arc::new(EventDispatcher::new(
        shared_state.clone(),
        boundary_event_sinks(
            &tx,
            #[cfg(feature = "web-monitoring")]
            web_state.as_ref(),
        ),
    ));
    let (event_bridge, event_bridge_handle) = dispatcher.bridge(EVENT_BRIDGE_BUFFER);

    let hooks = HookRunner::with_event_tx(config.get_hooks(), &repo_root, event_bridge.clone());
    let max_iterations = config.get_max_iterations();
    // Note: acceptance_max_continues is now handled by SerialRunService
    let mut agent = AgentRunner::new(config.clone());

    // Create AiCommandRunner for serial mode execution
    let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
    let ai_runner = AiCommandRunner::from_orchestrator_config(&config, shared_stagger_state);

    // Create serial run service for shared state and helpers
    let repo_root = std::env::current_dir()?;
    let mut serial_service = SerialRunService::new(repo_root, config);
    if explicit_retry {
        for change_id in &change_ids {
            serial_service
                .consume_explicit_acceptance_retry(change_id)
                .await?;
        }
    }

    {
        let mut state = shared_state.write().await;
        *state = crate::orchestration::state::OrchestratorState::new(change_ids, max_iterations);
    }

    // Run on_start hook
    let total_changes = shared_state.read().await.total_changes();
    let start_context = HookContext::new(0, total_changes, total_changes, false);
    if let Err(e) = hooks.run_hook(HookType::OnStart, &start_context).await {
        dispatcher
            .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_start hook failed: {}",
                e
            ))))
            .await;
    }

    // Main two-phase loop
    loop {
        // Check for cancellation before each iteration
        if cancel_token.is_cancelled() {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Check for graceful stop flag (stop after current change completes)
        if _graceful_stop_flag.load(Ordering::SeqCst) {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::info(
                    "Graceful stop: stopping after current change".to_string(),
                )))
                .await;
            dispatcher.dispatch(OrchestratorEvent::Stopped).await;
            break;
        }

        // Check max iterations limit (0 = no limit)
        let current_iteration = serial_service.iteration();
        if max_iterations > 0 && current_iteration >= max_iterations {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Max iterations ({}) reached, stopping orchestration",
                    max_iterations
                ))))
                .await;
            // Send completion event
            dispatcher.dispatch(OrchestratorEvent::AllCompleted).await;
            break;
        }

        // Log warning when approaching limit (80%)
        if max_iterations > 0 {
            let warning_threshold = (max_iterations as f32 * 0.8) as u32;
            if current_iteration == warning_threshold {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Approaching max iterations: {}/{}",
                        current_iteration, max_iterations
                    ))))
                    .await;
            }
        }

        // Check dynamic queue for new changes before checking if we're done
        while let Some(dynamic_id) = dynamic_queue.pop().await {
            // Skip if already archived or in pending
            let should_add = {
                let state = shared_state.read().await;
                !state.is_archived(&dynamic_id) && !state.is_pending(&dynamic_id)
            };
            if should_add {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Processing dynamically added: {}",
                        dynamic_id
                    ))))
                    .await;
                shared_state
                    .write()
                    .await
                    .add_dynamic_change(dynamic_id.clone());
            }
        }

        let removed_ids = dynamic_queue.drain_removed().await;
        if !removed_ids.is_empty() {
            let mut removed_pending = Vec::new();
            {
                let mut state = shared_state.write().await;
                for id in removed_ids {
                    if state.drop_pending_change(&id) {
                        removed_pending.push(id);
                    }
                }
            }

            for id in removed_pending {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Removed from pending queue: {}",
                        id
                    ))))
                    .await;
            }
        }

        // Check if all pending changes are done
        if shared_state.read().await.is_complete() {
            break;
        }

        // Note: Phase 1 archive processing has been removed.
        // SerialRunService::process_change() now handles archiving automatically
        // for completed changes. Archive results are handled in Phase 2 below.

        // Check for cancellation
        if cancel_token.is_cancelled() {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing cancelled".to_string(),
                )))
                .await;
            break;
        }

        // Phase 2: Select and apply next change (including completed ones for archiving)
        // Fetch current state to find best candidate using native implementation
        let changes = openspec::list_changes_native()?;

        // Filter to changes in pending set (include completed changes so they can be archived)
        let eligible_changes: Vec<_> = {
            let state = shared_state.read().await;
            changes
                .iter()
                .filter(|c| state.is_pending(&c.id))
                .cloned()
                .collect()
        };

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
            shared_state.write().await.drop_pending_change(&change_id);
            let change_stopped_event = OrchestratorEvent::ChangeDequeued {
                change_id: change_id.clone(),
            };
            dispatcher.dispatch(change_stopped_event).await;
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
                    "Change stopped: {}",
                    change_id
                ))))
                .await;
            continue;
        }

        // Notify processing started
        let processing_started_event = OrchestratorEvent::ProcessingStarted(change_id.clone());
        dispatcher.dispatch(processing_started_event).await;

        let remaining_changes = shared_state.read().await.remaining_changes();

        // Get current apply count for this change (before processing)
        let apply_count_before = shared_state.read().await.apply_count(&change_id);

        // Create output handler that forwards through the dispatch owner.
        // Use Arc<RwLock<String>> to track current operation (apply/acceptance/archive/resolve)
        let tx_clone = event_bridge.clone();
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
                    OutputMessage::AgentStderr(s) => {
                        let _ = tx
                            .send(OrchestratorEvent::Log(
                                LogEntry::info(s)
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
        let apply_task_format_context = crate::agent::build_task_format_repair_context(
            &crate::execution::apply::pending_task_format_repair(
                std::path::Path::new("."),
                &change_id,
            ),
        );
        let apply_full_prompt = crate::agent::build_apply_prompt_with_skill(
            agent.config().get_apply_skill(),
            &change_id,
            apply_user_prompt,
            &apply_history_context,
            &acceptance_tail,
            &apply_task_format_context,
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
        dispatcher.dispatch(apply_started_event).await;

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

        let total_changes = shared_state.read().await.total_changes();
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
        dispatcher.dispatch(apply_output_event).await;

        match result {
            Ok(ChangeProcessResult::Cancelled) => {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::warn(
                        "Processing cancelled".to_string(),
                    )))
                    .await;
                shared_state.write().await.clear_pending_changes();
                break;
            }
            Ok(ChangeProcessResult::ChangeStopped) => {
                // Clear the stopped flag to allow re-queueing
                dynamic_queue.clear_stopped(&change_id).await;
                // Send ChangeStopped event to move the change to not queued
                let change_stopped_event = OrchestratorEvent::ChangeDequeued {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(change_stopped_event).await;

                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
                        "Change {} stopped, continuing with other queued changes",
                        change_id
                    ))))
                    .await;
                // Remove this change from pending but continue processing others
                shared_state.write().await.remove_from_pending(&change_id);
                continue;
            }
            Ok(ChangeProcessResult::AcceptancePassed) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Send AcceptanceStarted event
                let acceptance_started_event = OrchestratorEvent::AcceptanceStarted {
                    change_id: change_id.clone(),
                    command: format!("opencode acceptance {}", change_id),
                };
                dispatcher.dispatch(acceptance_started_event).await;

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;

                // Send ProcessingCompleted event
                let processing_completed_event =
                    OrchestratorEvent::ProcessingCompleted(change_id.clone());
                dispatcher.dispatch(processing_completed_event).await;
            }
            Ok(ChangeProcessResult::ApplySuccessIncomplete) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;
            }
            Ok(ChangeProcessResult::AcceptanceContinue) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;
            }
            Ok(ChangeProcessResult::AcceptanceContinueExceeded) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;
            }
            Ok(ChangeProcessResult::Rejected { reason }) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;

                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Acceptance gated - rejection flow completed: {}",
                        reason
                    ))))
                    .await;

                dispatcher
                    .dispatch(OrchestratorEvent::ChangeRejected {
                        change_id: change_id.clone(),
                        reason,
                    })
                    .await;
            }
            Ok(ChangeProcessResult::AcceptanceFailed { .. }) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;
            }
            Ok(ChangeProcessResult::AcceptanceCommandFailed { error }) => {
                // Send ApplyCompleted event
                let apply_completed_event = OrchestratorEvent::ApplyCompleted {
                    change_id: change_id.clone(),
                    revision: String::new(),
                };
                dispatcher.dispatch(apply_completed_event).await;

                // Note: AcceptanceStarted event is sent from acceptance_test_streaming
                // with the actual command string (including diff context and last output)

                // Send AcceptanceCompleted event
                let acceptance_completed_event = OrchestratorEvent::AcceptanceCompleted {
                    change_id: change_id.clone(),
                };
                dispatcher.dispatch(acceptance_completed_event).await;

                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::error(format!(
                        "Acceptance command failed: {}",
                        error
                    ))))
                    .await;
            }
            Ok(ChangeProcessResult::ApplyFailed { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                dispatcher.dispatch(processing_error_event).await;
            }
            Ok(ChangeProcessResult::Archived) => {
                // Change was complete and successfully archived
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::success(format!(
                        "Change {} archived successfully",
                        change_id
                    ))))
                    .await;

                // Send ChangeArchived event
                let change_archived_event = OrchestratorEvent::ChangeArchived(change_id.clone());
                dispatcher.dispatch(change_archived_event).await;
                let post_archive_event = {
                    let state = shared_state.read().await;
                    post_archive_dispatch_event(&state, &change_id)
                };
                if let Some(post_event) = post_archive_event {
                    dispatcher.dispatch(post_event).await;
                }
            }
            // A validated acceptance stall carries structured blocker evidence,
            // so it is published as the same `AcceptanceGated` lifecycle event
            // parallel mode emits and displays as `stalled` rather than as an
            // opaque processing error.
            Ok(ref stalled @ ChangeProcessResult::AcceptanceStalled { ref error, .. }) => {
                for event in stalled.stalled_lifecycle_events(&change_id) {
                    dispatcher.dispatch(event).await;
                }
                tracing::warn!(
                    change_id = %change_id,
                    "Acceptance stalled on a validated external blocker: {error}"
                );

                // Remove stalled change from pending
                shared_state.write().await.remove_from_pending(&change_id);
            }
            Ok(ChangeProcessResult::Stalled { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                dispatcher.dispatch(processing_error_event).await;

                // Remove stalled change from pending
                shared_state.write().await.remove_from_pending(&change_id);
            }
            Ok(ChangeProcessResult::Failed { error }) => {
                let processing_error_event = OrchestratorEvent::ProcessingError {
                    id: change_id.clone(),
                    error: error.clone(),
                };
                dispatcher.dispatch(processing_error_event).await;
            }
            Err(e) => {
                // Check if this was a single-change stop (error message contains "Cancelled")
                let error_str = e.to_string();
                if error_str.contains("Cancelled") && dynamic_queue.try_is_stopped(&change_id) {
                    // Clear the stop flag and send ChangeStopped event
                    dynamic_queue.clear_stopped(&change_id).await;
                    shared_state.write().await.drop_pending_change(&change_id);
                    let change_stopped_event2 = OrchestratorEvent::ChangeDequeued {
                        change_id: change_id.clone(),
                    };
                    dispatcher.dispatch(change_stopped_event2).await;
                    dispatcher
                        .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
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
                    dispatcher.dispatch(processing_error_event).await;
                    break;
                }
            }
        }
    }

    // Run on_finish hook after all changes processed or stopped
    let state = shared_state.read().await;
    let complete_context =
        HookContext::new(state.changes_processed(), state.total_changes(), 0, false);
    if let Err(e) = hooks.run_hook(HookType::OnFinish, &complete_context).await {
        dispatcher
            .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                "on_finish hook failed: {}",
                e
            ))))
            .await;
    }

    // Flush the bridged producers before the terminal event so a hook or output
    // line cannot be ordered after completion. Bounded: a producer that somehow
    // outlives the run must not be able to hold the boundary open.
    drop(hooks);
    drop(event_bridge);
    let _ = tokio::time::timeout(EVENT_BRIDGE_DRAIN_TIMEOUT, event_bridge_handle).await;

    // Send completion event
    dispatcher.dispatch(OrchestratorEvent::AllCompleted).await;

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
    explicit_retry: bool,
    repo_root: PathBuf,
    config: OrchestratorConfig,
    tx: mpsc::Sender<OrchestratorEvent>,
    cancel_token: CancellationToken,
    dynamic_queue: DynamicQueue,
    _graceful_stop_flag: Arc<AtomicBool>,
    shared_state: Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
) -> Result<()> {
    use crate::openspec::list_changes_native_from;
    use crate::parallel::ParallelEvent;
    use crate::parallel_run_service::ParallelRunService;

    // The same dispatch owner serial mode uses. Parallel used to reach the TUI,
    // the reducer, and the web state through three hand-written paths with
    // different membership per event; routing the scheduler's event stream and
    // the boundary's own events through one owner is what makes both modes
    // deliver the same events to the same frontends.
    let dispatcher = Arc::new(EventDispatcher::new(
        shared_state.clone(),
        boundary_event_sinks(
            &tx,
            #[cfg(feature = "web-monitoring")]
            web_state.as_ref(),
        ),
    ));

    dispatcher
        .dispatch(OrchestratorEvent::Log(LogEntry::info(format!(
            "Starting parallel processing of {} change(s)",
            change_ids.len()
        ))))
        .await;

    // Create ParallelRunService and bind it to the caller-owned reducer so empty
    // manual resolve startup observes the same ResolveWait/RejectWait intent that
    // accepted the TUI command.
    let mut service = ParallelRunService::new(repo_root.clone(), config.clone());
    configure_parallel_post_archive_action(&mut service, post_archive_action);
    // The local TUI implements no Git operation of its own: it hands the same
    // validated runtime to the same shared parallel service `cflx run` uses, so
    // both frontends drive one change-scoped publication implementation.
    configure_parallel_upstream_integration(&mut service, upstream_runtime);
    service.set_shared_orchestrator_state(shared_state.clone());

    // Check if Git is available for parallel execution
    service.check_vcs_available().await?;

    initialize_parallel_shared_state(&shared_state, &change_ids, config.get_max_iterations()).await;

    // Create shared queue change timestamp for debouncing
    let shared_queue_change = Arc::new(tokio::sync::Mutex::new(None::<std::time::Instant>));

    // Fetch all changes for UI refresh
    let all_changes = list_changes_native_from(&repo_root)?;

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
    dispatcher
        .dispatch(OrchestratorEvent::ChangesRefreshed {
            changes: all_changes,
            rejected_changes: crate::openspec::list_rejected_changes_native_from(&repo_root)
                .unwrap_or_default(),
            committed_change_ids,
            uncommitted_file_change_ids,
            worktree_change_ids: HashSet::new(),
            worktree_paths: HashMap::new(),
            worktree_not_ahead_ids: HashSet::new(),
            merge_wait_ids: HashSet::new(),
        })
        .await;

    // Create event channel for forwarding to TUI
    let (parallel_tx, mut parallel_rx) = mpsc::channel::<ParallelEvent>(100);

    // Spawn event forwarding task.
    //
    // The forwarder deliberately does not break on the global cancellation token:
    // the scheduler keeps emitting cleanup events (and its own `Stopped`) while it
    // drains after cancellation, and a forwarder that quit early would both hide
    // those events and let a full channel block the very cleanup the outer
    // boundary is waiting for. It ends when the scheduler future is dropped and
    // closes the channel, or on a terminal event.
    let merge_deferred_stop = Arc::new(AtomicBool::new(false));
    let forward_merge_stop = merge_deferred_stop.clone();
    let forward_dispatcher = dispatcher.clone();
    let forward_handle = tokio::spawn(async move {
        while let Some(event) = parallel_rx.recv().await {
            match event {
                ParallelEvent::AllCompleted => {
                    // Execution is over, but whether it *completed* is the outer
                    // boundary's call: it decides between completion and a stop
                    // and publishes exactly one terminal event for both
                    // frontends. Publishing one here too would give the remote
                    // projection a completion the TUI never saw.
                    break;
                }
                ParallelEvent::Stopped => {
                    forward_merge_stop.store(true, Ordering::SeqCst);
                    forward_dispatcher.dispatch(ParallelEvent::Stopped).await;
                    break;
                }
                parallel_event => {
                    forward_dispatcher.dispatch(parallel_event).await;
                }
            }
        }
    });

    // Execute all changes using slot-driven continuous dispatch.
    //
    // The scheduler future is owned here so operator cancellation can keep
    // polling it instead of dropping it: the scheduler itself performs inner task
    // abort, join draining, execution-handle release, pending merge/base-lane
    // result handling, and workspace-guard drop.
    let mut scheduler = Box::pin(service.run_parallel_with_channel_and_queue_state(
        changes_to_process.clone(),
        parallel_tx,
        Some(cancel_token.clone()),
        Some(shared_queue_change.clone()),
        Some(Arc::new(dynamic_queue.clone())),
        Some(manual_resolve_counter.clone()),
        Some(shared_state.clone()),
        explicit_retry,
    ));

    let (termination, result) = tokio::select! {
        biased;
        result = &mut scheduler => (ParallelTermination::SchedulerReturned, Some(result)),
        _ = cancel_token.cancelled() => {
            let change_ids: Vec<String> = changes_to_process.iter().map(|c| c.id.clone()).collect();
            dispatcher.dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "Cancelled parallel execution ({} changes: {})",
                    change_ids.len(),
                    change_ids.join(", ")
                )))).await;
            drain_cancelled_scheduler(&mut scheduler, PARALLEL_CANCELLATION_CLEANUP_DEADLINE).await
        }
    };

    if termination == ParallelTermination::CancelledAfterCleanupTimeout {
        // Escalate to managed cleanup by dropping the scheduler, which aborts the
        // remaining work and releases its resources. This stays an operator
        // cancellation and is never reported as an execution failure.
        dispatcher
            .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                "Stop cleanup exceeded {}s; escalating to managed cleanup",
                PARALLEL_CANCELLATION_CLEANUP_DEADLINE.as_secs()
            ))))
            .await;
    }

    // Drop the scheduler before joining the forwarder: dropping it closes the
    // event channel, which is how the forwarder observes the end of the run.
    drop(scheduler);

    // Wait for forward task to complete
    let _ = forward_handle.await;

    let scheduler_reported_stop = merge_deferred_stop.load(Ordering::SeqCst);
    let scheduler_failed = matches!(result, Some(Err(_)));
    let scheduler_completed_with_errors = matches!(
        &result,
        Some(Ok(report)) if report.has_change_failures()
    );

    let has_reducer_owned_lane_wait_or_active = {
        let state = shared_state.read().await;
        !state.resolve_wait_change_ids().is_empty()
            || !state.reject_wait_change_ids().is_empty()
            || state.is_base_mutating_lane_occupied()
    };

    let report = classify_parallel_terminal_report(
        termination,
        scheduler_failed,
        scheduler_reported_stop,
        has_reducer_owned_lane_wait_or_active,
        scheduler_completed_with_errors,
    );

    match report {
        ParallelTerminalReport::Stopped => {
            if termination.is_operator_cancellation() {
                // Operator cancellation owns exactly one terminal stop transition.
                // The frontend may already have applied `Stopped`; that handler is
                // idempotent, so a late delivery reconciles state without adding a
                // duplicate terminal message.
                dispatcher.dispatch(OrchestratorEvent::Stopped).await;
            } else if scheduler_reported_stop {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Execution stopped with deferred merges ({} changes processed)",
                        changes_to_process.len()
                    ))))
                    .await;
            } else {
                dispatcher.dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                        "Execution paused with reducer-owned lane retry work still pending or active ({} changes processed)",
                        changes_to_process.len()
                    )))).await;
            }
        }
        ParallelTerminalReport::Failed => {
            if let Some(Err(e)) = &result {
                dispatcher
                    .dispatch(OrchestratorEvent::Log(LogEntry::error(format!(
                        "Execution failed: {}",
                        e
                    ))))
                    .await;
            }
        }
        // Deliberately no success log: changes are still waiting for an
        // explicit retry, so claiming completion here would be untruthful.
        ParallelTerminalReport::CompletedWithErrors => {}
        ParallelTerminalReport::Completed => {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::success(format!(
                    "Execution completed ({} changes processed)",
                    changes_to_process.len()
                ))))
                .await;
        }
    }

    // Only send completion message and AllCompleted event if not stopped/cancelled
    match report {
        ParallelTerminalReport::Stopped => {}
        ParallelTerminalReport::Failed | ParallelTerminalReport::CompletedWithErrors => {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::warn(
                    "Processing completed with errors".to_string(),
                )))
                .await;
            dispatcher.dispatch(OrchestratorEvent::AllCompleted).await;
        }
        ParallelTerminalReport::Completed => {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::success(
                    "All parallel changes completed".to_string(),
                )))
                .await;
            dispatcher.dispatch(OrchestratorEvent::AllCompleted).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn parallel_service_uses_tui_post_archive_action() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut service = crate::parallel_run_service::ParallelRunService::new(
            temp_dir.path().to_path_buf(),
            crate::config::OrchestratorConfig::default(),
        );

        super::configure_parallel_post_archive_action(
            &mut service,
            crate::parallel::PostArchiveAction::PushToRemote {
                remote: "origin".to_string(),
            },
        );

        assert_eq!(
            service.post_archive_action(),
            &crate::parallel::PostArchiveAction::PushToRemote {
                remote: "origin".to_string()
            }
        );
    }

    fn upstream_test_service(
        temp: &tempfile::TempDir,
    ) -> crate::parallel_run_service::ParallelRunService {
        crate::parallel_run_service::ParallelRunService::new(
            temp.path().to_path_buf(),
            crate::config::OrchestratorConfig::default(),
        )
    }

    fn upstream_test_runtime() -> crate::upstream::UpstreamRuntime {
        crate::upstream::UpstreamRuntime {
            config: crate::upstream::UpstreamIntegrationConfig::new("origin", "cargo test"),
            branch: "main".to_string(),
        }
    }

    #[test]
    fn per_change_upstream_absent_configuration_installs_nothing_in_tui() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut service = upstream_test_service(&temp_dir);

        super::configure_parallel_upstream_integration(&mut service, None);

        assert!(
            service.upstream_integration().is_none(),
            "a TUI without -u must install no upstream coordinator"
        );
    }

    #[test]
    fn per_change_upstream_local_tui_and_run_install_equivalent_runtime() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        // The `run` frontend installs the runtime directly on the shared service.
        let mut run_service = upstream_test_service(&temp_dir);
        run_service.set_upstream_integration(upstream_test_runtime());

        // The TUI frontend routes through its own wiring helper.
        let mut tui_service = upstream_test_service(&temp_dir);
        super::configure_parallel_upstream_integration(
            &mut tui_service,
            Some(upstream_test_runtime()),
        );

        assert_eq!(
            tui_service.upstream_integration(),
            run_service.upstream_integration(),
            "local TUI and run must construct one identical publication runtime"
        );
    }

    #[test]
    fn per_change_upstream_persistent_tui_reconstruction_retains_configuration() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let runtime = Some(upstream_test_runtime());

        // A persistent TUI builds a fresh service for every orchestrator start;
        // the invocation-scoped runtime must survive each reconstruction.
        for _ in 0..3 {
            let mut service = upstream_test_service(&temp_dir);
            super::configure_parallel_upstream_integration(&mut service, runtime.clone());
            assert_eq!(
                service.upstream_integration(),
                Some(&upstream_test_runtime())
            );
        }
    }

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

    #[tokio::test]
    async fn test_parallel_startup_preserves_empty_manual_resolve_wait_state() {
        use crate::orchestration::state::{
            ExecutionMode, OrchestratorState, ReducerCommand, WorkspaceObservation,
        };

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(
            OrchestratorState::with_mode(vec!["alpha".to_string()], 3, ExecutionMode::Serial),
        ));
        {
            let mut state = shared_state.write().await;
            state.apply_observation("alpha", WorkspaceObservation::WorkspaceArchived);
            state.apply_command(ReducerCommand::ResolveMerge("alpha".to_string()));
        }

        let preserved = super::initialize_parallel_shared_state(&shared_state, &[], 7).await;

        let state = shared_state.read().await;
        assert!(
            preserved,
            "empty ResolveWait startup should skip replacement"
        );
        assert_eq!(state.execution_mode(), ExecutionMode::Parallel);
        assert_eq!(state.display_status("alpha"), "resolve pending");
        assert_eq!(state.resolve_wait_change_ids(), vec!["alpha".to_string()]);
    }

    #[tokio::test]
    async fn test_parallel_startup_resets_selected_run_and_drops_stale_resolve_wait() {
        use crate::orchestration::state::{
            ExecutionMode, OrchestratorState, ReducerCommand, WorkspaceObservation,
        };

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(
            OrchestratorState::with_mode(vec!["stale".to_string()], 3, ExecutionMode::Parallel),
        ));
        {
            let mut state = shared_state.write().await;
            state.apply_observation("stale", WorkspaceObservation::WorkspaceArchived);
            state.apply_command(ReducerCommand::ResolveMerge("stale".to_string()));
        }

        let selected = vec!["fresh".to_string()];
        let preserved = super::initialize_parallel_shared_state(&shared_state, &selected, 7).await;

        let state = shared_state.read().await;
        assert!(!preserved, "selected startup must create a fresh run state");
        assert_eq!(state.execution_mode(), ExecutionMode::Parallel);
        assert_eq!(state.display_status("fresh"), "queued");
        assert_eq!(state.display_status("stale"), "not queued");
        assert!(state.resolve_wait_change_ids().is_empty());
        assert!(state.pending_changes().contains("fresh"));
        assert!(!state.pending_changes().contains("stale"));
    }

    #[tokio::test]
    async fn test_parallel_startup_empty_without_resolve_wait_resets_to_noop_state() {
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(
            OrchestratorState::with_mode(vec!["old".to_string()], 3, ExecutionMode::Parallel),
        ));

        let preserved = super::initialize_parallel_shared_state(&shared_state, &[], 7).await;

        let state = shared_state.read().await;
        assert!(
            !preserved,
            "empty startup without ResolveWait remains ordinary no-op"
        );
        assert_eq!(state.execution_mode(), ExecutionMode::Parallel);
        assert!(state.pending_changes().is_empty());
        assert_eq!(state.display_status("old"), "not queued");
        assert!(state.resolve_wait_change_ids().is_empty());
    }

    #[tokio::test]
    async fn test_tui_shared_state_pending_changes_decrease_when_cleared() {
        use crate::orchestration::state::OrchestratorState;

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string(), "change-b".to_string()],
            3,
        )));

        {
            let state = shared_state.read().await;
            assert_eq!(state.pending_changes().len(), 2);
        }

        shared_state.write().await.clear_pending_changes();

        let state = shared_state.read().await;
        assert_eq!(state.pending_changes().len(), 0);
        assert!(state.pending_changes().is_empty());
    }

    #[test]
    fn test_tui_archived_during_resolve() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState, WaitState};

        let mut state = OrchestratorState::with_mode(
            vec!["change-a".to_string(), "change-b".to_string()],
            3,
            ExecutionMode::Parallel,
        );

        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve change-a".to_string(),
            },
        );
        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeArchived("change-b".to_string()),
        );

        let deferred = super::post_archive_dispatch_event(&state, "change-b");
        assert!(matches!(
            deferred,
            Some(ExecutionEvent::MergeDeferred {
                ref change_id,
                auto_resumable: true,
                ..
            }) if change_id == "change-b"
        ));

        if let Some(event) = deferred {
            crate::orchestration::state::OrchestratorState::apply_execution_event(
                &mut state, &event,
            );
        }

        let runtime = state
            .change_runtime("change-b")
            .expect("change-b runtime should exist");
        assert_eq!(runtime.wait_state, WaitState::ResolveWait);
    }

    #[test]
    fn test_tui_archived_no_active_resolve_or_rejecting() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{
            ActivityState, ExecutionMode, OrchestratorState, WaitState,
        };

        let mut state =
            OrchestratorState::with_mode(vec!["change-a".to_string()], 3, ExecutionMode::Parallel);

        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeArchived("change-a".to_string()),
        );

        let deferred = super::post_archive_dispatch_event(&state, "change-a");
        assert!(deferred.is_none());

        let runtime = state
            .change_runtime("change-a")
            .expect("change-a runtime should exist");
        assert_eq!(runtime.wait_state, WaitState::None);
        assert_eq!(runtime.activity, ActivityState::Resolving);
    }

    #[test]
    fn test_tui_archived_during_rejecting_emits_auto_resumable_deferred() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};
        use crate::vcs::WorkspaceStatus;

        let mut state = OrchestratorState::with_mode(
            vec!["change-a".to_string(), "change-b".to_string()],
            3,
            ExecutionMode::Parallel,
        );

        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws-a".to_string(),
                status: WorkspaceStatus::Rejecting,
            },
        );
        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeArchived("change-b".to_string()),
        );

        let deferred = super::post_archive_dispatch_event(&state, "change-b");
        assert!(matches!(
            deferred,
            Some(ExecutionEvent::MergeDeferred {
                ref change_id,
                auto_resumable: true,
                ..
            }) if change_id == "change-b"
        ));
    }

    #[test]
    fn test_tui_archived_during_applying_does_not_emit_auto_resumable_deferred() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};

        let mut state = OrchestratorState::with_mode(
            vec!["change-a".to_string(), "change-b".to_string()],
            3,
            ExecutionMode::Parallel,
        );

        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply change-a".to_string(),
            },
        );
        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeArchived("change-b".to_string()),
        );

        let deferred = super::post_archive_dispatch_event(&state, "change-b");
        assert!(
            deferred.is_none(),
            "applying blocker must not trigger resolve-pending auto dispatch"
        );
    }

    #[test]
    fn test_tui_archived_with_terminal_rejected_change_does_not_emit_auto_resumable_deferred() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{ExecutionMode, OrchestratorState};

        let mut state = OrchestratorState::with_mode(
            vec!["change-a".to_string(), "change-b".to_string()],
            3,
            ExecutionMode::Parallel,
        );

        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeRejected {
                change_id: "change-a".to_string(),
                reason: "blocked".to_string(),
            },
        );
        crate::orchestration::state::OrchestratorState::apply_execution_event(
            &mut state,
            &ExecutionEvent::ChangeArchived("change-b".to_string()),
        );

        let deferred = super::post_archive_dispatch_event(&state, "change-b");
        assert!(
            deferred.is_none(),
            "terminal rejected blocker must not trigger resolve-pending auto dispatch"
        );
    }

    /// Test helper behavior for rejection-like removal from pending in TUI serial mode.
    #[tokio::test]
    async fn test_tui_rejection_removes_from_pending_selection() {
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

    // ---------------------------------------------------------------------
    // Idle parallel stop: cancellation cleanup barrier and terminal reporting
    // ---------------------------------------------------------------------

    use super::{
        classify_parallel_terminal_report, drain_cancelled_scheduler, ParallelTerminalReport,
        ParallelTermination,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    const TEST_CLEANUP_DEADLINE: Duration = Duration::from_secs(120);

    #[tokio::test(start_paused = true)]
    async fn idle_parallel_stop_cancellation_does_not_drop_the_scheduler_future() {
        let cleanup_done = Arc::new(AtomicBool::new(false));
        let scheduler_cleanup = cleanup_done.clone();

        // Stands in for the scheduler's post-cancellation work: abort, join
        // drain, execution-handle release, and workspace-guard drop.
        let scheduler = async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            scheduler_cleanup.store(true, Ordering::SeqCst);
            Ok::<(), String>(())
        };

        let (termination, result) =
            drain_cancelled_scheduler(scheduler, TEST_CLEANUP_DEADLINE).await;

        assert_eq!(termination, ParallelTermination::CancelledAfterCleanup);
        assert!(matches!(result, Some(Ok(()))));
        assert!(
            cleanup_done.load(Ordering::SeqCst),
            "cancellation must keep polling the scheduler so its cleanup completes"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn idle_parallel_stop_pending_merge_results_drain_before_terminal_stop() {
        let handled_merges = Arc::new(AtomicUsize::new(0));
        let scheduler_merges = handled_merges.clone();
        let (merge_tx, mut merge_rx) = tokio::sync::mpsc::channel::<&'static str>(4);

        // Stands in for a scheduler that still owns pending background merge
        // results when cancellation arrives.
        let scheduler = async move {
            while let Some(_merge_result) = merge_rx.recv().await {
                scheduler_merges.fetch_add(1, Ordering::SeqCst);
            }
            Ok::<(), String>(())
        };

        let producer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = merge_tx.send("merged").await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let _ = merge_tx.send("deferred").await;
        });

        let (termination, result) =
            drain_cancelled_scheduler(scheduler, TEST_CLEANUP_DEADLINE).await;
        producer.await.expect("merge producer");

        assert_eq!(termination, ParallelTermination::CancelledAfterCleanup);
        assert!(matches!(result, Some(Ok(()))));
        assert_eq!(
            handled_merges.load(Ordering::SeqCst),
            2,
            "pending base-lane results must be handled before terminal stop"
        );
        assert_eq!(
            classify_parallel_terminal_report(termination, false, true, false, false),
            ParallelTerminalReport::Stopped,
            "a drained pending merge is never a force-stopped agent process failure"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn idle_parallel_stop_cleanup_deadline_escalates_without_execution_failure() {
        let scheduler = std::future::pending::<Result<(), String>>();

        let (termination, result) =
            drain_cancelled_scheduler(scheduler, TEST_CLEANUP_DEADLINE).await;

        assert_eq!(
            termination,
            ParallelTermination::CancelledAfterCleanupTimeout
        );
        assert!(result.is_none());
        assert!(termination.is_operator_cancellation());
        assert_eq!(
            classify_parallel_terminal_report(termination, false, false, false, false),
            ParallelTerminalReport::Stopped,
            "a bounded cleanup escalation stays operator cancellation"
        );
    }

    #[test]
    fn idle_parallel_stop_operator_cancellation_is_not_an_execution_failure() {
        for termination in [
            ParallelTermination::CancelledAfterCleanup,
            ParallelTermination::CancelledAfterCleanupTimeout,
        ] {
            assert_eq!(
                classify_parallel_terminal_report(termination, true, false, false, false),
                ParallelTerminalReport::Stopped,
                "cancellation must never be reported as an agent-command failure"
            );
        }
    }

    #[test]
    fn idle_parallel_stop_genuine_failure_keeps_execution_error_reporting() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                true,
                false,
                false,
                false
            ),
            ParallelTerminalReport::Failed
        );
    }

    #[test]
    fn idle_parallel_stop_normal_completion_still_reports_completion() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                false,
                false,
                false,
                false
            ),
            ParallelTerminalReport::Completed
        );
    }

    #[test]
    fn idle_parallel_stop_scheduler_reported_stop_suppresses_completion() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                false,
                true,
                false,
                false
            ),
            ParallelTerminalReport::Stopped
        );
    }

    #[test]
    fn idle_parallel_stop_reducer_lane_wait_pauses_instead_of_completing() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                false,
                false,
                true,
                false
            ),
            ParallelTerminalReport::Stopped
        );
    }

    #[test]
    fn finite_change_local_failure_completes_with_errors_not_success() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                false,
                false,
                false,
                true
            ),
            ParallelTerminalReport::CompletedWithErrors,
            "a drained run holding a change in merge wait is neither success nor failure"
        );
    }

    #[test]
    fn run_fatal_scheduler_failure_outranks_change_local_failures() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::SchedulerReturned,
                true,
                false,
                false,
                true
            ),
            ParallelTerminalReport::Failed,
            "an aborted run stays Failed; change-local suppression must not downgrade it"
        );
    }

    #[test]
    fn operator_cancellation_outranks_change_local_failures() {
        assert_eq!(
            classify_parallel_terminal_report(
                ParallelTermination::CancelledAfterCleanup,
                false,
                false,
                false,
                true
            ),
            ParallelTerminalReport::Stopped,
            "operator cancellation owns the terminal transition"
        );
    }

    #[test]
    fn scheduler_run_report_only_flags_completed_with_errors() {
        use crate::parallel::SchedulerRunReport;

        assert!(SchedulerRunReport::CompletedWithErrors.has_change_failures());
        assert!(!SchedulerRunReport::Completed.has_change_failures());
        assert!(!SchedulerRunReport::Stopped.has_change_failures());
    }
}
