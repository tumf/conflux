//! TuiCommand handlers for TUI
//!
//! This module contains helper functions to handle TuiCommand processing.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::orchestration::operator_command::{
    HookRunnerQueueHooks, OperatorCommandService, QueueMutation,
};
use crate::orchestration::state::{ReduceOutcome, ReducerCommand};
use crate::parallel::PostArchiveAction;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
use crate::tui::orchestrator::{run_orchestrator, run_orchestrator_parallel};
use crate::tui::queue::DynamicQueue;
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, StopMode};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

#[cfg(test)]
#[derive(Clone, Debug)]
enum DeleteWorktreeTestOutcome {
    Success,
    Failure(String),
}

#[cfg(test)]
static DELETE_WORKTREE_TEST_OUTCOMES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, DeleteWorktreeTestOutcome>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
fn set_delete_worktree_test_outcome(path: PathBuf, outcome: DeleteWorktreeTestOutcome) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .insert(path, outcome);
}

use super::worktrees::load_worktrees_with_conflict_check;
use crate::worktree_ops::service::{
    ConflictPolicy, DeleteOptions, WorktreeBackend, WorktreeEventSink, WorktreeOperationEvent,
    WorktreeService,
};

/// Publish shared worktree operation events onto the TUI's orchestrator channel.
///
/// This is the whole reason the TUI and `/api/v2` cannot drift on merge
/// reporting: both frontends emit from the same points in the same service, and
/// only the projection into their own event vocabulary differs.
struct TuiWorktreeEvents {
    tx: mpsc::Sender<OrchestratorEvent>,
    repo_root: std::path::PathBuf,
}

#[async_trait::async_trait]
impl WorktreeEventSink for TuiWorktreeEvents {
    async fn emit(&self, event: WorktreeOperationEvent) {
        let projected = match event {
            WorktreeOperationEvent::MergeStarted { branch } => {
                OrchestratorEvent::BranchMergeStarted {
                    branch_name: branch,
                }
            }
            WorktreeOperationEvent::MergeCompleted { branch } => {
                OrchestratorEvent::BranchMergeCompleted {
                    branch_name: branch,
                }
            }
            WorktreeOperationEvent::MergeFailed { branch, error } => {
                OrchestratorEvent::BranchMergeFailed {
                    branch_name: branch,
                    error,
                }
            }
            WorktreeOperationEvent::Refreshed => {
                match load_worktrees_with_conflict_check(&self.repo_root).await {
                    Ok(worktrees) => OrchestratorEvent::WorktreesRefreshed { worktrees },
                    Err(error) => OrchestratorEvent::Log(LogEntry::error(format!(
                        "Failed to refresh worktrees: {}",
                        error
                    ))),
                }
            }
            // Creation and removal are reported by the handler's own log lines.
            WorktreeOperationEvent::Created { .. } | WorktreeOperationEvent::Deleted { .. } => {
                return
            }
        };
        let _ = self.tx.send(projected).await;
    }
}

/// Backend the TUI drives the shared service with.
///
/// Swapped for a stub under `cfg(test)` for the same reason the previous direct
/// removal helper was: these handlers are unit-tested, and a unit test must not
/// reach a real repository.
#[cfg(not(test))]
fn build_worktree_backend(
    repo_root: &Path,
    config: &OrchestratorConfig,
    tx: &mpsc::Sender<OrchestratorEvent>,
) -> Arc<dyn WorktreeBackend> {
    Arc::new(
        crate::worktree_ops::git_backend::GitWorktreeBackend::new(
            repo_root.to_path_buf(),
            Arc::new(config.clone()),
        )
        .with_hook_events(tx.clone()),
    )
}

#[cfg(test)]
fn build_worktree_backend(
    _repo_root: &Path,
    _config: &OrchestratorConfig,
    _tx: &mpsc::Sender<OrchestratorEvent>,
) -> Arc<dyn WorktreeBackend> {
    Arc::new(tests::StubWorktreeBackend)
}

/// Build the shared worktree operation service for the current TUI context.
fn build_worktree_service(
    repo_root: &Path,
    config: &OrchestratorConfig,
    tx: &mpsc::Sender<OrchestratorEvent>,
) -> WorktreeService {
    let workspace_base_dir = config
        .get_workspace_base_dir()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::config::defaults::default_workspace_base_dir(Some(repo_root)));
    WorktreeService::new(
        build_worktree_backend(repo_root, config, tx),
        Arc::new(TuiWorktreeEvents {
            tx: tx.clone(),
            repo_root: repo_root.to_path_buf(),
        }),
        workspace_base_dir,
    )
}

/// Context for TuiCommand handling
pub struct TuiCommandContext<'a> {
    pub app: &'a mut AppState,
    pub repo_root: &'a Path,
    pub config: &'a OrchestratorConfig,
    pub tx: &'a mpsc::Sender<OrchestratorEvent>,
    pub dynamic_queue: &'a DynamicQueue,
    pub remote_client: Option<crate::remote::RemoteClient>,
    pub post_archive_action: PostArchiveAction,
    /// Invocation-scoped upstream publication runtime for local parallel TUI.
    ///
    /// `None` is the default-off boundary: the parallel service installs no
    /// coordinator, so no fetch, verification, push, or confirmation happens and
    /// cumulative base integration keeps its existing terminal `merged` meaning.
    pub upstream_runtime: Option<crate::upstream::UpstreamRuntime>,
    pub orchestrator_running: bool,
    #[cfg(feature = "web-monitoring")]
    pub web_state: &'a Option<Arc<crate::web::WebState>>,
}

/// Build the shared operator command service for the current TUI context.
///
/// The TUI is an adapter: lifecycle validation, queue mutation, hook cardinality,
/// cancellation ordering, and retry routing all live in the shared service so a
/// remote frontend gets identical behavior.
fn build_operator_service(
    ctx: &TuiCommandContext<'_>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
) -> OperatorCommandService {
    let hook_runner = crate::hooks::HookRunner::with_event_tx(
        ctx.config.get_hooks(),
        ctx.repo_root.to_path_buf(),
        ctx.tx.clone(),
    );
    OperatorCommandService::new(
        shared_state.clone(),
        Arc::new(ctx.dynamic_queue.clone()),
        Arc::new(HookRunnerQueueHooks::new(hook_runner)),
        ctx.app.execution_marks(),
    )
}

