//! TuiCommand handlers for TUI
//!
//! This module contains helper functions to handle TuiCommand processing.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::orchestration::operator_command::{OperatorOutcome, QueueMutation};
use crate::orchestration::run_control::{
    ResolveReservation, RunControlError, RunControlOutcome, RunControlService, RunNoOpReason,
    SchedulerEffect,
};
#[cfg(test)]
use crate::parallel::PostArchiveAction;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
#[cfg(test)]
use crate::tui::queue::DynamicQueue;
use crate::tui::state::AppState;
use crate::tui::types::{AppMode, StopMode};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// TUI ↔ `/api/v2` parity, verified over one recording runtime.
#[cfg(all(test, feature = "web-monitoring"))]
mod cross_adapter_tests;

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
    /// The single process-local run-lifecycle service shared with `/api/v2`.
    ///
    /// Every start, stop, retry, and resolve in this module goes through it, so a
    /// keypress and a remote command cannot resolve the same intent differently.
    pub run_control: &'a Arc<RunControlService>,
    #[cfg(feature = "web-monitoring")]
    pub web_state: &'a Option<Arc<crate::web::WebState>>,
}

/// Handle TuiCommand::StartProcessing.
///
/// The TUI is an adapter here: it hands the shared service the mode it is in and
/// projects the returned outcome onto its own presentation state. It does not
/// choose targets, apply queue intent, or decide whether the scheduler should be
/// spawned or woken — the service owns all three, so `/api/v2` start reaches the
/// same decision.
///
/// A non-empty `ids` list (the F5 key path) republishes the marked set first, so
/// even an explicit selection is started through the authoritative mark store.
pub async fn handle_start_processing_command(ids: Vec<String>, ctx: &mut TuiCommandContext<'_>) {
    if !ids.is_empty() {
        ctx.app.execution_marks().replace(ids);
    }

    let mode = ctx.app.operator_mode();
    match ctx.run_control.start(mode).await {
        Ok(RunControlOutcome::RunDispatched {
            change_ids,
            scheduler,
            ..
        }) => {
            ctx.app.begin_run(&change_ids);
            let verb = match scheduler {
                SchedulerEffect::Started => "Starting",
                _ => "Queued for the running scheduler:",
            };
            ctx.app.add_log(LogEntry::info(format!(
                "{} processing {} change(s)",
                verb,
                change_ids.len()
            )));
        }
        Ok(RunControlOutcome::NoOp { reason }) => {
            report_run_no_op(ctx.app, &reason);
        }
        Ok(other) => {
            debug!("Start produced an unexpected outcome: {:?}", other);
        }
        Err(error) => {
            report_run_error(ctx.app, &error);
        }
    }
}

/// Surface a refusal from the shared run-control service.
///
/// A refusal is never silent: the operator gets the same actionable detail the
/// v2 command record carries.
fn report_run_error(app: &mut AppState, error: &RunControlError) {
    let message = error.to_string();
    app.warning_message = Some(message.clone());
    app.add_log(LogEntry::warn(message));
}

fn report_run_no_op(app: &mut AppState, reason: &RunNoOpReason) {
    let message = match reason {
        RunNoOpReason::ResolveAlreadyReserved { change_id } => {
            format!("Change '{}' is already queued for resolve", change_id)
        }
        RunNoOpReason::NoRetryableTarget => {
            "No marked change carries retryable evidence".to_string()
        }
    };
    app.warning_message = Some(message.clone());
    app.add_log(LogEntry::warn(message));
}

