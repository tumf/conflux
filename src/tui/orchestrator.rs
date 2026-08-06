//! Orchestrator execution logic for the TUI
//!
//! Contains the run_orchestrator function and archive operations.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::events::{EventDispatcher, EventSink};
use crate::openspec::Change;
use crate::parallel::PostArchiveAction;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
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
pub(crate) const PARALLEL_CANCELLATION_CLEANUP_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(120);

/// Run `on_finish` exactly once for a TUI parallel run.
///
/// The workspace task records the shared Apply-budget owner's refusal as a typed
/// observation on the reducer, so this boundary reports `iteration_limit` with
/// that change's exact cumulative dispatch count — the same contract `cflx run`
/// reports through `Orchestrator::run_parallel_finish_hook`. Without this call
/// the TUI never delivered the typed outcome to the hook at all.
async fn run_tui_parallel_finish_hook(
    hooks: &crate::hooks::HookRunner,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
) -> Result<()> {
    use crate::hooks::{HookContext, HookType};

    let state = shared_state.read().await;
    let (finish_status, finish_apply_count) = state.parallel_finish_report();
    let iteration_limit = state.apply_iteration_limits().first().cloned();
    let processed = state.changes_processed();
    let total = state.total_changes();
    drop(state);

    if let Some(record) = &iteration_limit {
        tracing::info!(
            change_id = %record.change_id,
            attempts = record.attempts,
            max = record.max,
            "Parallel run stopped on the Apply-dispatch ceiling"
        );
    }

    let finish_context = HookContext::new(processed, total, 0, false)
        .with_status(finish_status)
        .with_apply_count(finish_apply_count);
    hooks.run_hook(HookType::OnFinish, &finish_context).await
}

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
        // executor.has_resolve_wait() to avoid a zero-change no-op.
        true
    } else {
        *state = crate::orchestration::state::OrchestratorState::new(
            change_ids.to_vec(),
            max_iterations,
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

/// Run the cumulative worktree orchestrator
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
    run_command_scope: crate::ai_command_runner::RunCommandScope,
    dynamic_queue: DynamicQueue,
    _graceful_stop_flag: Arc<AtomicBool>,
    shared_state: Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: Arc<std::sync::atomic::AtomicUsize>,
    post_archive_action: PostArchiveAction,
    upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    marks: Option<crate::orchestration::mark_reconciliation::ExecutionMarkReconciler>,
    #[cfg(feature = "web-monitoring")] web_state: Option<Arc<crate::web::WebState>>,
) -> Result<()> {
    use crate::openspec::list_changes_native_from;
    use crate::parallel::ParallelEvent;
    use crate::parallel_run_service::ParallelRunService;

    // One dispatch owner for the whole boundary run. This used to reach the TUI,
    // the reducer, and the web state through three hand-written paths with
    // different membership per event; routing the scheduler's event stream and
    // the boundary's own events through one owner is what makes every frontend
    // receive the same events.
    // The boundary's dispatch owner also owns execution-mark reconciliation: it
    // is the only place that sees the reducer immediately before and after each
    // transition, which is what a mark-revoking *edge* is defined by. Binding the
    // shared reconciler here — not a new store — is what keeps the TUI row, the
    // `/api/v2` snapshot, and Start target resolution one value.
    let dispatcher = Arc::new(
        EventDispatcher::new(
            shared_state.clone(),
            boundary_event_sinks(
                &tx,
                #[cfg(feature = "web-monitoring")]
                web_state.as_ref(),
            ),
        )
        .with_mark_reconciler(marks),
    );

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
    // The run owner created the scope, so a clone survives outside this task
    // and local shutdown can still reach the process identities it holds.
    service.set_run_command_scope(run_command_scope);
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
    // Blocked/stalled remainders join change-local failures here: both leave
    // work the operator still owns, so neither may produce a success message.
    let scheduler_completed_with_errors = matches!(
        &result,
        Some(Ok(report)) if report.is_incomplete()
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

    // Exactly one `on_finish` per TUI parallel boundary run, before any terminal
    // event, so a hook log can never be ordered after completion. A run stopped
    // by the shared Apply-dispatch ceiling reports the typed `iteration_limit`
    // status with its exact cumulative count.
    {
        let (hook_event_bridge, hook_bridge_handle) = dispatcher.bridge(EVENT_BRIDGE_BUFFER);
        let hooks = crate::hooks::HookRunner::with_event_tx(
            config.get_hooks(),
            &repo_root,
            hook_event_bridge,
        );
        if let Err(e) = run_tui_parallel_finish_hook(&hooks, &shared_state).await {
            dispatcher
                .dispatch(OrchestratorEvent::Log(LogEntry::warn(format!(
                    "on_finish hook failed: {}",
                    e
                ))))
                .await;
        }
        drop(hooks);
        let _ = tokio::time::timeout(EVENT_BRIDGE_DRAIN_TIMEOUT, hook_bridge_handle).await;
    }

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

    /// The TUI parallel boundary owns the same typed finish contract `cflx run`
    /// owns: the reducer's Apply-ceiling observation, not an error string, is
    /// what `on_finish` reports.
    mod parallel_finish_hook {
        use crate::hooks::{HookConfig, HookConfigValue, HookRunner, HooksConfig};
        use crate::orchestration::state::OrchestratorState;
        use std::sync::Arc;
        use tempfile::TempDir;

        fn hooks_writing_finish_status(
            log: &std::path::Path,
            repo_root: &std::path::Path,
        ) -> HookRunner {
            HookRunner::new(
                HooksConfig {
                    on_finish: Some(HookConfigValue::Full(HookConfig {
                        command: format!(
                            "sh -c 'echo \"$OPENSPEC_STATUS $OPENSPEC_APPLY_COUNT\" >> {}'",
                            log.display()
                        ),
                        continue_on_failure: false,
                        timeout: 30,
                        git_commit_no_verify: false,
                        max_retries: 0,
                        retry_delay_secs: 0,
                    })),
                    ..Default::default()
                },
                repo_root,
            )
        }

        fn lines(log: &std::path::Path) -> Vec<String> {
            std::fs::read_to_string(log)
                .unwrap_or_default()
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        }

        fn parallel_state(
            change_id: &str,
            max_iterations: u32,
        ) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
            Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
                vec![change_id.to_string()],
                max_iterations,
            )))
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_recorded_iteration_limit_reports_status_and_exact_count_once() {
            let temp_dir = TempDir::new().unwrap();
            let log = temp_dir.path().join("on-finish.log");
            let hooks = hooks_writing_finish_status(&log, temp_dir.path());
            let shared_state = parallel_state("change-a", 7);

            {
                let mut state = shared_state.write().await;
                // The workspace task's typed observation: the ceiling refused
                // dispatch 8 after 7 cumulative dispatches.
                state.record_apply_iteration_limit("change-a", 7, 7);
                // A repeated observation for the same change must not duplicate.
                state.record_apply_iteration_limit("change-a", 7, 7);
            }

            super::super::run_tui_parallel_finish_hook(&hooks, &shared_state)
                .await
                .expect("the finish hook must run");

            assert_eq!(
                lines(&log),
                vec!["iteration_limit 7".to_string()],
                "the TUI parallel boundary runs on_finish exactly once with the typed status and \
                 the exact cumulative count"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_run_without_an_iteration_limit_reports_completed() {
            let temp_dir = TempDir::new().unwrap();
            let log = temp_dir.path().join("on-finish.log");
            let hooks = hooks_writing_finish_status(&log, temp_dir.path());
            let shared_state = parallel_state("change-a", 7);

            super::super::run_tui_parallel_finish_hook(&hooks, &shared_state)
                .await
                .expect("the finish hook must run");

            assert_eq!(lines(&log), vec!["completed 0".to_string()]);
        }

        /// A failing finish hook is a hook failure, not a permanent gate.
        ///
        /// The hook still observed the exact typed record before it ran, the
        /// record survives the failure, and nothing durable is written — so the
        /// gate is still retired by scheduler-task exit alone.
        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn active_iteration_limit_run_boundary_survives_a_failing_finish_hook() {
            use crate::orchestration::operator_command::{
                active_apply_iteration_limit, RunBoundaryLiveness,
            };

            struct Boundary(bool);
            impl RunBoundaryLiveness for Boundary {
                fn boundary_running(&self) -> bool {
                    self.0
                }
            }

            let temp_dir = TempDir::new().unwrap();
            let log = temp_dir.path().join("on-finish.log");
            let hooks = HookRunner::new(
                HooksConfig {
                    on_finish: Some(HookConfigValue::Full(HookConfig {
                        // Record the status it observed, then fail.
                        command: format!(
                            "sh -c 'echo \"$OPENSPEC_STATUS $OPENSPEC_APPLY_COUNT\" >> {}; exit 3'",
                            log.display()
                        ),
                        continue_on_failure: false,
                        timeout: 30,
                        git_commit_no_verify: false,
                        max_retries: 0,
                        retry_delay_secs: 0,
                    })),
                    ..Default::default()
                },
                temp_dir.path(),
            );
            let shared_state = parallel_state("change-a", 7);
            shared_state
                .write()
                .await
                .record_apply_iteration_limit("change-a", 7, 7);

            let result = super::super::run_tui_parallel_finish_hook(&hooks, &shared_state).await;

            assert!(
                result.is_err(),
                "the hook failure is reported, not swallowed"
            );
            assert_eq!(
                lines(&log),
                vec!["iteration_limit 7".to_string()],
                "the failing hook still observed the exact typed record"
            );

            let state = shared_state.read().await;
            assert!(
                state.apply_iteration_limit("change-a").is_some(),
                "a hook failure never clears the record"
            );
            assert!(
                active_apply_iteration_limit(&state, Some(&Boundary(true)), "change-a").is_some(),
                "the gate is active while the owning task is live"
            );
            assert_eq!(
                active_apply_iteration_limit(&state, Some(&Boundary(false)), "change-a"),
                None,
                "and scheduler-task exit retires it regardless of the hook outcome"
            );
        }

        /// Both parallel boundaries read the same reducer observation, so a
        /// frontend can never report a different finish status for one run.
        #[tokio::test]
        async fn tui_and_run_derive_the_same_report_from_one_observation() {
            let mut state = OrchestratorState::new(vec!["change-a".to_string()], 7);
            assert_eq!(state.parallel_finish_report(), ("completed", 0));
            state.record_apply_iteration_limit("change-a", 7, 7);
            assert_eq!(state.parallel_finish_report(), ("iteration_limit", 7));
        }
    }

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
            OrchestratorState, ReducerCommand, WorkspaceObservation,
        };

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["alpha".to_string()],
            3,
        )));
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
        assert_eq!(state.display_status("alpha"), "resolve pending");
        assert_eq!(state.resolve_wait_change_ids(), vec!["alpha".to_string()]);
    }

    #[tokio::test]
    async fn test_parallel_startup_resets_selected_run_and_drops_stale_resolve_wait() {
        use crate::orchestration::state::{
            OrchestratorState, ReducerCommand, WorkspaceObservation,
        };

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["stale".to_string()],
            3,
        )));
        {
            let mut state = shared_state.write().await;
            state.apply_observation("stale", WorkspaceObservation::WorkspaceArchived);
            state.apply_command(ReducerCommand::ResolveMerge("stale".to_string()));
        }

        let selected = vec!["fresh".to_string()];
        let preserved = super::initialize_parallel_shared_state(&shared_state, &selected, 7).await;

        let state = shared_state.read().await;
        assert!(!preserved, "selected startup must create a fresh run state");
        assert_eq!(state.display_status("fresh"), "queued");
        assert_eq!(state.display_status("stale"), "not queued");
        assert!(state.resolve_wait_change_ids().is_empty());
        assert!(state.pending_changes().contains("fresh"));
        assert!(!state.pending_changes().contains("stale"));
    }

    #[tokio::test]
    async fn test_parallel_startup_empty_without_resolve_wait_resets_to_noop_state() {
        use crate::orchestration::state::OrchestratorState;

        let shared_state = std::sync::Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["old".to_string()],
            3,
        )));

        let preserved = super::initialize_parallel_shared_state(&shared_state, &[], 7).await;

        let state = shared_state.read().await;
        assert!(
            !preserved,
            "empty startup without ResolveWait remains ordinary no-op"
        );
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
    fn scheduler_run_report_flags_unfinished_work_only() {
        use crate::parallel::SchedulerRunReport;

        assert!(SchedulerRunReport::CompletedWithErrors.is_incomplete());
        assert!(SchedulerRunReport::BlockedOrStalled.is_incomplete());
        assert!(!SchedulerRunReport::Completed.is_incomplete());
        assert!(!SchedulerRunReport::Stopped.is_incomplete());
    }

    // ------------------------------------------------------------------
    // Explicit-intent boundary at TUI/remote parallel startup
    // ------------------------------------------------------------------

    /// TUI and remote Start both come through this initialisation. Only the
    /// resolved targets may gain queue intent, and the initial all-change
    /// refresh that immediately follows must not widen it.
    #[tokio::test]
    async fn parallel_startup_queues_only_selected_targets_and_refresh_does_not_widen_it() {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::{OrchestratorState, QueueIntent};
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;

        let shared = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["fresh".to_string(), "stale".to_string()],
            1,
        )));

        let preserved = super::initialize_parallel_shared_state(
            &shared,
            std::slice::from_ref(&"fresh".to_string()),
            10,
        )
        .await;
        assert!(
            !preserved,
            "a non-empty target set replaces reducer state instead of preserving resolve startup"
        );

        let change = |id: &str| crate::openspec::Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        };

        shared
            .write()
            .await
            .apply_execution_event(&ExecutionEvent::ChangesRefreshed {
                changes: vec![change("fresh"), change("stale")],
                rejected_changes: Vec::new(),
                committed_change_ids: HashSet::from(["fresh".to_string(), "stale".to_string()]),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::from(["stale".to_string()]),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::new(),
            });

        let guard = shared.read().await;
        assert_eq!(guard.queued_change_ids(), vec!["fresh".to_string()]);
        assert_eq!(
            guard
                .change_runtime("stale")
                .expect("refresh registers the unselected change")
                .queue_intent,
            QueueIntent::NotQueued
        );
        assert!(!guard.is_ordinary_queue_eligible("stale"));
        assert!(guard.merge_wait_change_ids().is_empty());
        assert!(guard.resolve_wait_change_ids().is_empty());
        assert!(guard.reject_wait_change_ids().is_empty());
    }
}