/// Handle TuiCommand::StartProcessing
pub async fn handle_start_processing_command(
    ids: Vec<String>,
    explicit_retry: bool,
    ctx: &mut TuiCommandContext<'_>,
    graceful_stop_flag: &Arc<std::sync::atomic::AtomicBool>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: &Arc<AtomicUsize>,
    orchestrator_cancel: &mut Option<CancellationToken>,
) -> Option<tokio::task::JoinHandle<Result<()>>> {
    // Handle web control Start command (empty ids vec) or regular Start
    let mut explicit_retry = explicit_retry || (ids.is_empty() && ctx.app.mode == AppMode::Error);
    let cmd = if ids.is_empty() {
        // Web control start - determine which command based on app mode
        if ctx.app.mode == AppMode::Error {
            ctx.app.retry_error_changes()
        } else if ctx.app.mode == AppMode::Stopped {
            ctx.app.resume_processing()
        } else {
            ctx.app.start_processing()
        }
    } else {
        // Regular start with specific IDs (from F5 key)
        Some(TuiCommand::StartProcessing(ids.clone()))
    };

    // A retry accepted by the shared retry routing (F5 in Error mode, or the web
    // control retry) must reach the executor as an explicit retry so a reconciled
    // acceptance hold resumes acceptance instead of rerunning apply.
    explicit_retry |= ctx.app.take_pending_explicit_retry();

    if let Some(TuiCommand::StartProcessing(selected_ids)) = cmd {
        if !selected_ids.is_empty() {
            // Remote server mode: trigger server-side run instead of local orchestrator.
            if let Some(remote) = ctx.remote_client.as_ref() {
                // Group selected changes by project_id (encoded as "<project_id>::...").
                let mut by_project: std::collections::BTreeMap<String, Vec<String>> =
                    std::collections::BTreeMap::new();
                for id in &selected_ids {
                    let Some((project_id, rest)) = id.split_once("::") else {
                        // Unknown format; skip.
                        continue;
                    };
                    // rest looks like "<project_name>/<change_id>".
                    let change_id = rest.rsplit('/').next().unwrap_or(rest).to_string();
                    by_project
                        .entry(project_id.to_string())
                        .or_default()
                        .push(change_id);
                }

                for (project_id, change_ids) in by_project {
                    if let Err(e) = remote.control_run(&project_id, Some(change_ids)).await {
                        ctx.app.add_log(LogEntry::error(format!(
                            "Remote run failed for {}: {}",
                            project_id, e
                        )));
                    } else {
                        ctx.app.add_log(LogEntry::success(format!(
                            "Remote run started: {}",
                            project_id
                        )));
                    }
                }

                // No local orchestrator task.
                return None;
            }

            // An opted-in session's terminal success is remote confirmation, and
            // the serial dispatch branch below carries no upstream runtime — work
            // dispatched there would finalize as terminal `merged` and publish
            // nothing. Startup already rejects `-u` with serial effective mode,
            // but the runtime `=` toggle is allowed in Select/Stopped mode and
            // would otherwise walk around that validation.
            if ctx.upstream_runtime.is_some() && !ctx.app.parallel_mode {
                let message = "Serial mode cannot run while -u/--integrate-upstream is active: \
                    upstream publication is defined on the cumulative parallel base. \
                    Press '=' to restore parallel mode."
                    .to_string();
                ctx.app.warning_message = Some(message.clone());
                ctx.app.add_log(LogEntry::error(message));
                return None;
            }

            graceful_stop_flag.store(false, Ordering::SeqCst);
            let orch_tx = ctx.tx.clone();
            let orch_config = ctx.config.clone();
            let orch_cancel = CancellationToken::new();
            let orch_dynamic_queue = ctx.dynamic_queue.clone();
            let orch_graceful_stop = graceful_stop_flag.clone();
            let orch_shared_state = shared_state.clone();
            let orch_manual_resolve = manual_resolve_counter.clone();
            let post_archive_action = ctx.post_archive_action.clone();
            let upstream_runtime = ctx.upstream_runtime.clone();
            // The TUI already resolved its repository root; the orchestrator
            // uses that instead of re-deriving the process working directory.
            let orch_repo_root = ctx.repo_root.to_path_buf();
            *orchestrator_cancel = Some(orch_cancel.clone());
            let use_parallel = ctx.app.parallel_mode;
            #[cfg(feature = "web-monitoring")]
            let orch_web_state = ctx.web_state.clone();

            return Some(tokio::spawn(async move {
                #[cfg(feature = "web-monitoring")]
                let result = if use_parallel {
                    run_orchestrator_parallel(
                        selected_ids,
                        explicit_retry,
                        orch_repo_root.clone(),
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve.clone(),
                        post_archive_action.clone(),
                        upstream_runtime.clone(),
                        orch_web_state,
                    )
                    .await
                } else {
                    run_orchestrator(
                        selected_ids,
                        explicit_retry,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_web_state,
                    )
                    .await
                };
                #[cfg(not(feature = "web-monitoring"))]
                let result = if use_parallel {
                    run_orchestrator_parallel(
                        selected_ids,
                        explicit_retry,
                        orch_repo_root.clone(),
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                        post_archive_action,
                        upstream_runtime,
                    )
                    .await
                } else {
                    run_orchestrator(
                        selected_ids,
                        explicit_retry,
                        orch_config,
                        orch_tx.clone(),
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                    )
                    .await
                };
                // NOTE: Do not send Stopped here unconditionally.
                // The orchestrator already sends AllCompleted on normal completion
                // or Stopped when explicitly stopped via graceful_stop_flag.
                result
            }));
        }
    }
    None
}