/// Handle TuiCommand - main dispatcher
pub async fn handle_tui_command(
    cmd: TuiCommand,
    ctx: &mut TuiCommandContext<'_>,
    shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
) -> Result<()> {
    match cmd {
        TuiCommand::StartProcessing(ids) => {
            handle_start_processing_command(ids, ctx).await;
        }
        TuiCommand::AddToQueue(id) => {
            // Adapter only: the shared service owns reducer ordering, dynamic
            // queue mutation, and on_queue_add cardinality.
            let service = ctx.run_control.operator();
            let outcome = match service.add_to_queue(&id).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    ctx.app
                        .add_log(LogEntry::warn(format!("Queue add rejected: {}", error)));
                    return Ok(());
                }
            };
            if !outcome.reducer_changed {
                ctx.app.add_log(LogEntry::warn(format!(
                    "Queue add ignored by reducer: {}",
                    id
                )));
                return Ok(());
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
            let service = ctx.run_control.operator();
            let outcome = match service.remove_from_queue(&id).await {
                Ok(outcome) => outcome,
                Err(error) => {
                    ctx.app
                        .add_log(LogEntry::warn(format!("Queue remove rejected: {}", error)));
                    return Ok(());
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
        TuiCommand::SetParallelMode(enabled) => {
            // Adapter only: the shared service owns the Select/Stopped guard,
            // the availability guard, and the ineligible mark and queue-intent
            // cleanup. The TUI adopts the resulting toggle and reports what the
            // service actually did, so `=` and `/api/v2` cannot diverge.
            let service = ctx.run_control.operator();
            match service.set_parallel_mode(ctx.app.operator_mode(), enabled).await {
                Ok(OperatorOutcome::ParallelMode { enabled, cleared }) => {
                    ctx.app.sync_parallel_mode_from_runtime();
                    ctx.app.apply_display_statuses_from_reducer(
                        &shared_state.read().await.all_display_statuses(),
                    );
                    ctx.app.add_log(LogEntry::info(format!(
                        "Parallel mode {}",
                        if enabled { "enabled" } else { "disabled" }
                    )));
                    if !cleared.is_empty() {
                        let message = format!(
                            "Removed uncommitted changes from queue in parallel mode: {}",
                            cleared.join(", ")
                        );
                        ctx.app.warning_message = Some(message.clone());
                        ctx.app.add_log(LogEntry::warn(message));
                    }
                }
                Ok(OperatorOutcome::NoOp { .. }) => {
                    // Never silent: a keypress that changed nothing has to say so.
                    let message = format!(
                        "Parallel mode already {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                    ctx.app.warning_message = Some(message.clone());
                    ctx.app.add_log(LogEntry::info(message));
                }
                Ok(other) => debug!("Parallel toggle produced an unexpected outcome: {:?}", other),
                Err(error) => {
                    let message = format!("Parallel mode change rejected: {}", error);
                    ctx.app.warning_message = Some(message.clone());
                    ctx.app.add_log(LogEntry::warn(message));
                }
            }
        }
        TuiCommand::Stop => {
            // Adapter only: the shared service owns the mode matrix and the
            // graceful-stop request. The TUI projects the accepted outcome onto
            // its own mode and reports refusals verbatim.
            match ctx.run_control.stop(ctx.app.operator_mode()).await {
                Ok(RunControlOutcome::StopRequested) => {
                    ctx.app.stop_mode = StopMode::GracefulPending;
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
                }
                Ok(other) => debug!("Stop produced an unexpected outcome: {:?}", other),
                Err(error) => report_run_error(ctx.app, &error),
            }
        }
        TuiCommand::CancelStop => {
            match ctx.run_control.cancel_stop(ctx.app.operator_mode()).await {
                Ok(RunControlOutcome::StopCancelled) => {
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
                }
                Ok(other) => debug!("Cancel stop produced an unexpected outcome: {:?}", other),
                Err(error) => report_run_error(ctx.app, &error),
            }
        }
        TuiCommand::ForceStop => {
            // Immediate stop. `AppMode::Stopping` describes TUI lifecycle only, so
            // the force-vs-ordinary decision comes from the one runtime activity
            // snapshot the shared service takes, and cancellation is issued there
            // for both reporting classes.
            match ctx.run_control.force_stop(ctx.app.operator_mode()).await {
                Ok(RunControlOutcome::ForceStopped {
                    classification,
                    awaiting_safe_boundary,
                }) => {
                    if classification.process_report.is_force_stop() {
                        ctx.app.stop_mode = StopMode::ForceStopped;
                        ctx.app.add_log(LogEntry::warn("Force stopped"));
                    }

                    // A live parallel scheduler with in-flight execution or
                    // pending background merge/base-lane work owns the terminal
                    // stop: it must reach its cancellation-safe boundary first.
                    if awaiting_safe_boundary {
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
                }
                Ok(other) => debug!("Force stop produced an unexpected outcome: {:?}", other),
                Err(error) => report_run_error(ctx.app, &error),
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
            let service = ctx.run_control.operator();
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
            // Adapter only: the shared service owns the reducer intent, the
            // single-resolver reservation, FIFO ordering, duplicate rejection,
            // and whether the scheduler is started or merely woken.
            match ctx.run_control.resolve_merge(&id).await {
                Ok(RunControlOutcome::ResolveReserved {
                    change_id,
                    reservation,
                    scheduler,
                }) => {
                    if let Some(change) = ctx.app.changes.iter_mut().find(|c| c.id == change_id) {
                        change.set_display_status_cache("resolve pending");
                    }
                    match reservation {
                        ResolveReservation::Active => {
                            if matches!(ctx.app.mode, AppMode::Select | AppMode::Stopped) {
                                ctx.app.mode = AppMode::Running;
                            }
                            let how = match scheduler {
                                SchedulerEffect::Started => "started scheduler for manual resolve",
                                _ => "notified existing scheduler",
                            };
                            ctx.app.add_log(LogEntry::info(format!(
                                "Scheduled merge-wait retry intent for '{}'; {}",
                                change_id, how
                            )));
                        }
                        ResolveReservation::Queued { position } => {
                            ctx.app.add_log(LogEntry::info(format!(
                                "Queued '{}' for resolve (position: {})",
                                change_id, position
                            )));
                        }
                    }
                }
                Ok(RunControlOutcome::NoOp { reason }) => report_run_no_op(ctx.app, &reason),
                Ok(other) => debug!("Resolve produced an unexpected outcome: {:?}", other),
                Err(RunControlError::TargetIneligible { change_id, .. }) => {
                    // A stale resolve target gets a resolve-specific message, but
                    // it is still surfaced the way every other refusal is: the
                    // operator must not have to read the log panel to learn that
                    // `/api/v2` would have reported `target_ineligible` here.
                    let message = format!(
                        "Manual merge-wait retry intent for '{}' was not accepted by scheduler state",
                        change_id
                    );
                    ctx.app.warning_message = Some(message.clone());
                    ctx.app.add_log(LogEntry::warn(message));
                }
                Err(error) => report_run_error(ctx.app, &error),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::orchestration::operator_command::{
        ExecutionMarkStore, HookRunnerQueueHooks, OperatorCommandService,
    };
    use crate::orchestration::run_control::testing::{RecordingScheduler, SchedulerCall};
    use crate::orchestration::run_control::{ResolveReservations, StartEligibility};
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::types::WorktreeInfo;
    use std::path::{Path, PathBuf};
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

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

    pub(super) fn create_test_change(id: &str) -> Change {
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

    /// Everything a TUI adapter test needs, wired to one recording scheduler.
    ///
    /// The adapter under test is given exactly the production services; only the
    /// scheduler is replaced, so a test still exercises the real lifecycle
    /// matrix, the real reducer, and the real reservation ledger.
    pub(super) struct AdapterHarness {
        pub(super) state: Arc<RwLock<OrchestratorState>>,
        pub(super) queue: DynamicQueue,
        pub(super) scheduler: Arc<RecordingScheduler>,
        pub(super) run_control: Arc<RunControlService>,
        pub(super) marks: Arc<ExecutionMarkStore>,
        /// The one parallel runtime store both adapters read and mutate.
        pub(super) parallel:
            Arc<crate::orchestration::operator_command::ParallelRuntime>,
        pub(super) resolves: Arc<ResolveReservations>,
        pub(super) config: OrchestratorConfig,
        pub(super) tx: mpsc::Sender<OrchestratorEvent>,
        pub(super) rx: mpsc::Receiver<OrchestratorEvent>,
    }

    impl AdapterHarness {
        pub(super) fn new(change_ids: &[&str]) -> Self {
            Self::with_config(change_ids, create_test_config())
        }

        pub(super) fn with_config(change_ids: &[&str], config: OrchestratorConfig) -> Self {
            let state = Arc::new(RwLock::new(OrchestratorState::with_mode(
                change_ids.iter().map(|id| id.to_string()).collect(),
                10,
                crate::orchestration::state::ExecutionMode::Parallel,
            )));
            Self::over(state, DynamicQueue::new(), config)
        }

        pub(super) fn over(
            state: Arc<RwLock<OrchestratorState>>,
            queue: DynamicQueue,
            config: OrchestratorConfig,
        ) -> Self {
            let (tx, rx) = mpsc::channel(256);
            let marks = Arc::new(ExecutionMarkStore::new());
            let parallel = Arc::new(StartEligibility::new());
            parallel.set_available(true);
            let hook_runner = crate::hooks::HookRunner::with_event_tx(
                config.get_hooks(),
                PathBuf::from("."),
                tx.clone(),
            );
            let operator = Arc::new(
                OperatorCommandService::new(
                    state.clone(),
                    Arc::new(queue.clone()),
                    Arc::new(HookRunnerQueueHooks::new(hook_runner)),
                    marks.clone(),
                )
                .with_parallel(parallel.clone()),
            );
            let scheduler = Arc::new(RecordingScheduler::new());
            let resolves = Arc::new(ResolveReservations::new());
            let run_control = Arc::new(RunControlService::new(
                state.clone(),
                operator,
                scheduler.clone(),
                resolves.clone(),
                parallel.clone(),
            ));
            Self {
                state,
                queue,
                scheduler,
                run_control,
                marks,
                parallel,
                resolves,
                config,
                tx,
                rx,
            }
        }

        /// An `AppState` already bound to this harness's shared handles.
        pub(super) fn app(&self, change_ids: &[&str]) -> AppState {
            let mut app = AppState::new(
                change_ids
                    .iter()
                    .map(|id| create_test_change(id))
                    .collect::<Vec<_>>(),
            );
            app.set_shared_state(self.state.clone());
            app.set_resolve_reservations(self.resolves.clone());
            app.set_execution_marks(self.marks.clone());
            app.parallel_available = true;
            app.set_parallel_runtime(self.parallel.clone());
            app.sync_parallel_mode_from_runtime();
            app
        }

        pub(super) fn context<'a>(&'a self, app: &'a mut AppState) -> TuiCommandContext<'a> {
            TuiCommandContext {
                app,
                repo_root: Path::new("."),
                config: &self.config,
                tx: &self.tx,
                run_control: &self.run_control,
                #[cfg(feature = "web-monitoring")]
                web_state: &None,
            }
        }

        /// Run one command through the TUI adapter.
        pub(super) async fn run(&self, app: &mut AppState, command: TuiCommand) {
            let mut ctx = self.context(app);
            handle_tui_command(command, &mut ctx, &self.state)
                .await
                .expect("tui command should succeed");
        }

        pub(super) async fn status(&self, change_id: &str) -> String {
            self.state
                .read()
                .await
                .display_status(change_id)
                .to_string()
        }
    }

    fn log_count(app: &AppState, needle: &str) -> usize {
        app.logs
            .iter()
            .filter(|entry| entry.message.contains(needle))
            .count()
    }

    // ── Worktree deletion ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_success() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-success");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktreeByPath(
                    path.clone(),
                    Some("feature-a".to_string()),
                    false,
                ),
            )
            .await;

        assert!(!app.is_worktree_deleting(&path));
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Deleted worktree")));
    }

    #[tokio::test]
    async fn test_delete_worktree_command_clears_marker_on_failure() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-failure");
        set_delete_worktree_test_outcome(
            path.clone(),
            DeleteWorktreeTestOutcome::Failure("locked".to_string()),
        );
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktreeByPath(
                    path.clone(),
                    Some("feature-a".to_string()),
                    false,
                ),
            )
            .await;

        assert!(
            !app.is_worktree_deleting(&path),
            "a failed delete must still clear the in-progress marker"
        );
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Worktree delete failed")));
    }

    // ── Queue intent ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_to_queue_updates_reducer_intent_even_if_dynamic_queue_already_contains_id() {
        use crate::orchestration::state::ReducerCommand;

        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);

        // Pre-populate dynamic queue so the AddToQueue push path returns false.
        harness.queue.push("change-a".to_string()).await;
        {
            let mut guard = harness.state.write().await;
            guard.apply_command(ReducerCommand::RemoveFromQueue("change-a".to_string()));
            assert_eq!(guard.display_status("change-a"), "not queued");
        }

        harness
            .run(&mut app, TuiCommand::AddToQueue("change-a".to_string()))
            .await;

        assert_eq!(
            harness.status("change-a").await,
            "queued",
            "reducer queue intent must be queued even when dynamic queue push is duplicate"
        );
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Already in dynamic queue: change-a")));
    }

    #[tokio::test]
    async fn remove_from_queue_updates_reducer_snapshot_and_dynamic_queue() {
        use crate::orchestration::state::ReducerCommand;

        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);

        harness.queue.push("change-a".to_string()).await;
        harness
            .state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue("change-a".to_string()));
        app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
        assert!(app.changes[0].selected);
        assert_eq!(app.changes[0].display_status_cache, "queued");

        harness
            .run(
                &mut app,
                TuiCommand::RemoveFromQueue("change-a".to_string()),
            )
            .await;

        assert_eq!(harness.status("change-a").await, "not queued");
        assert_eq!(app.changes[0].display_status_cache, "not queued");
        assert!(!harness.queue.contains("change-a").await);
        assert_eq!(
            harness.queue.drain_removed().await,
            vec!["change-a".to_string()]
        );
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

        let harness = AdapterHarness::with_config(&["change-a"], config);
        let mut app = harness.app(&["change-a"]);

        harness
            .run(&mut app, TuiCommand::AddToQueue("change-a".to_string()))
            .await;

        let after_first = std::fs::read_to_string(&marker).unwrap_or_default();
        assert_eq!(
            after_first, "add:change-a\n",
            "a real dynamic queue addition must run on_queue_add exactly once"
        );

        // A duplicate addition is a no-op and must not run the hook again.
        harness
            .run(&mut app, TuiCommand::AddToQueue("change-a".to_string()))
            .await;

        let after_duplicate = std::fs::read_to_string(&marker).unwrap_or_default();
        assert_eq!(
            after_duplicate, after_first,
            "a duplicate addition must not run on_queue_add again"
        );
    }

    // ── Stop-and-dequeue ────────────────────────────────────────────────────

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

    #[tokio::test]
    async fn operator_command_tui_dequeue_waits_for_confirmed_termination() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
        );

        let token = CancellationToken::new();
        harness
            .queue
            .register_kill_token("change-a".to_string(), token.clone())
            .await;

        let worker_queue = harness.queue.clone();
        let worker = tokio::spawn(async move {
            token.cancelled().await;
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            worker_queue.unregister_kill_token("change-a").await;
        });

        harness
            .run(&mut app, TuiCommand::DequeueChange("change-a".to_string()))
            .await;

        wait_for_status(&harness.state, "change-a", "not queued").await;
        worker.await.expect("worker task");
    }

    #[tokio::test]
    async fn operator_command_tui_dequeue_preserves_active_state_without_handle() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
        );

        harness
            .run(&mut app, TuiCommand::DequeueChange("change-a".to_string()))
            .await;

        // The spawned stop attempt must fail without mutating active state.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            harness.status("change-a").await,
            "applying",
            "a missing cancellation handle must leave the change active"
        );
    }

    // ── Start / retry ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn start_dispatches_the_marked_target_set_and_moves_the_tui_to_running() {
        let harness = AdapterHarness::new(&["change-a", "change-b"]);
        let mut app = harness.app(&["change-a", "change-b"]);
        app.changes[0].selected = true;
        app.changes[1].selected = false;
        app.publish_execution_marks();

        harness
            .run(&mut app, TuiCommand::StartProcessing(Vec::new()))
            .await;

        assert_eq!(
            harness.scheduler.started_targets(),
            vec![vec!["change-a".to_string()]]
        );
        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.changes[0].display_status_cache, "queued");
        assert_eq!(app.changes[1].display_status_cache, "not queued");
    }

    #[tokio::test]
    async fn start_without_an_eligible_target_reports_the_refusal_and_starts_nothing() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.publish_execution_marks();

        harness
            .run(&mut app, TuiCommand::StartProcessing(Vec::new()))
            .await;

        assert!(
            harness.scheduler.calls().is_empty(),
            "an empty target set must not start a scheduler"
        );
        assert_eq!(app.mode, AppMode::Select);
        let warning = app.warning_message.clone().unwrap_or_default();
        assert!(
            warning.contains("no eligible target"),
            "the operator must be told why start was refused: {warning}"
        );
    }

    #[tokio::test]
    async fn an_explicit_start_selection_is_started_through_the_shared_mark_store() {
        let harness = AdapterHarness::new(&["change-a", "change-b"]);
        let mut app = harness.app(&["change-a", "change-b"]);

        harness
            .run(
                &mut app,
                TuiCommand::StartProcessing(vec!["change-b".to_string()]),
            )
            .await;

        assert_eq!(
            harness.marks.marked_ids(),
            vec!["change-b".to_string()],
            "an explicit selection republishes the authoritative mark set"
        );
        assert_eq!(
            harness.scheduler.started_targets(),
            vec![vec!["change-b".to_string()]]
        );
    }

    #[tokio::test]
    async fn tui_retry_routes_error_rows_and_proves_scheduler_dispatch() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::ProcessingError {
                id: "change-a".to_string(),
                error: "boom".to_string(),
            },
        );
        app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
        app.mode = AppMode::Error;
        app.changes[0].selected = true;
        app.publish_execution_marks();

        // Retry is start in Error mode: the same command variant a keypress sends.
        harness
            .run(&mut app, TuiCommand::StartProcessing(Vec::new()))
            .await;

        assert_eq!(
            harness.scheduler.calls(),
            vec![SchedulerCall::Started {
                targets: vec!["change-a".to_string()],
                explicit_retry: true,
            }],
            "operator retry must start the run with explicit-retry semantics"
        );
        assert_eq!(
            harness.status("change-a").await,
            "queued",
            "retry routing must clear the terminal error through the reducer"
        );
    }

    #[tokio::test]
    async fn tui_retry_covers_acceptance_stalled_rows() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::AcceptanceGated {
                change_id: "change-a".to_string(),
                blocker: crate::events::StalledBlocker {
                    category: "acceptance_finding".to_string(),
                    phase: "acceptance".to_string(),
                    gate: "acceptance".to_string(),
                    error_summary: "unresolved finding".to_string(),
                    evidence: vec!["tests/acceptance.rs:1".to_string()],
                    // No unblock condition and an unsupported category: this
                    // stays an execution stall, never an external wait.
                    unblock_condition: None,
                    prerequisite_owner: None,
                    next_action: "resolve and retry".to_string(),
                    resumable: true,
                    worktree_preserved: true,
                },
            },
        );
        app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
        assert_eq!(app.changes[0].display_status_cache, "stalled");
        app.mode = AppMode::Error;
        app.changes[0].selected = true;
        app.publish_execution_marks();

        harness
            .run(&mut app, TuiCommand::StartProcessing(Vec::new()))
            .await;

        assert_eq!(harness.status("change-a").await, "queued");
        assert!(harness.scheduler.calls().contains(&SchedulerCall::Started {
            targets: vec!["change-a".to_string()],
            explicit_retry: true,
        }));
    }

    /// The TUI renders dependency waits, external prerequisite waits, and
    /// execution stalls from one reducer snapshot, and only the external wait is
    /// explicitly retryable.
    #[tokio::test]
    async fn tui_distinguishes_dependency_external_and_stalled_rows_from_the_reducer() {
        let ids = ["dependency-wait", "external-wait", "execution-stall"];
        let harness = AdapterHarness::new(&ids);
        let mut app = harness.app(&ids);
        {
            let mut guard = harness.state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::DependencyBlocked {
                change_id: "dependency-wait".to_string(),
                dependency_ids: vec!["alpha".to_string()],
            });
            guard.apply_execution_event(&crate::events::ExecutionEvent::AcceptanceGated {
                change_id: "external-wait".to_string(),
                blocker: crate::events::StalledBlocker::acceptance_external(
                    "credential",
                    "STAGING_API_KEY is unset",
                ),
            });
            let denial = crate::permission::classify_permission_denial(&[Some(
                "Read permission denied for /private/secret.txt",
            )])
            .expect("denial should classify");
            guard.apply_execution_event(&crate::events::ExecutionEvent::ExecutionBlocked {
                change_id: "execution-stall".to_string(),
                blocker: crate::events::StalledBlocker::permission_denial("acceptance", &denial),
            });
        }

        let (display_map, blocker_views) = {
            let guard = harness.state.read().await;
            (guard.all_display_statuses(), guard.all_blocker_views())
        };
        app.apply_display_statuses_from_reducer(&display_map);
        app.apply_blocker_views_from_reducer(&blocker_views);

        let row = |id: &str| {
            app.changes
                .iter()
                .find(|change| change.id == id)
                .expect("row")
        };

        // Both waits are `blocked`; the badge is what keeps them apart.
        assert_eq!(row("dependency-wait").display_status_cache, "blocked");
        assert_eq!(row("external-wait").display_status_cache, "blocked");
        assert_eq!(row("execution-stall").display_status_cache, "stalled");
        assert_eq!(row("dependency-wait").status_badge(), "blocked:dependency");
        assert_eq!(row("external-wait").status_badge(), "blocked:external");
        assert_eq!(row("execution-stall").status_badge(), "stalled");

        // The external wait exposes its category, condition, and next action.
        let detail = row("external-wait")
            .blocker_detail_cache
            .clone()
            .expect("external detail");
        assert!(detail.contains("credential"));
        assert!(detail.contains("unblock when"));
        assert!(detail.contains("next action"));

        // Retry routing follows the blocker kind, not the shared status word.
        use crate::orchestration::operator_command::classify_retry_route;
        assert!(classify_retry_route(
            &row("dependency-wait").display_status_cache,
            row("dependency-wait").blocker_kind_cache
        )
        .is_none());
        assert!(classify_retry_route(
            &row("external-wait").display_status_cache,
            row("external-wait").blocker_kind_cache
        )
        .is_some());
        assert!(classify_retry_route(
            &row("execution-stall").display_status_cache,
            row("execution-stall").blocker_kind_cache
        )
        .is_some());
    }

    // ── Resolve ─────────────────────────────────────────────────────────────

    /// Put a change into a reducer-visible merge wait.
    async fn to_merge_wait(harness: &AdapterHarness, change_id: &str) {
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::MergeDeferred {
                change_id: change_id.to_string(),
                reason: "manual resolution required".to_string(),
                auto_resumable: false,
            },
        );
    }

    #[tokio::test]
    async fn test_resolve_merge_starts_scheduler_when_idle() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        to_merge_wait(&harness, "change-a").await;

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;

        assert_eq!(
            harness.scheduler.started_targets(),
            vec![Vec::<String>::new()],
            "an idle scheduler is started to consume reducer-owned ResolveWait"
        );
        assert_eq!(log_count(&app, "started scheduler for manual resolve"), 1);
        assert_eq!(harness.status("change-a").await, "resolve pending");
        assert_eq!(
            harness.state.read().await.resolve_wait_change_ids(),
            vec!["change-a".to_string()]
        );
        assert_eq!(app.mode, AppMode::Running);
    }

    #[tokio::test]
    async fn test_resolve_merge_notifies_live_scheduler_without_duplicate_spawn() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        to_merge_wait(&harness, "change-a").await;
        harness.scheduler.set_running(true);

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;

        assert_eq!(harness.scheduler.calls(), vec![SchedulerCall::Notified]);
        assert_eq!(log_count(&app, "notified existing scheduler"), 1);
    }

    #[tokio::test]
    async fn test_resolve_merge_noop_does_not_notify_or_log_scheduled() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::MergeCompleted {
                change_id: "change-a".to_string(),
                revision: "rev-a".to_string(),
            },
        );
        harness.scheduler.set_running(true);

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;

        assert!(harness.scheduler.calls().is_empty());
        assert_eq!(log_count(&app, "was not accepted by scheduler state"), 1);
        assert_eq!(log_count(&app, "Scheduled merge-wait retry intent"), 0);
        assert!(harness
            .state
            .read()
            .await
            .resolve_wait_change_ids()
            .is_empty());
    }

    #[tokio::test]
    async fn a_second_resolve_queues_behind_the_active_resolver_without_a_second_dispatch() {
        let harness = AdapterHarness::new(&["change-a", "change-b"]);
        let mut app = harness.app(&["change-a", "change-b"]);
        to_merge_wait(&harness, "change-a").await;
        to_merge_wait(&harness, "change-b").await;

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;
        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-b".to_string()))
            .await;
        // A duplicate submission must not create a second queue entry.
        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-b".to_string()))
            .await;

        assert_eq!(harness.resolves.waiting(), vec!["change-b".to_string()]);
        assert_eq!(
            harness.scheduler.started_targets().len(),
            1,
            "only the active resolver dispatches scheduler work"
        );
        assert_eq!(log_count(&app, "Queued 'change-b' for resolve"), 1);
        assert_eq!(log_count(&app, "already queued for resolve"), 1);
    }

    // ── Stop family ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn stop_requests_graceful_stop_only_while_running() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.mode = AppMode::Running;

        harness.run(&mut app, TuiCommand::Stop).await;

        assert_eq!(app.mode, AppMode::Stopping);
        assert_eq!(app.stop_mode, StopMode::GracefulPending);
        assert_eq!(
            harness.scheduler.calls(),
            vec![SchedulerCall::GracefulStop(true)]
        );

        // A second stop in Stopping mode is refused without a further effect.
        harness.run(&mut app, TuiCommand::Stop).await;
        assert_eq!(
            harness.scheduler.calls(),
            vec![SchedulerCall::GracefulStop(true)]
        );
        assert!(app
            .warning_message
            .as_deref()
            .is_some_and(|message| message.contains("stop is not available")));
    }

    #[tokio::test]
    async fn cancel_stop_returns_to_running_only_from_stopping() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.mode = AppMode::Stopping;
        app.stop_mode = StopMode::GracefulPending;

        harness.run(&mut app, TuiCommand::CancelStop).await;

        assert_eq!(app.mode, AppMode::Running);
        assert_eq!(app.stop_mode, StopMode::None);
        assert_eq!(
            harness.scheduler.calls(),
            vec![SchedulerCall::GracefulStop(false)]
        );

        harness.run(&mut app, TuiCommand::CancelStop).await;
        assert_eq!(
            harness.scheduler.calls(),
            vec![SchedulerCall::GracefulStop(false)],
            "cancel stop without a pending stop must have no effect"
        );
    }

    /// Run `TuiCommand::ForceStop` against a prepared activity snapshot.
    async fn run_force_stop(
        harness: &AdapterHarness,
        activity: crate::tui::stop_classification::StopActivitySnapshot,
        scheduler_running: bool,
    ) -> AppState {
        harness.scheduler.set_activity(activity);
        harness.scheduler.set_running(scheduler_running);
        let mut app = harness.app(&["change-a"]);
        app.mode = AppMode::Stopping;
        app.stop_mode = StopMode::ImmediatePending;
        app.changes[0].selected = true;
        app.changes[0].set_display_status_cache("applying");
        app.publish_execution_marks();

        harness.run(&mut app, TuiCommand::ForceStop).await;
        app
    }

    fn activity(
        registered: usize,
        shutdown_pending: bool,
    ) -> crate::tui::stop_classification::StopActivitySnapshot {
        use crate::tui::stop_classification::{
            ExecutionEvidence, ShutdownWorkEvidence, StopActivitySnapshot,
        };
        StopActivitySnapshot {
            execution_handles: ExecutionEvidence::Known { registered },
            reducer_agent_execution_active: false,
            shutdown_work: ShutdownWorkEvidence::Known {
                pending: shutdown_pending,
            },
        }
    }

    #[tokio::test]
    async fn idle_parallel_stop_active_execution_reports_force_stop() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(1, false), true).await;

        assert!(
            harness
                .scheduler
                .calls()
                .contains(&SchedulerCall::Cancelled),
            "an active execution must still request managed cancellation"
        );
        assert_eq!(log_count(&app, "Force stopped"), 1);
        assert_eq!(app.stop_mode, StopMode::ForceStopped);
        assert_eq!(
            app.mode,
            AppMode::Stopping,
            "the scheduler owns terminal stop while in-flight cleanup is pending"
        );
        assert_eq!(log_count(&app, "Processing stopped"), 0);
    }

    #[tokio::test]
    async fn idle_parallel_stop_merge_wait_does_not_claim_process_termination() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(0, false), true).await;

        assert!(harness
            .scheduler
            .calls()
            .contains(&SchedulerCall::Cancelled));
        assert_eq!(
            log_count(&app, "Force stopped"),
            0,
            "an idle wait must not claim forceful process termination"
        );
        assert_eq!(log_count(&app, "Processing stopped"), 1);
        assert_eq!(app.mode, AppMode::Stopped);
    }

    #[tokio::test]
    async fn idle_parallel_stop_pending_background_merge_waits_for_safe_boundary() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(0, true), true).await;

        assert!(harness
            .scheduler
            .calls()
            .contains(&SchedulerCall::Cancelled));
        assert_eq!(
            log_count(&app, "Force stopped"),
            0,
            "a background merge is shutdown work, not a force-stopped agent process"
        );
        assert_eq!(
            app.mode,
            AppMode::Stopping,
            "terminal stop waits for the base-lane operation to reach its boundary"
        );
        assert_eq!(log_count(&app, "Processing stopped"), 0);
    }

    #[tokio::test]
    async fn idle_parallel_stop_without_live_scheduler_applies_terminal_stop() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(0, true), false).await;

        assert_eq!(
            app.mode,
            AppMode::Stopped,
            "with no live scheduler there is nothing left to drain"
        );
        assert_eq!(log_count(&app, "Processing stopped"), 1);
    }

    #[tokio::test]
    async fn idle_parallel_stop_preserves_execution_marks_and_resets_queue_status() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(0, false), true).await;

        assert!(
            app.changes[0].selected,
            "stopped-state handling must preserve execution marks"
        );
        assert_eq!(
            app.execution_marks().marked_ids(),
            vec!["change-a".to_string()]
        );
        assert_eq!(
            app.changes[0].display_status_cache, "not queued",
            "transient in-flight presentation must reset on stop"
        );
    }

    #[tokio::test]
    async fn force_stop_outside_running_or_stopping_cancels_nothing() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.mode = AppMode::Select;

        harness.run(&mut app, TuiCommand::ForceStop).await;

        assert!(
            harness.scheduler.calls().is_empty(),
            "a refused force stop must not cancel the run"
        );
    }
}