/// Handle TuiCommand - main dispatcher
///
/// Returns Some(JoinHandle) if a new orchestrator task was spawned
#[allow(clippy::too_many_arguments)]
pub async fn handle_tui_command(
    cmd: TuiCommand,
    ctx: &mut TuiCommandContext<'_>,
    graceful_stop_flag: &Arc<std::sync::atomic::AtomicBool>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    manual_resolve_counter: &Arc<AtomicUsize>,
    orchestrator_cancel: &mut Option<CancellationToken>,
) -> Result<Option<tokio::task::JoinHandle<Result<()>>>> {
    match cmd {
        TuiCommand::StartProcessing(ids) => {
            let handle = handle_start_processing_command(
                ids,
                false,
                ctx,
                graceful_stop_flag,
                shared_state,
                manual_resolve_counter,
                orchestrator_cancel,
            )
            .await;
            return Ok(handle);
        }
        TuiCommand::AddToQueue(id) => {
            // Adapter only: the shared service owns reducer ordering, dynamic
            // queue mutation, and on_queue_add cardinality.
            let service = build_operator_service(ctx, shared_state);
            let outcome = match service.add_to_queue(&id).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    ctx.app
                        .add_log(LogEntry::warn(format!("Queue add rejected: {}", error)));
                    return Ok(None);
                }
            };
            if !outcome.reducer_changed {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Queue add ignored by reducer: {}",
                    id
                )));
                return Ok(None);
            }
            ctx.app.apply_display_statuses_from_reducer(
                &shared_state.read().await.all_display_statuses(),
            );
            if outcome.dynamic_queue_mutated {
                ctx.app
                    .add_log(LogEntry::info(format!("Added to dynamic queue: {}", id)));
            } else {
                ctx.app
                    .add_log(LogEntry::warn(format!("Already in dynamic queue: {}", id)));
            }
        }
        TuiCommand::RemoveFromQueue(id) => {
            // Adapter only: the shared service owns reducer ordering, dynamic
            // queue mutation, and on_queue_remove cardinality.
            let service = build_operator_service(ctx, shared_state);
            let outcome = match service.remove_from_queue(&id).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    ctx.app
                        .add_log(LogEntry::warn(format!("Queue remove rejected: {}", error)));
                    return Ok(None);
                }
            };
            ctx.app.apply_display_statuses_from_reducer(
                &shared_state.read().await.all_display_statuses(),
            );
            let suffix = if outcome.dynamic_queue_mutated {
                " (dynamic queue updated)"
            } else {
                ""
            };
            ctx.app.add_log(LogEntry::info(format!(
                "Removed from queue: {}{}",
                id, suffix
            )));
            debug_assert_eq!(outcome.mutation, QueueMutation::Removed);
        }
        TuiCommand::DeleteWorktreeByPath(path, _branch_name, skip_teardown) => {
            // Adapter only: the shared service owns the delete guards, mandatory
            // teardown ordering, branch cleanup, and the refresh event. The TUI
            // keeps its local recovery `skip_teardown` escape hatch and its
            // documented fail-open behavior for an unobservable dirty state;
            // `/api/v2` passes the fail-closed policy instead.
            let service = build_worktree_service(ctx.repo_root, ctx.config, ctx.tx);
            let delete_result = service
                .delete_worktree(&path, DeleteOptions::local(skip_teardown))
                .await;
            ctx.app.clear_worktree_deleting(&path);

            match delete_result {
                Ok(_) => {
                    info!("Worktree deleted successfully: {}", path.display());
                    ctx.app.add_log(LogEntry::success(format!(
                        "Deleted worktree: {}",
                        path.display()
                    )));
                }
                Err(e) => {
                    ctx.app.show_warning_popup(
                        "Worktree delete failed",
                        format!("Failed to delete worktree '{}': {}", path.display(), e),
                    );
                    ctx.app.add_log(LogEntry::error(format!(
                        "Worktree delete failed for '{}': {}",
                        path.display(),
                        e
                    )));
                }
            }
        }
        TuiCommand::Stop => {
            // Initiate graceful stop
            if ctx.app.mode == AppMode::Running {
                ctx.app.stop_mode = StopMode::GracefulPending;
                graceful_stop_flag.store(true, Ordering::SeqCst);
                ctx.app.mode = AppMode::Stopping;
                ctx.app
                    .add_log(LogEntry::warn("Stopping after current change completes..."));
                // Emit Stopping event for web clients
                ctx.app
                    .handle_orchestrator_event(OrchestratorEvent::Stopping);
                // Forward to web state immediately for web control API
                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = ctx.web_state {
                    web_state
                        .apply_execution_event(&OrchestratorEvent::Stopping)
                        .await;
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot stop: not running (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::CancelStop => {
            // Cancel graceful stop and return to Running mode
            if ctx.app.mode == AppMode::Stopping {
                graceful_stop_flag.store(false, Ordering::SeqCst);
                ctx.app.stop_mode = StopMode::None;
                ctx.app.mode = AppMode::Running;
                ctx.app
                    .add_log(LogEntry::info("Stop canceled, continuing..."));
                // Forward to web state immediately for web control API
                #[cfg(feature = "web-monitoring")]
                if let Some(ref web_state) = ctx.web_state {
                    // Use ProcessingStarted with empty string to transition to running mode
                    web_state
                        .apply_execution_event(&OrchestratorEvent::ProcessingStarted(
                            "".to_string(),
                        ))
                        .await;
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot cancel stop: not stopping (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::ForceStop => {
            // Immediate stop. `AppMode::Stopping` describes TUI lifecycle only, so
            // the force-vs-ordinary decision comes from one runtime activity
            // snapshot shared with the Esc key path.
            if matches!(ctx.app.mode, AppMode::Running | AppMode::Stopping) {
                let snapshot = crate::tui::stop_classification::collect_stop_activity_snapshot(
                    ctx.dynamic_queue,
                    shared_state,
                )
                .await;
                let classification = snapshot.classify();

                // One cancellation mechanism for both reporting classes:
                // classification controls reporting and waiting, never whether
                // managed cleanup runs.
                if let Some(cancel) = orchestrator_cancel {
                    cancel.cancel();
                }

                if classification.process_report.is_force_stop() {
                    ctx.app.stop_mode = StopMode::ForceStopped;
                    ctx.app.add_log(LogEntry::warn("Force stopped"));
                }

                // A live parallel scheduler with in-flight execution or pending
                // background merge/base-lane work owns the terminal stop: it must
                // reach its cancellation-safe boundary first. Everything else has
                // nothing left to drain, so terminal stop applies immediately.
                let scheduler_owns_terminal_stop = ctx.orchestrator_running
                    && snapshot.scheduler_owns_cleanup()
                    && classification.shutdown_barrier.is_required();

                if scheduler_owns_terminal_stop {
                    ctx.app.add_log(LogEntry::info(
                        "Waiting for in-flight work to reach a safe stop boundary...",
                    ));
                } else {
                    ctx.app
                        .handle_orchestrator_event(OrchestratorEvent::Stopped);
                    ctx.app.current_change = None;

                    // Forward stopped event to web state
                    #[cfg(feature = "web-monitoring")]
                    if let Some(ref web_state) = ctx.web_state {
                        use crate::events::ExecutionEvent;
                        web_state
                            .apply_execution_event(&ExecutionEvent::Stopped)
                            .await;
                    }
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot force stop: not running or stopping (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::Retry => {
            // Retry error changes (same as F5 in error mode)
            if ctx.app.mode == AppMode::Error {
                if let Some(TuiCommand::StartProcessing(ids)) = ctx.app.retry_error_changes() {
                    // Handle StartProcessing directly to avoid recursion
                    let handle = handle_start_processing_command(
                        ids,
                        true,
                        ctx,
                        graceful_stop_flag,
                        shared_state,
                        manual_resolve_counter,
                        orchestrator_cancel,
                    )
                    .await;
                    return Ok(handle);
                }
            } else {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Cannot retry: not in error mode (current mode: {:?})",
                    ctx.app.mode
                )));
            }
        }
        TuiCommand::MergeWorktreeBranch {
            worktree_path,
            branch_name,
        } => {
            debug!(
                "Processing TuiCommand::MergeWorktreeBranch: worktree_path={}, branch_name={}",
                worktree_path.display(),
                branch_name
            );

            // Adapter only: the shared service owns the merge guards, the base
            // merge itself, `on_merged` cardinality, and the merge event
            // sequence. The TUI's policy difference is one value — a conflicting
            // merge is aborted here, and preserved for `/api/v2` clients that
            // have no way to resolve it remotely.
            let service = build_worktree_service(ctx.repo_root, ctx.config, ctx.tx);
            let merge_tx = ctx.tx.clone();

            tokio::spawn(async move {
                if let Err(error) = service
                    .merge_worktree(&worktree_path, ConflictPolicy::AbortOnConflict)
                    .await
                {
                    // Failures that reached a known branch already published
                    // `BranchMergeFailed`; this line is what reports the ones
                    // that did not (a busy root, an unobservable worktree).
                    debug!("Merge refused for branch {}: {}", branch_name, error);
                    let _ = merge_tx
                        .send(OrchestratorEvent::Log(LogEntry::error(format!(
                            "Merge failed for '{}': {}",
                            branch_name, error
                        ))))
                        .await;
                }
            });
        }
        TuiCommand::DequeueChange(id) => {
            // Cancellation-first: the shared service issues cancellation, waits for
            // confirmed task termination, and only then applies DequeueChange.
            // The wait is bounded but can take seconds, so it runs off the TUI
            // input loop and reports its outcome through the event channel.
            let service = build_operator_service(ctx, shared_state);
            let result_tx = ctx.tx.clone();
            ctx.app.add_log(LogEntry::info(format!(
                "Stop-and-dequeue request received for: {}",
                id
            )));
            tokio::spawn(async move {
                let entry = match service.stop_and_dequeue(&id).await {
                    Ok(crate::orchestration::operator_command::OperatorOutcome::Dequeued {
                        change_id,
                    }) => LogEntry::success(format!(
                        "Stopped and dequeued after confirmed termination: {}",
                        change_id
                    )),
                    Ok(_) => LogEntry::warn(format!("Stop-and-dequeue ignored for: {}", id)),
                    Err(error) => {
                        warn!("Stop-and-dequeue failed for {}: {}", id, error);
                        LogEntry::error(format!("Stop-and-dequeue failed: {}", error))
                    }
                };
                let _ = result_tx.send(OrchestratorEvent::Log(entry)).await;
            });
        }
        TuiCommand::ResolveMerge(id) => {
            // Apply reducer command first so shared state reflects scheduler-visible resolve intent.
            // A NoOp means reducer-owned retry intent was not accepted, so do not notify or
            // start the scheduler as if display-only pending were real work.
            let reduce_outcome = shared_state
                .write()
                .await
                .apply_command(ReducerCommand::ResolveMerge(id.clone()));
            if matches!(reduce_outcome, ReduceOutcome::NoOp) {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Manual merge-wait retry intent for '{}' was not accepted by scheduler state",
                    id
                )));
                return Ok(None);
            }

            if ctx.orchestrator_running {
                // Scheduler-owned execution model when scheduler is already alive:
                // wake scheduler so normal loop can pick up retry intent and queued work together.
                ctx.dynamic_queue.notify_scheduler();
                ctx.app.add_log(LogEntry::info(format!(
                    "Scheduled merge-wait retry intent for '{}'; notified existing scheduler",
                    id
                )));
            } else {
                // Scheduler not alive: spawn a scheduler-owned run to consume reducer-owned ResolveWait.
                graceful_stop_flag.store(false, Ordering::SeqCst);
                let orch_tx = ctx.tx.clone();
                let orch_config = ctx.config.clone();
                let orch_cancel = CancellationToken::new();
                let orch_dynamic_queue = ctx.dynamic_queue.clone();
                let orch_graceful_stop = graceful_stop_flag.clone();
                let orch_shared_state = shared_state.clone();
                let orch_manual_resolve = manual_resolve_counter.clone();
                let post_archive_action = ctx.post_archive_action.clone();
                let upstream_runtime = ctx.upstream_runtime.clone();
                let orch_repo_root = ctx.repo_root.to_path_buf();
                *orchestrator_cancel = Some(orch_cancel.clone());

                #[cfg(feature = "web-monitoring")]
                let orch_web_state = ctx.web_state.clone();

                let handle = tokio::spawn(async move {
                    #[cfg(feature = "web-monitoring")]
                    let result = run_orchestrator_parallel(
                        Vec::new(),
                        false,
                        orch_repo_root.clone(),
                        orch_config,
                        orch_tx,
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                        post_archive_action.clone(),
                        upstream_runtime.clone(),
                        orch_web_state,
                    )
                    .await;

                    #[cfg(not(feature = "web-monitoring"))]
                    let result = run_orchestrator_parallel(
                        Vec::new(),
                        false,
                        orch_repo_root.clone(),
                        orch_config,
                        orch_tx,
                        orch_cancel,
                        orch_dynamic_queue,
                        orch_graceful_stop,
                        orch_shared_state,
                        orch_manual_resolve,
                        post_archive_action,
                        upstream_runtime,
                    )
                    .await;

                    result
                });

                ctx.app.add_log(LogEntry::info(format!(
                    "Scheduled merge-wait retry intent for '{}'; started scheduler for manual resolve",
                    id
                )));
                return Ok(Some(handle));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::types::WorktreeInfo;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use tokio::sync::RwLock;

    use crate::worktree_ops::service::{
        DirtyState, MergeAttempt, WorktreeFacts, WorktreeOpError, WorktreeOpResult,
    };

    /// Backend the TUI command handlers are unit-tested against.
    ///
    /// Every worktree registered through [`set_delete_worktree_test_outcome`] is
    /// observable and deletable; `remove` replays that registered outcome. No
    /// repository, process, or filesystem state is involved.
    pub(super) struct StubWorktreeBackend;

    #[async_trait::async_trait]
    impl WorktreeBackend for StubWorktreeBackend {
        async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
            Ok(DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .keys()
                .map(|path| {
                    let mut facts = WorktreeFacts::new(path.clone(), "feature-a");
                    facts.dirty = DirtyState::Clean;
                    facts
                })
                .collect())
        }

        async fn base_head(&self) -> WorktreeOpResult<String> {
            Ok("stubhead".to_string())
        }

        async fn create(
            &self,
            _path: &Path,
            _branch: &str,
            _base_commit: &str,
        ) -> WorktreeOpResult<()> {
            Ok(())
        }

        async fn remove(&self, path: &Path, _skip_teardown: bool) -> WorktreeOpResult<()> {
            let outcome = DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .remove(path)
                .unwrap_or(DeleteWorktreeTestOutcome::Success);
            match outcome {
                DeleteWorktreeTestOutcome::Success => Ok(()),
                DeleteWorktreeTestOutcome::Failure(message) => Err(WorktreeOpError::Internal(
                    format!("stubbed delete failure for {}: {}", path.display(), message),
                )),
            }
        }

        async fn delete_branch(&self, _branch: &str) -> WorktreeOpResult<()> {
            Ok(())
        }

        async fn merge_into_base(
            &self,
            _branch: &str,
            _policy: ConflictPolicy,
        ) -> WorktreeOpResult<MergeAttempt> {
            Ok(MergeAttempt::Merged)
        }

        async fn run_on_merged(
            &self,
            _change_id: &str,
            _worktree_path: &Path,
        ) -> WorktreeOpResult<()> {
            Ok(())
        }

        async fn change_is_eligible(&self, _change_id: &str) -> WorktreeOpResult<()> {
            Ok(())
        }
    }

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    fn create_test_config() -> OrchestratorConfig {
        OrchestratorConfig::default()
    }

    fn create_test_worktree(path: &str) -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from(path),
            head: "abc123".to_string(),
            branch: "feature-a".to_string(),
            is_detached: false,
            is_main: false,
            merge_conflict: None,
            has_commits_ahead: true,
            is_merging: false,
        }
    }

    fn create_command_context<'a>(
        app: &'a mut AppState,
        tx: &'a mpsc::Sender<OrchestratorEvent>,
        dynamic_queue: &'a DynamicQueue,
        config: &'a OrchestratorConfig,
    ) -> TuiCommandContext<'a> {
        TuiCommandContext {
            app,
            repo_root: Path::new("."),
            config,
            tx,
            dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        }
    }

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_success() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(vec![], 10)));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let path = PathBuf::from("/tmp/worktree-success");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        let mut app = AppState::new(vec![]);

        app.worktrees = vec![create_test_worktree("/tmp/worktree-success")];
        app.mark_worktree_deleting(path.clone());

        let mut ctx = create_command_context(&mut app, &tx, &dynamic_queue, &config);
        let handle = handle_tui_command(
            TuiCommand::DeleteWorktreeByPath(path.clone(), None, false),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("delete command should succeed");

        assert!(handle.is_none());
        assert!(!ctx.app.is_worktree_deleting(&path));
        assert!(ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("Deleted worktree: /tmp/worktree-success")));
    }

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_failure() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(vec![], 10)));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let path = PathBuf::from("/tmp/worktree-failure");
        set_delete_worktree_test_outcome(
            path.clone(),
            DeleteWorktreeTestOutcome::Failure("boom".to_string()),
        );
        let mut app = AppState::new(vec![]);
        app.worktrees = vec![create_test_worktree("/tmp/worktree-failure")];
        app.mark_worktree_deleting(path.clone());

        let mut ctx = create_command_context(&mut app, &tx, &dynamic_queue, &config);
        let handle = handle_tui_command(
            TuiCommand::DeleteWorktreeByPath(path.clone(), None, false),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("delete command should handle failures as UI errors");

        assert!(handle.is_none());
        assert!(!ctx.app.is_worktree_deleting(&path));
        assert!(ctx.app.warning_popup.is_some());
        assert!(ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Worktree delete failed")));
    }

    #[tokio::test]
    async fn test_resolve_merge_starts_parallel_scheduler_when_idle() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["change-a".to_string()],
            10,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        {
            let mut guard = shared_state.write().await;
            guard.apply_observation(
                "change-a",
                crate::orchestration::state::WorkspaceObservation::WorkspaceArchived,
            );
        }
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should succeed");

        assert!(
            handle.is_some(),
            "idle scheduler must spawn a new orchestrator task"
        );
        assert!(
            orchestrator_cancel.is_some(),
            "spawned scheduler must install cancellation token"
        );
        assert!(
            ctx.app.logs.iter().any(|entry| entry
                .message
                .contains("started scheduler for manual resolve")),
            "log must report scheduler startup"
        );
        {
            let state = shared_state.read().await;
            assert_eq!(
                state.display_status("change-a"),
                "resolve pending",
                "ResolveMerge reducer intent must move change to resolve pending"
            );
            assert_eq!(
                state.resolve_wait_change_ids(),
                vec!["change-a".to_string()],
                "idle handoff must leave reducer-owned ResolveWait visible to scheduler startup"
            );
        }

        if let Some(join) = handle {
            join.abort();
        }
    }

    #[tokio::test]
    async fn test_resolve_merge_notifies_live_scheduler_without_duplicate_spawn() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should succeed");

        assert!(
            handle.is_none(),
            "live scheduler path must not spawn a duplicate orchestrator task"
        );
        assert!(
            orchestrator_cancel.is_none(),
            "live scheduler notification path must not replace cancel token"
        );
        assert!(
            ctx.app
                .logs
                .iter()
                .any(|entry| entry.message.contains("notified existing scheduler")),
            "log must report scheduler notification"
        );
    }

    #[tokio::test]
    async fn test_resolve_merge_logs_scheduler_start_or_notify_truthfully() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

        let mut app_idle = AppState::new(vec![create_test_change("change-a")]);
        let mut cancel_idle: Option<CancellationToken> = None;
        let mut idle_ctx = TuiCommandContext {
            app: &mut app_idle,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let idle_handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut idle_ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut cancel_idle,
        )
        .await
        .expect("idle resolve merge should succeed");

        assert!(idle_ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("started scheduler for manual resolve")));

        if let Some(join) = idle_handle {
            join.abort();
        }

        let mut app_live = AppState::new(vec![create_test_change("change-a")]);
        let mut cancel_live: Option<CancellationToken> = None;
        let mut live_ctx = TuiCommandContext {
            app: &mut app_live,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let live_handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut live_ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut cancel_live,
        )
        .await
        .expect("live resolve merge should succeed");

        assert!(live_handle.is_none());
        assert!(live_ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("notified existing scheduler")));
    }

    #[tokio::test]
    async fn test_resolve_merge_noop_does_not_notify_or_log_scheduled() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["change-a".to_string()],
            10,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        {
            let mut guard = shared_state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-a".to_string(),
            });
        }
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("resolve merge command should not fail");

        assert!(handle.is_none());
        assert!(orchestrator_cancel.is_none());
        assert!(ctx.app.logs.iter().any(|entry| entry
            .message
            .contains("was not accepted by scheduler state")));
        assert!(!ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Scheduled merge-wait retry intent")));
        assert!(shared_state
            .read()
            .await
            .resolve_wait_change_ids()
            .is_empty());
    }

    #[tokio::test]
    async fn test_add_to_queue_updates_reducer_intent_even_if_dynamic_queue_already_contains_id() {
        use crate::orchestration::state::ReducerCommand;

        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let mut app = AppState::new(vec![create_test_change("change-a")]);
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;

        // Pre-populate dynamic queue so AddToQueue push path returns false (already present).
        dynamic_queue.push("change-a".to_string()).await;

        {
            // Clear reducer intent first to ensure command handler is the source of re-queue intent.
            let mut guard = shared_state.write().await;
            guard.apply_command(ReducerCommand::RemoveFromQueue("change-a".to_string()));
            assert_eq!(guard.display_status("change-a"), "not queued");
        }

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::AddToQueue("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("add-to-queue command should succeed");

        assert!(
            handle.is_none(),
            "queue command should not spawn orchestrator"
        );
        assert_eq!(
            shared_state.read().await.display_status("change-a"),
            "queued",
            "reducer queue intent must be queued even when dynamic queue push is duplicate"
        );
        assert!(ctx
            .app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Already in dynamic queue: change-a")));
    }

    #[tokio::test]
    async fn remove_from_queue_updates_reducer_snapshot_and_dynamic_queue() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let mut app = AppState::new(vec![create_test_change("change-a")]);

        dynamic_queue.push("change-a".to_string()).await;
        shared_state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        app.apply_display_statuses_from_reducer(&shared_state.read().await.all_display_statuses());
        assert!(app.changes[0].selected);
        assert_eq!(app.changes[0].display_status_cache, "queued");

        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_tui_command(
            TuiCommand::RemoveFromQueue("change-a".to_string()),
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("remove-from-queue command should succeed");

        assert!(handle.is_none());
        assert_eq!(
            shared_state.read().await.display_status("change-a"),
            "not queued"
        );
        assert_eq!(ctx.app.changes[0].display_status_cache, "not queued");
        assert!(!dynamic_queue.contains("change-a").await);
        assert_eq!(
            dynamic_queue.drain_removed().await,
            vec!["change-a".to_string()]
        );
    }

    #[tokio::test]
    async fn test_resolve_merge_scheduler_liveness_none_finished_live() {
        let (tx, _rx) = mpsc::channel(16);
        let dynamic_queue = DynamicQueue::new();
        let config = create_test_config();
        let shared_state = Arc::new(RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            10,
        )));
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));

        // none/finished = idle path => scheduler spawn
        for running in [false, false] {
            let mut app = AppState::new(vec![create_test_change("change-a")]);
            let mut orchestrator_cancel: Option<CancellationToken> = None;
            let mut ctx = TuiCommandContext {
                app: &mut app,
                repo_root: Path::new("."),
                config: &config,
                tx: &tx,
                dynamic_queue: &dynamic_queue,
                remote_client: None,
                post_archive_action: PostArchiveAction::MergeToBase,
                upstream_runtime: None,
                orchestrator_running: running,
                #[cfg(feature = "web-monitoring")]
                web_state: &None,
            };

            let handle = handle_tui_command(
                TuiCommand::ResolveMerge("change-a".to_string()),
                &mut ctx,
                &graceful_stop_flag,
                &shared_state,
                &manual_resolve_counter,
                &mut orchestrator_cancel,
            )
            .await
            .expect("resolve merge should succeed");

            assert!(
                handle.is_some(),
                "non-live scheduler state must spawn scheduler"
            );
            if let Some(join) = handle {
                join.abort();
            }
        }

        // live = notification-only path => no spawn
        let mut app_live = AppState::new(vec![create_test_change("change-a")]);
        let mut orchestrator_cancel_live: Option<CancellationToken> = None;
        let mut ctx_live = TuiCommandContext {
            app: &mut app_live,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle_live = handle_tui_command(
            TuiCommand::ResolveMerge("change-a".to_string()),
            &mut ctx_live,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel_live,
        )
        .await
        .expect("resolve merge should succeed");

        assert!(
            handle_live.is_none(),
            "live scheduler state must not spawn scheduler"
        );
    }

    // ── Opted-in per-change upstream publication ────────────────────────────

    /// Run `git` in `cwd`, returning trimmed stdout on success.
    fn git_in(cwd: &Path, args: &[&str]) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// A repository on `main` with a real local bare remote `origin`, one
    /// archived change, and a publication-required integration that never
    /// reached the remote.
    fn per_change_upstream_unpublished_repo() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let root = dir.path().join("repo");
        let remote = dir.path().join("remote.git");
        std::fs::create_dir_all(&root).ok()?;
        git_in(dir.path(), &["init", "--bare", "-b", "main", "remote.git"])?;
        git_in(&root, &["init", "-b", "main"])?;
        git_in(&root, &["config", "user.email", "test@example.com"])?;
        git_in(&root, &["config", "user.name", "Test User"])?;
        git_in(&root, &["config", "commit.gpgsign", "false"])?;
        std::fs::write(root.join("README.md"), "# base\n").ok()?;
        git_in(&root, &["add", "-A"])?;
        git_in(&root, &["commit", "-m", "Initial commit"])?;
        git_in(&root, &["remote", "add", "origin", remote.to_str()?])?;
        git_in(&root, &["push", "-u", "origin", "main"])?;

        // `alpha` is already accepted, archived, and integrated into cumulative
        // base; only publication is owed.
        let archive = root.join("openspec/changes/archive/alpha");
        std::fs::create_dir_all(&archive).ok()?;
        std::fs::write(archive.join("proposal.md"), "# archived alpha\n").ok()?;
        git_in(&root, &["add", "-A"])?;
        git_in(&root, &["commit", "-m", "Archive: alpha"])?;
        let marker = crate::upstream::publication::format_publication_marker_message(
            "alpha", "origin", "main",
        );
        git_in(&root, &["commit", "--allow-empty", "-m", &marker])?;
        Some((dir, root))
    }

    /// The recoverable-error projection a failed publication leaves behind.
    async fn per_change_upstream_failed_publication_state() -> Arc<RwLock<OrchestratorState>> {
        use crate::events::ExecutionEvent;
        use crate::orchestration::state::ExecutionMode;

        let mut state =
            OrchestratorState::with_mode(vec!["alpha".to_string()], 0, ExecutionMode::Parallel);
        state.apply_execution_event(&ExecutionEvent::ChangeArchived("alpha".to_string()));
        state.apply_execution_event(&ExecutionEvent::PushStarted {
            change_id: "alpha".to_string(),
            remote: "origin".to_string(),
            branch: "main".to_string(),
        });
        state.apply_execution_event(&ExecutionEvent::PushFailed {
            change_id: "alpha".to_string(),
            remote: "origin".to_string(),
            branch: "main".to_string(),
            error: "upstream publication incomplete: verification failed".to_string(),
        });
        assert_eq!(state.display_status("alpha"), "error");
        Arc::new(RwLock::new(state))
    }

    /// Exhausted publication in a persistent local TUI surfaces as Error mode,
    /// and the operator's explicit retry (F5, or the local web control's Start)
    /// must resume *publication* — not rerun apply. This drives the real
    /// command handler so the retry routing, the runtime hand-off, and the
    /// resumption are covered as one path.
    #[tokio::test]
    async fn per_change_upstream_explicit_tui_retry_resumes_publication() {
        // Publication resumption takes the process-global merge lock, so this
        // test must hold the base-lane test mutex (see `crate::parallel`) to
        // avoid observing a lane another base-lane test owns.
        let _serialize = crate::parallel::merge_lock_test_mutex().lock().await;
        let Some((dir, root)) = per_change_upstream_unpublished_repo() else {
            println!("Skipping test: git not available");
            return;
        };
        let base_head = git_in(&root, &["rev-parse", "HEAD"]).expect("base head");

        // Any apply, acceptance, or archive dispatch would leave a sentinel
        // behind; publication resumption must create none of them.
        let dispatched = dir.path().join("dispatched.txt");
        let sentinel =
            |label: &str| format!("sh -c 'echo {label} >> \"{}\"'", dispatched.display());
        let config = OrchestratorConfig {
            apply_command: Some(sentinel("apply")),
            acceptance_command: Some(sentinel("acceptance")),
            archive_command: Some(sentinel("archive")),
            resolve_command: Some(sentinel("resolve")),
            workspace_base_dir: Some(dir.path().join("workspaces").to_string_lossy().to_string()),
            ..OrchestratorConfig::default()
        };

        let shared_state = per_change_upstream_failed_publication_state().await;
        let mut app = AppState::new(vec![create_test_change("alpha")]);
        app.parallel_mode = true;
        app.shared_orchestrator_state = Some(shared_state.clone());
        app.apply_display_statuses_from_reducer(&shared_state.read().await.all_display_statuses());
        assert_eq!(app.changes[0].display_status_cache, "error");
        app.mode = AppMode::Error;
        app.changes[0].selected = true;

        let (tx, mut rx) = mpsc::channel(256);
        let dynamic_queue = DynamicQueue::new();
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: &root,
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: Some(crate::upstream::UpstreamRuntime {
                config: crate::upstream::UpstreamIntegrationConfig::new("origin", "exit 0"),
                branch: "main".to_string(),
            }),
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        // The local web control's Start sends no IDs; Error mode turns it into
        // the same explicit retry F5 produces.
        let handle = handle_start_processing_command(
            Vec::new(),
            false,
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("explicit retry must start a local orchestrator run");

        let confirmed = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            while let Some(event) = rx.recv().await {
                if matches!(
                    &event,
                    crate::events::ExecutionEvent::PushCompleted { change_id, .. }
                        if change_id == "alpha"
                ) {
                    return true;
                }
            }
            false
        })
        .await
        .expect("explicit retry must reach publication rather than hang");
        assert!(
            confirmed,
            "explicit retry must resume publication for the unpublished change"
        );

        // A persistent TUI stays alive after publishing, so the operator ends it.
        orchestrator_cancel
            .expect("retry must own a cancellation token")
            .cancel();
        let _ = handle.await;

        assert_eq!(
            git_in(&root, &["ls-remote", "origin", "refs/heads/main"])
                .expect("ls-remote")
                .split_whitespace()
                .next()
                .expect("remote head"),
            base_head,
            "confirmation is a remote observation of the integrated cumulative HEAD"
        );
        assert!(
            !dispatched.exists(),
            "resumed publication must not redispatch apply or acceptance: {}",
            std::fs::read_to_string(&dispatched).unwrap_or_default()
        );
    }

    /// The `=` toggle is accepted in Select/Stopped mode, so an opted-in session
    /// can reach the serial dispatch branch — which carries no upstream runtime
    /// and would finalize completions as terminal `merged` with nothing
    /// published. Startup validation cannot see that; dispatch must refuse it.
    #[tokio::test]
    async fn per_change_upstream_serial_dispatch_is_refused_while_publication_is_owed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        let dispatched = dir.path().join("dispatched.txt");
        let config = OrchestratorConfig {
            apply_command: Some(format!(
                "sh -c 'echo apply >> \"{}\"'",
                dispatched.display()
            )),
            ..OrchestratorConfig::default()
        };

        let mut app = AppState::new(vec![create_test_change("alpha")]);
        app.parallel_available = true;
        app.parallel_mode = true;
        app.mode = AppMode::Select;
        app.changes[0].selected = true;

        // The operator flips to serial while the session is opted in.
        assert!(
            app.toggle_parallel_mode(),
            "the toggle is reachable in Select mode"
        );
        assert!(!app.parallel_mode);

        let shared_state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["alpha".to_string()],
            0,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        let (tx, _rx) = mpsc::channel(64);
        let dynamic_queue = DynamicQueue::new();
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: &root,
            config: &config,
            tx: &tx,
            dynamic_queue: &dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: Some(crate::upstream::UpstreamRuntime {
                config: crate::upstream::UpstreamIntegrationConfig::new("origin", "exit 0"),
                branch: "main".to_string(),
            }),
            orchestrator_running: false,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        let handle = handle_start_processing_command(
            vec!["alpha".to_string()],
            false,
            &mut ctx,
            &graceful_stop_flag,
            &shared_state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await;

        assert!(
            handle.is_none(),
            "an opted-in session must not dispatch serial work"
        );
        assert!(
            orchestrator_cancel.is_none(),
            "no orchestrator may be started for the refused dispatch"
        );
        assert!(
            !dispatched.exists(),
            "the refused dispatch must run no apply command"
        );
        let warning = app.warning_message.clone().unwrap_or_default();
        assert!(
            warning.contains("-u/--integrate-upstream"),
            "the operator must be told why the dispatch was refused: {warning}"
        );

        // Restoring parallel mode restores the publication contract.
        assert!(app.toggle_parallel_mode());
        assert!(app.parallel_mode);
    }
}

/// Adapter parity coverage: the same operator intent must produce identical
/// reducer transitions, runtime queue state, and cancellation ordering whether it
/// arrives through the TUI adapter or directly through the shared service.
#[cfg(test)]
mod operator_command_parity_tests {
    use super::*;
    use crate::orchestration::operator_command::{
        ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, OperatorOutcome,
    };
    use crate::orchestration::state::OrchestratorState;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use tokio::sync::RwLock;

    fn test_change(id: &str) -> crate::openspec::Change {
        crate::openspec::Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 1,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    fn parallel_state(ids: &[&str]) -> Arc<RwLock<OrchestratorState>> {
        Arc::new(RwLock::new(
            crate::orchestration::state::OrchestratorState::with_mode(
                ids.iter().map(|id| id.to_string()).collect(),
                10,
                crate::orchestration::state::ExecutionMode::Parallel,
            ),
        ))
    }

    fn direct_service(
        state: &Arc<RwLock<OrchestratorState>>,
        queue: &DynamicQueue,
    ) -> OperatorCommandService {
        OperatorCommandService::new(
            state.clone(),
            Arc::new(queue.clone()),
            Arc::new(NoopQueueHooks),
            Arc::new(ExecutionMarkStore::new()),
        )
    }

    /// Run one TuiCommand through the adapter and return the resulting state.
    async fn run_tui_command(
        command: TuiCommand,
        state: &Arc<RwLock<OrchestratorState>>,
        queue: &DynamicQueue,
    ) {
        run_tui_command_with_config(command, state, queue, OrchestratorConfig::default()).await
    }

    async fn run_tui_command_with_config(
        command: TuiCommand,
        state: &Arc<RwLock<OrchestratorState>>,
        queue: &DynamicQueue,
        config: OrchestratorConfig,
    ) {
        let (tx, _rx) = mpsc::channel(64);
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let mut orchestrator_cancel: Option<CancellationToken> = None;
        let mut app = AppState::new(vec![test_change("change-a")]);
        let mut ctx = TuiCommandContext {
            app: &mut app,
            repo_root: Path::new("."),
            config: &config,
            tx: &tx,
            dynamic_queue: queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running: true,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        };

        handle_tui_command(
            command,
            &mut ctx,
            &graceful_stop_flag,
            state,
            &manual_resolve_counter,
            &mut orchestrator_cancel,
        )
        .await
        .expect("tui command should succeed");
    }

    async fn wait_for_status(state: &Arc<RwLock<OrchestratorState>>, id: &str, expected: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if state.read().await.display_status(id) == expected {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "'{}' never reached status '{}' (last: '{}')",
                id,
                expected,
                state.read().await.display_status(id)
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// End-to-end hook wiring: a configured `on_queue_add` really runs, exactly
    /// once per real dynamic mutation, when the operator adds through the TUI.
    #[tokio::test]
    async fn operator_command_tui_add_runs_configured_queue_hook_once() {
        let temp = tempfile::tempdir().expect("temp dir");
        let marker = temp.path().join("queue-hook.log");
        let config = OrchestratorConfig {
            hooks: Some(
                serde_json::from_str(&format!(
                    r#"{{"on_queue_add": "printf 'add:{{change_id}}\n' >> {}"}}"#,
                    marker.display()
                ))
                .expect("hooks config"),
            ),
            ..Default::default()
        };

        let state = parallel_state(&["change-a"]);
        let queue = DynamicQueue::new();
        run_tui_command_with_config(
            TuiCommand::AddToQueue("change-a".to_string()),
            &state,
            &queue,
            config.clone(),
        )
        .await;

        let after_first = std::fs::read_to_string(&marker).unwrap_or_default();
        assert_eq!(
            after_first, "add:change-a\n",
            "a real dynamic queue addition must run on_queue_add exactly once"
        );

        // A duplicate addition is a no-op and must not run the hook again.
        run_tui_command_with_config(
            TuiCommand::AddToQueue("change-a".to_string()),
            &state,
            &queue,
            config,
        )
        .await;

        let after_duplicate = std::fs::read_to_string(&marker).unwrap_or_default();
        assert_eq!(
            after_duplicate, after_first,
            "a duplicate addition must not run on_queue_add again"
        );
    }

    #[tokio::test]
    async fn operator_command_tui_add_matches_direct_service_add() {
        let tui_state = parallel_state(&["change-a"]);
        let tui_queue = DynamicQueue::new();
        run_tui_command(
            TuiCommand::AddToQueue("change-a".to_string()),
            &tui_state,
            &tui_queue,
        )
        .await;

        let service_state = parallel_state(&["change-a"]);
        let service_queue = DynamicQueue::new();
        let outcome = direct_service(&service_state, &service_queue)
            .add_to_queue("change-a")
            .await
            .expect("direct add");

        assert!(outcome.reducer_changed && outcome.dynamic_queue_mutated);
        assert_eq!(
            tui_state.read().await.display_status("change-a"),
            service_state.read().await.display_status("change-a"),
            "TUI and direct service paths must produce the same reducer transition"
        );
        assert_eq!(
            tui_queue.contains("change-a").await,
            service_queue.contains("change-a").await,
            "TUI and direct service paths must produce the same dynamic queue state"
        );
    }

    #[tokio::test]
    async fn operator_command_tui_remove_matches_direct_service_remove() {
        let tui_state = parallel_state(&["change-a"]);
        let tui_queue = DynamicQueue::new();
        run_tui_command(
            TuiCommand::AddToQueue("change-a".to_string()),
            &tui_state,
            &tui_queue,
        )
        .await;
        run_tui_command(
            TuiCommand::RemoveFromQueue("change-a".to_string()),
            &tui_state,
            &tui_queue,
        )
        .await;

        let service_state = parallel_state(&["change-a"]);
        let service_queue = DynamicQueue::new();
        let service = direct_service(&service_state, &service_queue);
        service.add_to_queue("change-a").await.expect("direct add");
        service
            .remove_from_queue("change-a")
            .await
            .expect("direct remove");

        assert_eq!(
            tui_state.read().await.display_status("change-a"),
            service_state.read().await.display_status("change-a")
        );
        assert_eq!(
            tui_queue.contains("change-a").await,
            service_queue.contains("change-a").await
        );
        assert_eq!(
            tui_queue.drain_removed().await,
            service_queue.drain_removed().await,
            "both paths must record the same pending removal"
        );
    }

    #[tokio::test]
    async fn operator_command_tui_dequeue_waits_for_confirmed_termination() {
        let state = parallel_state(&["change-a"]);
        let queue = DynamicQueue::new();
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            });
        }

        let token = CancellationToken::new();
        queue
            .register_kill_token("change-a".to_string(), token.clone())
            .await;

        let worker_queue = queue.clone();
        let worker = tokio::spawn(async move {
            token.cancelled().await;
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            worker_queue.unregister_kill_token("change-a").await;
        });

        run_tui_command(
            TuiCommand::DequeueChange("change-a".to_string()),
            &state,
            &queue,
        )
        .await;

        wait_for_status(&state, "change-a", "not queued").await;
        worker.await.expect("worker task");
    }