/// The local run supervisor's own dispatch guards.
///
/// These drive the real [`TuiRunSupervisor`], which owns process/task spawning,
/// so they are integration-scoped rather than unit-scoped.
#[cfg(test)]
mod run_supervisor_tests {
    use super::tests::create_test_change;
    use super::*;
    use crate::orchestration::operator_command::{
        ExecutionMarkStore, NoopQueueHooks, OperatorCommandService,
    };
    use crate::orchestration::run_control::{
        ResolveReservations, RunSchedulerPort, StartEligibility,
    };
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::run_supervisor::TuiRunSupervisor;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn upstream_runtime() -> crate::upstream::UpstreamRuntime {
        crate::upstream::UpstreamRuntime {
            config: crate::upstream::UpstreamIntegrationConfig::new("origin", "exit 0"),
            branch: "main".to_string(),
        }
    }

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

    /// A repository whose cumulative base carries an archived, unpublished change.
    fn per_change_upstream_unpublished_repo() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let remote = dir.path().join("remote.git");
        std::fs::create_dir_all(&remote).ok()?;
        git_in(&remote, &["init", "--bare", "--initial-branch=main"])?;

        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).ok()?;
        git_in(&root, &["init", "--initial-branch=main"])?;
        git_in(&root, &["config", "user.email", "test@example.com"])?;
        git_in(&root, &["config", "user.name", "Test"])?;
        git_in(&root, &["remote", "add", "origin", remote.to_str()?])?;
        std::fs::write(root.join("README.md"), "base\n").ok()?;
        git_in(&root, &["add", "-A"])?;
        git_in(&root, &["commit", "-m", "base"])?;
        git_in(&root, &["push", "-u", "origin", "main"])?;

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
    /// and the operator's explicit retry (F5, or a remote `start`) must resume
    /// *publication* — not rerun apply. This drives the real command handler over
    /// the real supervisor, so retry routing, the runtime hand-off, and the
    /// resumption are covered as one path.
    #[tokio::test]
    #[cfg_attr(not(feature = "heavy-tests"), ignore)]
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
        let (tx, mut rx) = mpsc::channel(256);
        let queue = DynamicQueue::new();
        let marks = Arc::new(ExecutionMarkStore::new());
        marks.set("alpha", true);
        let parallel_mode = Arc::new(AtomicBool::new(true));
        let supervisor = Arc::new(TuiRunSupervisor::new(
            root.clone(),
            config.clone(),
            tx.clone(),
            queue.clone(),
            shared_state.clone(),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            PostArchiveAction::MergeToBase,
            Some(upstream_runtime()),
            Arc::new(AtomicBool::new(false)),
            parallel_mode,
            #[cfg(feature = "web-monitoring")]
            None,
        ));
        let run_control = Arc::new(RunControlService::new(
            shared_state.clone(),
            Arc::new(OperatorCommandService::new(
                shared_state.clone(),
                Arc::new(queue.clone()),
                Arc::new(NoopQueueHooks),
                marks.clone(),
            )),
            supervisor.clone(),
            Arc::new(ResolveReservations::new()),
            Arc::new(StartEligibility::new()),
        ));

        let mut app = AppState::new(vec![create_test_change("alpha")]);
        app.parallel_mode = true;
        app.set_shared_state(shared_state.clone());
        app.apply_display_statuses_from_reducer(&shared_state.read().await.all_display_statuses());
        assert_eq!(app.changes[0].display_status_cache, "error");
        app.mode = AppMode::Error;
        app.changes[0].selected = true;

        {
            let mut ctx = TuiCommandContext {
                app: &mut app,
                repo_root: &root,
                config: &config,
                tx: &tx,
                run_control: &run_control,
                #[cfg(feature = "web-monitoring")]
                web_state: &None,
            };
            // A remote `start` carries no IDs either; Error mode turns both into
            // the same explicit retry.
            handle_start_processing_command(Vec::new(), &mut ctx).await;
        }
        assert!(
            supervisor.is_running(),
            "explicit retry must start a local orchestrator run"
        );

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
        let (handle, cancel) = supervisor.take_run();
        cancel
            .expect("retry must own a cancellation token")
            .cancel();
        if let Some(handle) = handle {
            let _ = handle.await;
        }

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
        let dispatched = dir.path().join("dispatched.txt");
        let config = OrchestratorConfig {
            apply_command: Some(format!(
                "sh -c 'echo apply >> \"{}\"'",
                dispatched.display()
            )),
            ..OrchestratorConfig::default()
        };

        let (tx, _rx) = mpsc::channel(64);
        let state = Arc::new(RwLock::new(OrchestratorState::with_mode(
            vec!["alpha".to_string()],
            0,
            crate::orchestration::state::ExecutionMode::Parallel,
        )));
        // Serial mode while the session is opted in.
        let parallel_mode = Arc::new(AtomicBool::new(false));
        let supervisor = TuiRunSupervisor::new(
            dir.path().to_path_buf(),
            config,
            tx,
            DynamicQueue::new(),
            state,
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            PostArchiveAction::MergeToBase,
            Some(upstream_runtime()),
            Arc::new(AtomicBool::new(false)),
            parallel_mode.clone(),
            #[cfg(feature = "web-monitoring")]
            None,
        );

        let refusal = supervisor
            .start_run(vec!["alpha".to_string()], false)
            .await
            .expect_err("an opted-in session must not dispatch serial work");

        assert!(
            refusal.contains("-u/--integrate-upstream"),
            "the refusal must say why: {refusal}"
        );
        assert!(!supervisor.is_running(), "no run may have been spawned");
        assert!(
            !dispatched.exists(),
            "the refused dispatch must run no apply command"
        );

        // Restoring parallel mode restores the publication contract.
        parallel_mode.store(true, std::sync::atomic::Ordering::SeqCst);
        supervisor
            .start_run(Vec::new(), false)
            .await
            .expect("parallel dispatch is allowed for an opted-in session");
        let (handle, cancel) = supervisor.take_run();
        if let Some(cancel) = cancel {
            cancel.cancel();
        }
        if let Some(handle) = handle {
            handle.abort();
        }
        let _ = create_test_change("alpha");
    }
}