    #[tokio::test]
    async fn operator_command_tui_dequeue_preserves_active_state_without_handle() {
        let state = parallel_state(&["change-a"]);
        let queue = DynamicQueue::new();
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            });
        }

        run_tui_command(
            TuiCommand::DequeueChange("change-a".to_string()),
            &state,
            &queue,
        )
        .await;

        // The spawned stop attempt must fail without mutating active state.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            state.read().await.display_status("change-a"),
            "applying",
            "a missing cancellation handle must leave the change active"
        );
    }

    #[tokio::test]
    async fn operator_command_stop_and_dequeue_service_call_matches_tui_result() {
        let service_state = parallel_state(&["change-a"]);
        let service_queue = DynamicQueue::new();
        {
            let mut guard = service_state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            });
        }
        let token = CancellationToken::new();
        service_queue
            .register_kill_token("change-a".to_string(), token.clone())
            .await;
        let worker_queue = service_queue.clone();
        let worker = tokio::spawn(async move {
            token.cancelled().await;
            worker_queue.unregister_kill_token("change-a").await;
        });

        let outcome = direct_service(&service_state, &service_queue)
            .stop_and_dequeue("change-a")
            .await
            .expect("direct stop-and-dequeue");

        assert_eq!(
            outcome,
            OperatorOutcome::Dequeued {
                change_id: "change-a".to_string()
            }
        );
        assert_eq!(
            service_state.read().await.display_status("change-a"),
            "not queued"
        );
        worker.await.expect("worker task");
    }

    #[tokio::test]
    async fn operator_command_retry_requests_an_explicit_retry_run() {
        let mut app = AppState::new(vec![test_change("change-a")]);
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ProcessingError {
                id: "change-a".to_string(),
                error: "boom".to_string(),
            });
        }
        app.shared_orchestrator_state = Some(state.clone());
        app.apply_display_statuses_from_reducer(&state.read().await.all_display_statuses());
        app.mode = AppMode::Error;
        app.changes[0].selected = true;

        let command = app.retry_error_changes();

        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ref ids)) if ids == &vec!["change-a".to_string()]),
            "a marked error row must be retried"
        );
        assert!(
            app.take_pending_explicit_retry(),
            "operator retry must request explicit-retry semantics so a reconciled \
             acceptance hold resumes acceptance instead of rerunning apply"
        );
        assert!(
            !app.take_pending_explicit_retry(),
            "the explicit-retry request must be consumed exactly once"
        );
        assert_eq!(
            state.read().await.display_status("change-a"),
            "queued",
            "retry routing must clear the terminal error through the reducer"
        );
    }

    #[tokio::test]
    async fn operator_command_retry_also_covers_acceptance_stalled_rows() {
        let mut app = AppState::new(vec![test_change("change-a")]);
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: crate::events::StalledBlocker {
                    category: "acceptance_finding".to_string(),
                    phase: "acceptance".to_string(),
                    gate: "acceptance".to_string(),
                    error_summary: "unresolved finding".to_string(),
                    evidence: vec!["tests/acceptance.rs:1".to_string()],
                    next_action: "resolve and retry".to_string(),
                    resumable: true,
                    worktree_preserved: true,
                },
            });
        }
        app.shared_orchestrator_state = Some(state.clone());
        app.apply_display_statuses_from_reducer(&state.read().await.all_display_statuses());
        assert_eq!(app.changes[0].display_status_cache, "stalled");
        app.mode = AppMode::Error;
        app.changes[0].selected = true;

        let command = app.retry_error_changes();

        assert!(
            matches!(command, Some(TuiCommand::StartProcessing(ref ids)) if ids == &vec!["change-a".to_string()]),
            "a marked acceptance-stalled row must be retryable"
        );
        assert!(app.take_pending_explicit_retry());
        assert_eq!(state.read().await.display_status("change-a"), "queued");
    }

    /// Build a stop-path command context bound to a live parallel scheduler.
    fn stop_command_context<'a>(
        app: &'a mut AppState,
        tx: &'a mpsc::Sender<OrchestratorEvent>,
        dynamic_queue: &'a DynamicQueue,
        config: &'a OrchestratorConfig,
        orchestrator_running: bool,
    ) -> TuiCommandContext<'a> {
        TuiCommandContext {
            app,
            repo_root: Path::new("."),
            config,
            tx,
            dynamic_queue,
            remote_client: None,
            post_archive_action: PostArchiveAction::MergeToBase,
            upstream_runtime: None,
            orchestrator_running,
            #[cfg(feature = "web-monitoring")]
            web_state: &None,
        }
    }

    struct StopCommandResult {
        app: AppState,
        cancel: CancellationToken,
    }

    impl StopCommandResult {
        fn log_count(&self, needle: &str) -> usize {
            self.app
                .logs
                .iter()
                .filter(|entry| entry.message.contains(needle))
                .count()
        }
    }

    /// Run `TuiCommand::ForceStop` against a prepared reducer/queue snapshot.
    async fn run_force_stop(
        state: &Arc<RwLock<OrchestratorState>>,
        queue: &DynamicQueue,
        orchestrator_running: bool,
    ) -> StopCommandResult {
        let (tx, _rx) = mpsc::channel(64);
        let config = OrchestratorConfig::default();
        let graceful_stop_flag = Arc::new(AtomicBool::new(false));
        let manual_resolve_counter = Arc::new(AtomicUsize::new(0));
        let cancel = CancellationToken::new();
        let mut orchestrator_cancel = Some(cancel.clone());
        let mut app = AppState::new(vec![test_change("change-a")]);
        app.mode = AppMode::Stopping;
        app.stop_mode = StopMode::ImmediatePending;
        app.changes[0].selected = true;
        app.changes[0].set_display_status_cache("applying");
        app.publish_execution_marks();

        {
            let mut ctx = stop_command_context(&mut app, &tx, queue, &config, orchestrator_running);
            handle_tui_command(
                TuiCommand::ForceStop,
                &mut ctx,
                &graceful_stop_flag,
                state,
                &manual_resolve_counter,
                &mut orchestrator_cancel,
            )
            .await
            .expect("force stop command");
        }

        StopCommandResult { app, cancel }
    }

    #[tokio::test]
    async fn idle_parallel_stop_active_execution_reports_force_stop() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            });
        }
        let queue = DynamicQueue::new();
        queue
            .register_kill_token("change-a".to_string(), CancellationToken::new())
            .await;

        let result = run_force_stop(&state, &queue, true).await;

        assert!(
            result.cancel.is_cancelled(),
            "an active execution must still request managed cancellation"
        );
        assert_eq!(result.log_count("Force stopped"), 1);
        assert_eq!(result.app.stop_mode, StopMode::ForceStopped);
        assert_eq!(
            result.app.mode,
            AppMode::Stopping,
            "the scheduler owns terminal stop while in-flight cleanup is pending"
        );
        assert_eq!(result.log_count("Processing stopped"), 0);
    }

    #[tokio::test]
    async fn idle_parallel_stop_merge_wait_does_not_claim_process_termination() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "manual resolution required".to_string(),
                auto_resumable: false,
            });
        }
        let queue = DynamicQueue::new();

        let result = run_force_stop(&state, &queue, true).await;

        assert!(
            result.cancel.is_cancelled(),
            "an idle scheduler must still be cancelled"
        );
        assert_eq!(
            result.log_count("Force stopped"),
            0,
            "an idle wait must not claim forceful process termination"
        );
        assert_eq!(result.log_count("Processing stopped"), 1);
        assert_eq!(result.app.mode, AppMode::Stopped);
    }

    #[tokio::test]
    async fn idle_parallel_stop_deferred_merge_wait_is_an_ordinary_stop() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "lane occupied".to_string(),
                auto_resumable: true,
            });
        }
        let queue = DynamicQueue::new();

        let result = run_force_stop(&state, &queue, true).await;

        assert_eq!(result.log_count("Force stopped"), 0);
        assert_eq!(result.log_count("Processing stopped"), 1);
        assert_eq!(result.app.mode, AppMode::Stopped);
    }

    #[tokio::test]
    async fn idle_parallel_stop_pending_background_merge_waits_for_safe_boundary() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "merge".to_string(),
            });
        }
        let queue = DynamicQueue::new();

        let result = run_force_stop(&state, &queue, true).await;

        assert!(result.cancel.is_cancelled());
        assert_eq!(
            result.log_count("Force stopped"),
            0,
            "a background merge is shutdown work, not a force-stopped agent process"
        );
        assert_eq!(
            result.app.mode,
            AppMode::Stopping,
            "terminal stop waits for the base-lane operation to reach its boundary"
        );
        assert_eq!(result.log_count("Processing stopped"), 0);
    }

    #[tokio::test]
    async fn idle_parallel_stop_without_live_scheduler_applies_terminal_stop() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "merge".to_string(),
            });
        }
        let queue = DynamicQueue::new();

        let result = run_force_stop(&state, &queue, false).await;

        assert_eq!(
            result.app.mode,
            AppMode::Stopped,
            "with no live scheduler there is nothing left to drain"
        );
        assert_eq!(result.log_count("Processing stopped"), 1);
    }

    #[tokio::test]
    async fn idle_parallel_stop_preserves_execution_marks_and_resets_queue_status() {
        let state = parallel_state(&["change-a"]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "manual resolution required".to_string(),
                auto_resumable: false,
            });
        }
        let queue = DynamicQueue::new();

        let result = run_force_stop(&state, &queue, true).await;

        assert!(
            result.app.changes[0].selected,
            "stopped-state handling must preserve execution marks"
        );
        assert_eq!(
            result.app.execution_marks().marked_ids(),
            vec!["change-a".to_string()]
        );
        assert_eq!(
            result.app.changes[0].display_status_cache, "not queued",
            "transient in-flight presentation must reset on stop"
        );
    }
}
