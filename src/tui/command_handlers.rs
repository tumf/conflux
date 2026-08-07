//! TuiCommand handlers for TUI
//!
//! This module contains helper functions to handle TuiCommand processing.

use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::orchestration::operator_command::{OperatorOutcome, QueueMutation};
use crate::orchestration::operator_coordinator::{
    ApplicationOutcome, ApplicationResult, OperatorApplication, OperatorIntent,
};
use crate::orchestration::run_control::{
    ResolveReservation, RunControlError, RunControlOutcome, RunNoOpReason, SchedulerEffect,
};
#[cfg(test)]
use crate::parallel::PostArchiveAction;
use crate::tui::events::{LogEntry, OrchestratorEvent, TuiCommand};
#[cfg(test)]
use crate::tui::queue::DynamicQueue;
use crate::tui::state::AppState;
use crate::tui::types::DeleteIntent;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// TUI ↔ `/api/v2` parity, verified over one recording runtime.
#[cfg(all(test, feature = "web-monitoring"))]
mod cross_adapter_tests;

/// Same-process convergence: one command, both frontends, next frame.
#[cfg(all(test, feature = "web-monitoring"))]
mod convergence_tests;

#[cfg(test)]
#[derive(Clone, Debug)]
enum DeleteWorktreeTestOutcome {
    Success,
    Failure(String),
}

/// What the stub backend observes and does for one registered worktree.
#[cfg(test)]
#[derive(Clone, Debug)]
struct DeleteWorktreeTestState {
    /// Branch `observe()` reports for this path.
    ///
    /// Per-path rather than fixed so a test that scripts a *branch*-keyed
    /// outcome can claim a name of its own instead of one every other
    /// concurrently running test also observes.
    branch: String,
    /// Dirty state `observe()` reports for this path.
    dirty: crate::worktree_ops::service::DirtyState,
    /// Commits-ahead state `observe()` reports for this path.
    has_commits_ahead: crate::worktree_ops::service::SafetyFact,
    /// What `remove_worktree()` replays.
    removal: DeleteWorktreeTestOutcome,
}

/// Branches whose deletion fails, keyed by branch name.
///
/// Separate from [`DELETE_WORKTREE_TEST_OUTCOMES`] because branch cleanup runs
/// *after* `remove_worktree` has already retired the worktree's entry — which is
/// the whole state the partial-success path exists to describe.
#[cfg(test)]
static DELETE_BRANCH_TEST_FAILURES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
fn set_delete_branch_test_failure(branch: &str, message: &str) {
    DELETE_BRANCH_TEST_FAILURES
        .lock()
        .expect("delete branch test failures lock")
        .insert(branch.to_string(), message.to_string());
}

#[cfg(test)]
static DELETE_WORKTREE_TEST_OUTCOMES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<PathBuf, DeleteWorktreeTestState>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[cfg(test)]
fn set_delete_worktree_test_outcome(path: PathBuf, outcome: DeleteWorktreeTestOutcome) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .insert(
            path,
            DeleteWorktreeTestState {
                removal: outcome,
                ..delete_worktree_test_state()
            },
        );
}

/// A registered worktree that is observable, clean, at base, and removable.
#[cfg(test)]
fn delete_worktree_test_state() -> DeleteWorktreeTestState {
    DeleteWorktreeTestState {
        branch: "feature-a".to_string(),
        dirty: crate::worktree_ops::service::DirtyState::Clean,
        has_commits_ahead: crate::worktree_ops::service::SafetyFact::No,
        removal: DeleteWorktreeTestOutcome::Success,
    }
}

#[cfg(test)]
fn set_delete_worktree_test_branch(path: PathBuf, branch: &str) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .entry(path)
        .or_insert_with(delete_worktree_test_state)
        .branch = branch.to_string();
}

#[cfg(test)]
fn set_delete_worktree_test_dirty(path: PathBuf, dirty: crate::worktree_ops::service::DirtyState) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .entry(path)
        .or_insert_with(delete_worktree_test_state)
        .dirty = dirty;
}

#[cfg(test)]
fn set_delete_worktree_test_ahead(
    path: PathBuf,
    has_commits_ahead: crate::worktree_ops::service::SafetyFact,
) {
    DELETE_WORKTREE_TEST_OUTCOMES
        .lock()
        .expect("delete worktree test outcomes lock")
        .entry(path)
        .or_insert_with(delete_worktree_test_state)
        .has_commits_ahead = has_commits_ahead;
}

use super::worktrees::load_worktrees_with_conflict_check;
use crate::worktree_ops::service::{
    ConflictPolicy, DeleteOptions, ExpectedTarget, WorktreeBackend, WorktreeEventSink,
    WorktreeOpError, WorktreeOperationEvent, WorktreeService,
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

/// Build *the* worktree operation service for one repository.
///
/// Called once per process, at startup, and shared from there: the service owns
/// the repository mutation guard, so a second instance would be a second guard
/// and two overlapping mutations could each believe they held it. The TUI and
/// `/api/v2` are handed the same `Arc`.
pub(crate) fn build_worktree_service(
    repo_root: &Path,
    config: &OrchestratorConfig,
    tx: &mpsc::Sender<OrchestratorEvent>,
) -> Arc<WorktreeService> {
    let workspace_base_dir = config
        .get_workspace_base_dir()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| crate::config::defaults::default_workspace_base_dir(Some(repo_root)));
    Arc::new(WorktreeService::new(
        build_worktree_backend(repo_root, config, tx),
        Arc::new(TuiWorktreeEvents {
            tx: tx.clone(),
            repo_root: repo_root.to_path_buf(),
        }),
        workspace_base_dir,
    ))
}

/// Operator-facing feedback from a command that settled off the event loop.
///
/// The accepted *state* never travels this way — it reaches the TUI through the
/// authoritative dispatch, exactly as a remote command's would. What travels
/// here is only wording: the log line and, when the command did not do what the
/// operator asked, the message the status bar surfaces.
#[derive(Debug, Clone)]
pub struct CommandFeedback {
    /// The line to append to the operator log.
    pub log: LogEntry,
    /// Set when the operator must be told the command was refused or changed
    /// nothing.
    pub warning: Option<String>,
}

impl CommandFeedback {
    /// A command that did what it was asked to do.
    fn accepted(log: LogEntry) -> Self {
        Self { log, warning: None }
    }

    /// A refusal or a no-op, surfaced as well as logged.
    fn refused(message: String) -> Self {
        Self {
            log: LogEntry::warn(message.clone()),
            warning: Some(message),
        }
    }
}

/// One operator intent queued for the shared transaction, with the wording its
/// settlement produces.
///
/// Queued rather than executed inline because the coordinator gate is a real
/// lock a remote command can be holding: awaiting it inside the TUI's
/// event-processing loop would stall rendering and event drain behind another
/// frontend's command.
pub struct Submission {
    intent: OperatorIntent,
    describe: Box<dyn FnOnce(ApplicationResult) -> CommandFeedback + Send>,
}

impl Submission {
    /// Build a submission from an intent and its settlement wording.
    pub fn new(
        intent: OperatorIntent,
        describe: impl FnOnce(ApplicationResult) -> CommandFeedback + Send + 'static,
    ) -> Self {
        Self {
            intent,
            describe: Box::new(describe),
        }
    }

    /// Run this submission against the shared transaction.
    ///
    /// The worker that drives these is single-consumer, so keypresses reach the
    /// coordinator in the order the operator made them.
    pub async fn run(self, application: &OperatorApplication) -> CommandFeedback {
        let result = application.apply(self.intent).await;
        (self.describe)(result)
    }
}

/// Context for TuiCommand handling
pub struct TuiCommandContext<'a> {
    pub app: &'a mut AppState,
    pub tx: &'a mpsc::Sender<OrchestratorEvent>,
    /// The single process-local application transaction shared with `/api/v2`.
    ///
    /// Every start, stop, retry, mark, queue, dequeue, and resolve in this module
    /// goes through it, so a keypress and a remote command cannot resolve the
    /// same intent differently — and neither can apply a lifecycle transition the
    /// coordinator has not accepted.
    pub application: &'a Arc<OperatorApplication>,
    /// Ordered submission queue drained by a worker outside the render loop.
    pub submissions: &'a mpsc::Sender<Submission>,
    /// Feedback channel the worker reports settlement wording on.
    pub feedback: &'a mpsc::Sender<CommandFeedback>,
    /// The single process-local worktree service shared with `/api/v2`.
    ///
    /// One instance means one repository mutation guard, which is what makes a
    /// keypress and a remote command serialize against each other instead of
    /// racing through two independent guards over the same repository.
    pub worktree_service: &'a Arc<WorktreeService>,
}

/// Queue one intent for the shared transaction.
///
/// A full queue is the one case where the TUI answers for itself, and it answers
/// with a bounded typed refusal rather than by blocking: an operator whose
/// keypress could not be queued is told so immediately instead of watching the
/// interface freeze.
fn submit(
    ctx: &mut TuiCommandContext<'_>,
    intent: OperatorIntent,
    describe: impl FnOnce(ApplicationResult) -> CommandFeedback + Send + 'static,
) {
    if let Err(error) = ctx.submissions.try_send(Submission::new(intent, describe)) {
        let message = format!("Command could not be queued right now: {error}");
        ctx.app.warning_message = Some(message.clone());
        ctx.app.add_log(LogEntry::warn(message));
    }
}

/// How the shared transaction's settlement of a Start reads to an operator.
fn describe_start(retry: RetryContext) -> impl FnOnce(ApplicationResult) -> CommandFeedback {
    move |result| match result.outcome {
        Ok(ApplicationOutcome::Run(RunControlOutcome::RunDispatched {
            change_ids,
            scheduler,
            ..
        })) => {
            let verb = match scheduler {
                SchedulerEffect::Started => "Starting",
                _ => "Queued for the running scheduler:",
            };
            CommandFeedback::accepted(LogEntry::info(format!(
                "{} processing {} change(s)",
                verb,
                change_ids.len()
            )))
        }
        Ok(ApplicationOutcome::Run(RunControlOutcome::NoOp { reason })) => {
            CommandFeedback::refused(no_op_message(&reason, retry))
        }
        Ok(other) => {
            debug!("Start produced an unexpected outcome: {:?}", other);
            CommandFeedback::accepted(LogEntry::info("Start produced no reportable effect"))
        }
        Err(error) => CommandFeedback::refused(error.to_string()),
    }
}

/// The Apply-ceiling facts a retry refusal has to be worded against.
///
/// Captured at submission time because the message is produced on a worker task
/// that has no access to this frontend's rows.
#[derive(Debug, Clone, Copy)]
struct RetryContext {
    active_limit: bool,
    admissible_target: bool,
}

impl RetryContext {
    fn observe(app: &AppState) -> Self {
        Self {
            active_limit: app.has_active_apply_iteration_limit(),
            admissible_target: app.has_admissible_retry_target(),
        }
    }
}

/// Message for a retry the shared service settled without a target.
///
/// `NoRetryableTarget` is truthful but not actionable when the reason is an
/// active-run Apply ceiling, so the active condition is named when one exists.
fn no_retryable_target_message(retry: RetryContext) -> String {
    if retry.active_limit && !retry.admissible_target {
        return format!(
            "Retry is unavailable: every candidate is {}",
            crate::tui::state::ACTIVE_APPLY_LIMIT_EXPLANATION
        );
    }
    "No marked change carries retryable evidence".to_string()
}

fn no_op_message(reason: &RunNoOpReason, retry: RetryContext) -> String {
    match reason {
        RunNoOpReason::ResolveAlreadyReserved { change_id } => {
            format!("Change '{}' is already queued for resolve", change_id)
        }
        RunNoOpReason::NoRetryableTarget => no_retryable_target_message(retry),
    }
}

/// Handle TuiCommand::StartProcessing.
///
/// The TUI is an adapter here: it queues the intent and words what the shared
/// transaction settled. It does not choose targets, apply queue intent, decide
/// the mode, or decide whether the scheduler should be spawned or woken — the
/// coordinator owns all four, so `/api/v2` start reaches the same decision.
///
/// A non-empty `ids` list records those targets in the authoritative mark store
/// first, one change at a time, so even an explicit selection is started through
/// it. The write is target-scoped on purpose: replacing the whole store from a
/// caller-supplied list would also clear marks this frontend never observed.
pub async fn handle_start_processing_command(ids: Vec<String>, ctx: &mut TuiCommandContext<'_>) {
    if !ids.is_empty() {
        let service = ctx.application.run_control().operator();
        for id in &ids {
            service.apply_execution_mark(id, true).await;
        }
        ctx.app.sync_execution_marks_from_store();
    }

    let retry = RetryContext::observe(ctx.app);
    submit(ctx, OperatorIntent::Start, describe_start(retry));
}

/// Handle a confirmed worktree deletion.
///
/// Adapter only: the shared service owns the delete guards, the teardown/removal
/// split, the second observation, branch cleanup, and the refresh event. What
/// this function owns is the *escalation* — the one refusal the TUI answers with
/// a second confirmation instead of a warning.
///
/// The identity the modal confirmed travels with the command so the service can
/// revalidate it under its own mutation guard: the pre-dispatch modal check is
/// necessary but not sufficient, since the path can be re-occupied between
/// confirmation and mutation.
async fn handle_delete_worktree_command(intent: DeleteIntent, ctx: &mut TuiCommandContext<'_>) {
    let mut expected = ExpectedTarget::on_branch(intent.branch.clone());
    if let Some(identity) = intent.identity.clone() {
        expected = expected.with_identity(identity);
    }
    if let Some(head) = intent.head.clone() {
        expected = expected.with_head(head);
    }

    let options = if intent.allow_commits_ahead {
        DeleteOptions::local_discarding_ahead(intent.skip_teardown, intent.allow_known_dirty)
    } else if intent.allow_known_dirty {
        DeleteOptions::local_discarding_dirty(intent.skip_teardown)
    } else {
        DeleteOptions::local(intent.skip_teardown)
    };

    let delete_result = ctx
        .worktree_service
        .delete_worktree(&intent.path, &expected, options)
        .await;
    ctx.app.clear_worktree_deleting(&intent.path);

    match delete_result {
        // The worktree is gone but its branch is not. That is a success with a
        // resource still outstanding, not a plain success: reporting it as one
        // would leave the operator believing a branch they still own was cleaned
        // up.
        Ok(outcome) if outcome.branch_retained => {
            warn!(
                "Worktree deleted but its branch was retained: {}",
                intent.path.display()
            );
            ctx.app.add_log(LogEntry::warn(format!(
                "Partially deleted worktree: {} ({})",
                intent.path.display(),
                outcome.detail
            )));
        }
        Ok(outcome) => {
            info!("Worktree deleted successfully: {}", intent.path.display());
            ctx.app.add_log(LogEntry::success(format!(
                "Deleted worktree: {} ({})",
                intent.path.display(),
                outcome.detail
            )));
        }
        // A known-dirty refusal is the escalation, not a failure: the operator
        // asked to delete something that still holds uncommitted work, and the
        // answer is a second confirmation naming exactly what the service just
        // observed. `allow_known_dirty` was already granted here only if the
        // refusal is about something else, so this never loops.
        Err(WorktreeOpError::Dirty { target, .. }) if !intent.allow_known_dirty => {
            ctx.app
                .open_dirty_discard_confirmation(&target, intent.skip_teardown);
        }
        // The second escalation, on the same terms: a known-ahead refusal opens
        // the confirmation that names the branch and its unmerged commits.
        Err(WorktreeOpError::CommitsAhead { target, .. }) if !intent.allow_commits_ahead => {
            ctx.app
                .open_ahead_discard_confirmation(&target, intent.skip_teardown);
        }
        Err(e) => {
            ctx.app.show_warning_popup(
                "Worktree delete failed",
                format!(
                    "Failed to delete worktree '{}': {}",
                    intent.path.display(),
                    e
                ),
            );
            ctx.app.add_log(LogEntry::error(format!(
                "Worktree delete failed for '{}': {}",
                intent.path.display(),
                e
            )));
        }
    }
}

/// Handle TuiCommand - main dispatcher
pub async fn handle_tui_command(
    cmd: TuiCommand,
    ctx: &mut TuiCommandContext<'_>,
    // Retained so a handler that needs a reducer read has one without reaching
    // through the coordinator. No current handler does: every lifecycle decision
    // is the shared transaction's, and every reducer-derived row refresh happens
    // on the authoritative dispatch path.
    _shared_state: &Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
) -> Result<()> {
    match cmd {
        TuiCommand::StartProcessing(ids) => {
            handle_start_processing_command(ids, ctx).await;
        }
        TuiCommand::AddToQueue(id) => {
            // Adapter only: the shared transaction owns reducer ordering, dynamic
            // queue mutation, on_queue_add cardinality, and the outcome dispatch
            // that carries the delta to every other frontend.
            let intent = OperatorIntent::SetQueueIntent {
                change_id: id.clone(),
                queued: true,
            };
            submit(ctx, intent, move |result| match result.outcome {
                Err(error) => CommandFeedback::refused(format!("Queue add rejected: {}", error)),
                Ok(ApplicationOutcome::Operator(OperatorOutcome::Queue(queue)))
                    if !queue.reducer_changed =>
                {
                    CommandFeedback::refused(format!("Queue add ignored by reducer: {}", id))
                }
                Ok(ApplicationOutcome::Operator(OperatorOutcome::Queue(queue))) => {
                    if queue.dynamic_queue_mutated {
                        CommandFeedback::accepted(LogEntry::info(format!(
                            "Added to dynamic queue: {}",
                            id
                        )))
                    } else {
                        CommandFeedback::refused(format!("Already in dynamic queue: {}", id))
                    }
                }
                Ok(other) => {
                    debug!("Queue add produced an unexpected outcome: {:?}", other);
                    CommandFeedback::accepted(LogEntry::info(format!("Queue add settled: {}", id)))
                }
            });
        }
        TuiCommand::RemoveFromQueue(id) => {
            // Adapter only, for the same reason as the addition above.
            let intent = OperatorIntent::SetQueueIntent {
                change_id: id.clone(),
                queued: false,
            };
            submit(ctx, intent, move |result| match result.outcome {
                Err(error) => CommandFeedback::refused(format!("Queue remove rejected: {}", error)),
                Ok(ApplicationOutcome::Operator(OperatorOutcome::Queue(queue))) => {
                    debug_assert_eq!(queue.mutation, QueueMutation::Removed);
                    let suffix = if queue.dynamic_queue_mutated {
                        " (dynamic queue updated)"
                    } else {
                        ""
                    };
                    CommandFeedback::accepted(LogEntry::info(format!(
                        "Removed from queue: {}{}",
                        id, suffix
                    )))
                }
                Ok(other) => {
                    debug!("Queue remove produced an unexpected outcome: {:?}", other);
                    CommandFeedback::accepted(LogEntry::info(format!(
                        "Queue remove settled: {}",
                        id
                    )))
                }
            });
        }
        TuiCommand::DeleteWorktree(intent) => {
            handle_delete_worktree_command(intent, ctx).await;
        }
        TuiCommand::Stop => {
            // Adapter only: the shared transaction owns the mode matrix, the
            // graceful-stop request, and the `Stopping` dispatch that moves every
            // frontend's mode. The TUI reports refusals verbatim and nothing else.
            submit(ctx, OperatorIntent::Stop, |result| match result.outcome {
                Ok(ApplicationOutcome::Run(RunControlOutcome::StopRequested)) => {
                    CommandFeedback::accepted(LogEntry::warn(
                        "Stopping after current change completes...",
                    ))
                }
                Ok(other) => {
                    debug!("Stop produced an unexpected outcome: {:?}", other);
                    CommandFeedback::accepted(LogEntry::info("Stop settled"))
                }
                Err(error) => CommandFeedback::refused(error.to_string()),
            });
        }
        TuiCommand::CancelStop => {
            submit(ctx, OperatorIntent::CancelStop, |result| {
                match result.outcome {
                    Ok(ApplicationOutcome::Run(RunControlOutcome::StopCancelled)) => {
                        CommandFeedback::accepted(LogEntry::info("Stop canceled, continuing..."))
                    }
                    Ok(other) => {
                        debug!("Cancel stop produced an unexpected outcome: {:?}", other);
                        CommandFeedback::accepted(LogEntry::info("Cancel stop settled"))
                    }
                    Err(error) => CommandFeedback::refused(error.to_string()),
                }
            });
        }
        TuiCommand::ForceStop => {
            // Immediate stop. The force-vs-ordinary decision comes from the one
            // runtime activity snapshot the shared service takes, and cancellation
            // is issued there for both reporting classes. Which authoritative
            // event that produces — a waiting `OperatorCommandApplied` or a
            // settled `Stopped` — is the coordinator's call, so the row lifecycle
            // for a process stop keeps exactly one authority.
            submit(ctx, OperatorIntent::ForceStop, |result| {
                match result.outcome {
                    Ok(ApplicationOutcome::Run(RunControlOutcome::ForceStopped {
                        classification,
                        ..
                    })) => {
                        let message = if classification.process_report.is_force_stop() {
                            "Force stopped"
                        } else {
                            "Run cancelled; no agent execution was active"
                        };
                        CommandFeedback::accepted(LogEntry::warn(message))
                    }
                    Ok(other) => {
                        debug!("Force stop produced an unexpected outcome: {:?}", other);
                        CommandFeedback::accepted(LogEntry::info("Force stop settled"))
                    }
                    Err(error) => CommandFeedback::refused(error.to_string()),
                }
            });
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
            let service = ctx.worktree_service.clone();
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
            // The two-phase intent. Its confirmation wait is bounded but can take
            // seconds, so the whole submission runs off this loop: awaiting the
            // application gate or a termination waiter inside event processing
            // would stall rendering and event fan-out for every other change.
            //
            // The accepted state reaches this frontend through the authoritative
            // dispatch like any other command's; only the operator-facing wording
            // comes back on the log channel.
            //
            // It gets its *own* task rather than the ordered submission queue:
            // queueing it would let one never-completing waiter hold every later
            // keypress, which is the exact monopoly the two-phase split exists to
            // prevent.
            let application = ctx.application.clone();
            let feedback = ctx.feedback.clone();
            ctx.app.add_log(LogEntry::info(format!(
                "Stop-and-dequeue request received for: {}",
                id
            )));
            tokio::spawn(async move {
                let intent = OperatorIntent::StopAndDequeue {
                    change_id: id.clone(),
                };
                let settled = match application.apply(intent).await.outcome {
                    Ok(ApplicationOutcome::Operator(OperatorOutcome::Dequeued { change_id })) => {
                        CommandFeedback::accepted(LogEntry::success(format!(
                            "Stopped and dequeued after confirmed termination: {}",
                            change_id
                        )))
                    }
                    Ok(_) => {
                        CommandFeedback::refused(format!("Stop-and-dequeue ignored for: {}", id))
                    }
                    Err(error) => {
                        warn!("Stop-and-dequeue failed for {}: {}", id, error);
                        CommandFeedback::refused(format!("Stop-and-dequeue failed: {}", error))
                    }
                };
                let _ = feedback.send(settled).await;
            });
        }
        TuiCommand::ResolveMerge(id) => {
            // Adapter only: the shared transaction owns the reducer intent, the
            // single-resolver reservation, FIFO ordering, duplicate rejection,
            // the mode transition, and whether the scheduler is started or merely
            // woken.
            let retry = RetryContext::observe(ctx.app);
            let intent = OperatorIntent::ResolveMerge {
                change_id: id.clone(),
            };
            submit(ctx, intent, move |result| {
                match result.outcome {
                Ok(ApplicationOutcome::Run(RunControlOutcome::ResolveReserved {
                    change_id,
                    reservation,
                    scheduler,
                })) => match reservation {
                    ResolveReservation::Active => {
                        let how = match scheduler {
                            SchedulerEffect::Started => "started scheduler for manual resolve",
                            _ => "notified existing scheduler",
                        };
                        CommandFeedback::accepted(LogEntry::info(format!(
                            "Scheduled merge-wait retry intent for '{}'; {}",
                            change_id, how
                        )))
                    }
                    ResolveReservation::Queued { position } => {
                        CommandFeedback::accepted(LogEntry::info(format!(
                            "Queued '{}' for resolve (position: {})",
                            change_id, position
                        )))
                    }
                },
                Ok(ApplicationOutcome::Run(RunControlOutcome::NoOp { reason })) => {
                    CommandFeedback::refused(no_op_message(&reason, retry))
                }
                Ok(other) => {
                    debug!("Resolve produced an unexpected outcome: {:?}", other);
                    CommandFeedback::accepted(LogEntry::info("Resolve settled"))
                }
                // A stale resolve target gets a resolve-specific message, but it
                // is still surfaced the way every other refusal is: the operator
                // must not have to read the log panel to learn that `/api/v2`
                // would have reported `target_ineligible` here.
                Err(RunControlError::TargetIneligible { change_id, .. }) => {
                    CommandFeedback::refused(format!(
                        "Manual merge-wait retry intent for '{}' was not accepted by scheduler state",
                        change_id
                    ))
                }
                Err(error) => CommandFeedback::refused(error.to_string()),
            }
            });
        }
    }

    Ok(())
}

/// Apply one settled command's operator-facing wording to this frontend.
pub fn apply_command_feedback(app: &mut AppState, feedback: CommandFeedback) {
    if let Some(warning) = feedback.warning {
        app.warning_message = Some(warning);
    }
    app.add_log(feedback.log);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openspec::{Change, ProposalMetadata};
    use crate::orchestration::operator_command::{
        ExecutionMarkStore, HookRunnerQueueHooks, OperatorCommandService,
    };
    use crate::orchestration::operator_coordinator::CoreMode;
    use crate::orchestration::run_control::testing::{RecordingScheduler, SchedulerCall};
    use crate::orchestration::run_control::RunControlService;
    use crate::orchestration::run_control::{ResolveReservations, StartEligibility};
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::types::WorktreeInfo;
    use crate::tui::types::{AppExecutionMode, StopMode};
    use std::path::{Path, PathBuf};
    use tokio::sync::RwLock;
    use tokio_util::sync::CancellationToken;

    use crate::worktree_ops::service::{
        MergeAttempt, WorktreeFacts, WorktreeOpError, WorktreeOpResult,
    };

    /// HEAD every stub-observed worktree reports.
    pub(super) const STUB_HEAD: &str = "stubhead00000000";

    /// Replay whatever [`set_delete_branch_test_failure`] scripted for a branch.
    fn scripted_branch_failure(branch: &str) -> WorktreeOpResult<()> {
        match DELETE_BRANCH_TEST_FAILURES
            .lock()
            .expect("delete branch test failures lock")
            .get(branch)
        {
            Some(message) => Err(WorktreeOpError::Internal(message.clone())),
            None => Ok(()),
        }
    }

    /// Backend the TUI command handlers are unit-tested against.
    ///
    /// Every worktree registered through [`set_delete_worktree_test_outcome`] is
    /// observable; `remove_worktree` replays that registered outcome and
    /// [`set_delete_worktree_test_dirty`] chooses what `observe` reports. No
    /// repository, process, or filesystem state is involved.
    pub(super) struct StubWorktreeBackend;

    #[async_trait::async_trait]
    impl WorktreeBackend for StubWorktreeBackend {
        async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
            Ok(DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .iter()
                .map(|(path, state)| {
                    let mut facts = WorktreeFacts::new(path.clone(), state.branch.clone());
                    facts.identity = format!("gitdir: {}/.git", path.display());
                    facts.head = STUB_HEAD.to_string();
                    facts.dirty = state.dirty;
                    facts.has_commits_ahead = state.has_commits_ahead;
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

        async fn teardown(&self, _path: &Path) -> WorktreeOpResult<()> {
            Ok(())
        }

        async fn remove_worktree(&self, path: &Path) -> WorktreeOpResult<()> {
            let removal = DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .remove(path)
                .map(|state| state.removal)
                .unwrap_or(DeleteWorktreeTestOutcome::Success);
            match removal {
                DeleteWorktreeTestOutcome::Success => Ok(()),
                DeleteWorktreeTestOutcome::Failure(message) => Err(WorktreeOpError::Internal(
                    format!("stubbed delete failure for {}: {}", path.display(), message),
                )),
            }
        }

        async fn branch_ref(&self, _branch: &str) -> WorktreeOpResult<Option<String>> {
            Ok(Some(STUB_HEAD.to_string()))
        }

        async fn delete_branch_if_merged(&self, branch: &str) -> WorktreeOpResult<()> {
            scripted_branch_failure(branch)
        }

        async fn delete_branch_at(&self, branch: &str, expected_oid: &str) -> WorktreeOpResult<()> {
            // The real backend compares inside the ref transaction; the stub
            // makes the same comparison explicit so a test handing over the
            // wrong OID sees the branch survive for the right reason.
            if expected_oid != STUB_HEAD {
                return Err(WorktreeOpError::Internal(format!(
                    "stubbed ref mismatch: expected {STUB_HEAD}, got {expected_oid}"
                )));
            }
            scripted_branch_failure(branch)
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
        /// The one application transaction both adapters submit to.
        pub(super) application: Arc<OperatorApplication>,
        /// The process lifecycle mode the transaction validates against.
        pub(super) core_mode: Arc<CoreMode>,
        /// The process-lifetime dispatch owner accepted outcomes travel through.
        pub(super) dispatcher: Arc<crate::events::EventDispatcher>,
        /// Frontends attached to that owner after construction.
        attached: Arc<AttachedSinks>,
        /// The revision authority attached to that owner after construction.
        revisions: Arc<AttachedRevisions>,
        pub(super) marks: Arc<ExecutionMarkStore>,
        /// The one parallel runtime store both adapters read and mutate.
        pub(super) parallel: Arc<crate::orchestration::operator_command::ParallelRuntime>,
        pub(super) resolves: Arc<ResolveReservations>,
        pub(super) config: OrchestratorConfig,
        pub(super) tx: mpsc::Sender<OrchestratorEvent>,
        pub(super) rx: mpsc::Receiver<OrchestratorEvent>,
        /// Authoritative deliveries the dispatch owner fans out to this frontend.
        frontend_rx: std::sync::Mutex<mpsc::Receiver<OrchestratorEvent>>,
        submissions_tx: mpsc::Sender<Submission>,
        submissions_rx: std::sync::Mutex<mpsc::Receiver<Submission>>,
        feedback_tx: mpsc::Sender<CommandFeedback>,
        feedback_rx: std::sync::Mutex<mpsc::Receiver<CommandFeedback>>,
        /// The one shared worktree service, built the same way production does.
        pub(super) worktree_service: Arc<WorktreeService>,
    }

    /// Frontends attached to the dispatch owner after it was built.
    ///
    /// A convergence test has to bind a `WebState` to the *same* boundary the
    /// TUI is on, and the `WebState` cannot exist before the shared mark store
    /// it reads. One indirection sink resolves that ordering without giving the
    /// two frontends two owners, which is the exact arrangement under test.
    #[derive(Default)]
    pub(super) struct AttachedSinks {
        sinks: std::sync::Mutex<Vec<Arc<dyn crate::events::EventSink>>>,
        /// How many authoritative dispatches this boundary has fanned out.
        ///
        /// Counted here rather than in a frontend because event *cardinality* is
        /// a property of the dispatch owner: a command that published its effect
        /// twice, or published nothing, is wrong regardless of what any
        /// individual sink then did with it.
        dispatches: std::sync::atomic::AtomicUsize,
    }

    impl AttachedSinks {
        fn snapshot(&self) -> Vec<Arc<dyn crate::events::EventSink>> {
            self.sinks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl crate::events::EventSink for AttachedSinks {
        async fn on_event(&self, _event: &crate::events::ExecutionEvent) {}

        async fn on_state_changed(&self, _state: &OrchestratorState) {}

        async fn on_dispatch(&self, dispatch: &crate::events::EventDispatch<'_>) {
            self.dispatches
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            for sink in self.snapshot() {
                sink.on_dispatch(dispatch).await;
            }
        }
    }

    /// A revision source bound after the dispatch owner already exists.
    ///
    /// The coordinator reads its outcome revisions through this, and a `WebState`
    /// is what answers — but the `WebState` needs the shared mark store, which is
    /// built alongside the coordinator. One indirection resolves that ordering
    /// without letting a test invent a second revision authority.
    #[derive(Default)]
    pub(super) struct AttachedRevisions {
        inner: std::sync::Mutex<Option<Arc<dyn crate::events::OutcomeRevisions>>>,
    }

    impl AttachedRevisions {
        fn inner(&self) -> Option<Arc<dyn crate::events::OutcomeRevisions>> {
            self.inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    impl crate::events::OutcomeRevisions for AttachedRevisions {
        fn revision_for_dispatch(&self, dispatch_id: u64) -> Option<u64> {
            self.inner()
                .and_then(|inner| inner.revision_for_dispatch(dispatch_id))
        }

        fn current_revision(&self) -> u64 {
            self.inner().map_or(0, |inner| inner.current_revision())
        }
    }

    impl AdapterHarness {
        pub(super) fn new(change_ids: &[&str]) -> Self {
            Self::with_config(change_ids, create_test_config())
        }

        pub(super) fn with_config(change_ids: &[&str], config: OrchestratorConfig) -> Self {
            let state = Arc::new(RwLock::new(OrchestratorState::new(
                change_ids.iter().map(|id| id.to_string()).collect(),
                10,
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
            // The same three-part wiring production builds: one core mode, one
            // dispatch owner over the shared reducer, and one coordinator. A test
            // that stubbed any of them would stop proving the thing these tests
            // exist for.
            let core_mode = Arc::new(CoreMode::new());
            let (frontend_tx, frontend_rx) = mpsc::channel(256);
            let attached = Arc::new(AttachedSinks::default());
            let dispatcher = Arc::new(
                crate::events::EventDispatcher::new(
                    state.clone(),
                    vec![
                        Arc::new(crate::tui::events::TuiEventSink::new(frontend_tx)),
                        attached.clone(),
                    ],
                )
                .with_core_mode(Some(core_mode.clone())),
            );
            let revisions = Arc::new(AttachedRevisions::default());
            let application = Arc::new(
                OperatorApplication::new(
                    core_mode.clone(),
                    run_control.clone(),
                    dispatcher.clone(),
                )
                .with_revisions(Some(
                    revisions.clone() as Arc<dyn crate::events::OutcomeRevisions>
                )),
            );
            let (submissions_tx, submissions_rx) = mpsc::channel(256);
            let (feedback_tx, feedback_rx) = mpsc::channel(256);
            let worktree_service = build_worktree_service(Path::new("."), &config, &tx);
            Self {
                state,
                queue,
                scheduler,
                run_control,
                application,
                core_mode,
                dispatcher,
                attached,
                revisions,
                marks,
                parallel,
                resolves,
                config,
                tx,
                rx,
                frontend_rx: std::sync::Mutex::new(frontend_rx),
                submissions_tx,
                submissions_rx: std::sync::Mutex::new(submissions_rx),
                feedback_tx,
                feedback_rx: std::sync::Mutex::new(feedback_rx),
                worktree_service,
            }
        }

        /// Attach another frontend to the shared dispatch owner.
        pub(super) fn attach(&self, sink: Arc<dyn crate::events::EventSink>) {
            self.attached
                .sinks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(sink);
        }

        /// Bind the revision authority accepted outcomes are recorded against.
        pub(super) fn attach_revisions(&self, revisions: Arc<dyn crate::events::OutcomeRevisions>) {
            *self
                .revisions
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(revisions);
        }

        /// How many authoritative dispatches this boundary has fanned out.
        pub(super) fn dispatch_count(&self) -> usize {
            self.attached
                .dispatches
                .load(std::sync::atomic::Ordering::SeqCst)
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
            app.set_parallel_runtime(self.parallel.clone());
            app
        }

        pub(super) fn context<'a>(&'a self, app: &'a mut AppState) -> TuiCommandContext<'a> {
            TuiCommandContext {
                app,
                tx: &self.tx,
                application: &self.application,
                submissions: &self.submissions_tx,
                feedback: &self.feedback_tx,
                worktree_service: &self.worktree_service,
            }
        }

        /// Run one command through the TUI adapter and settle the frame it produced.
        ///
        /// This is deliberately a whole runner frame rather than just the handler:
        /// submission, coordinator execution, authoritative dispatch delivery, and
        /// mode adoption are what the TUI actually shows an operator, and a test
        /// that stopped after the handler would be asserting on a frontend state
        /// that no longer exists by the time anything is rendered.
        pub(super) async fn run(&self, app: &mut AppState, command: TuiCommand) {
            // The arranged frontend mode *is* the arranged process mode: the two
            // are one value in production, so a test that arranges only one of
            // them would be exercising a state the process cannot be in.
            self.core_mode.set(app.operator_mode());

            let mut ctx = self.context(app);
            handle_tui_command(command, &mut ctx, &self.state)
                .await
                .expect("tui command should succeed");
            self.settle(app).await;
        }

        /// Wait for one settlement that a command handled on its own task.
        ///
        /// `stop_and_dequeue` is the only such command: it must not occupy the
        /// ordered submission queue, so its result arrives asynchronously and a
        /// test frame has to join it before asserting.
        pub(super) async fn await_feedback(&self, app: &mut AppState, within: std::time::Duration) {
            let deadline = std::time::Instant::now() + within;
            while std::time::Instant::now() < deadline {
                let settled = self
                    .feedback_rx
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .try_recv()
                    .ok();
                if let Some(settled) = settled {
                    apply_command_feedback(app, settled);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
            self.deliver(app).await;
        }

        /// Drain the submission queue, the dispatch deliveries, and the feedback.
        pub(super) async fn settle(&self, app: &mut AppState) {
            loop {
                let next = self
                    .submissions_rx
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .try_recv()
                    .ok();
                let Some(submission) = next else { break };
                let settled = submission.run(&self.application).await;
                let _ = self.feedback_tx.send(settled).await;
            }
            self.deliver(app).await;
        }

        /// Apply everything a runner frame would apply after commands were handled.
        pub(super) async fn deliver(&self, app: &mut AppState) {
            loop {
                let next = self
                    .frontend_rx
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .try_recv()
                    .ok();
                let Some(event) = next else { break };
                crate::tui::runner::sync_reducer_display_caches(app, &self.state, &event).await;
                app.handle_orchestrator_event(event);
            }
            loop {
                let next = self
                    .feedback_rx
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .try_recv()
                    .ok();
                let Some(settled) = next else { break };
                apply_command_feedback(app, settled);
            }
            app.adopt_core_mode(self.core_mode.get());
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
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-a".to_string(),
                    false,
                )),
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
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-a".to_string(),
                    false,
                )),
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

    #[tokio::test]
    async fn test_delete_worktree_command_refuses_a_stale_confirmed_branch() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-replaced");
        // The stub observes every registered path on `feature-a`, so confirming a
        // different branch is exactly the "path was re-occupied since the modal"
        // case: the adapter must forward the confirmed identity and the service
        // must refuse before the backend is asked to remove anything.
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-stale".to_string(),
                    false,
                )),
            )
            .await;

        assert!(!app.is_worktree_deleting(&path));
        assert!(
            app.logs
                .iter()
                .any(|entry| entry.message.contains("Worktree delete failed")
                    && entry.message.contains("feature-stale")),
            "the identity refusal must be reported verbatim: {:?}",
            app.logs.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
        assert!(
            DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "a refused delete must never reach the backend remove"
        );
    }

    // ── Explicit dirty discard ──────────────────────────────────────────────

    #[tokio::test]
    async fn tui_dirty_worktree_delete_escalates_the_services_refusal_into_a_confirmation() {
        for skip_teardown in [false, true] {
            let harness = AdapterHarness::new(&["change-a"]);
            let mut app = harness.app(&["change-a"]);
            let path = PathBuf::from("/tmp/worktree-dirty");
            set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
            set_delete_worktree_test_dirty(
                path.clone(),
                crate::worktree_ops::service::DirtyState::Dirty,
            );
            app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
            app.mark_worktree_deleting(path.clone());

            harness
                .run(
                    &mut app,
                    TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                        path.clone(),
                        "feature-a".to_string(),
                        skip_teardown,
                    )),
                )
                .await;

            // The refusal is an escalation, not a failure: no warning popup, and
            // the confirmation carries the *service's* observation.
            assert!(
                app.warning_popup.is_none(),
                "a dirty refusal must not be reported as a delete failure"
            );
            assert_eq!(
                app.modal,
                Some(crate::tui::types::ModalState::ConfirmDirtyDiscard {
                    path: path.clone(),
                    identity: format!("gitdir: {}/.git", path.display()),
                    branch: "feature-a".to_string(),
                    head: STUB_HEAD.to_string(),
                    skip_teardown,
                })
            );
            assert!(
                DELETE_WORKTREE_TEST_OUTCOMES
                    .lock()
                    .expect("delete worktree test outcomes lock")
                    .contains_key(&path),
                "a refused delete must never reach the backend removal"
            );
            assert!(!app.is_worktree_deleting(&path));
        }
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_removes_the_worktree_once_discard_is_granted() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-dirty-confirmed");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        set_delete_worktree_test_dirty(
            path.clone(),
            crate::worktree_ops::service::DirtyState::Dirty,
        );
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent {
                    path: path.clone(),
                    branch: "feature-a".to_string(),
                    identity: Some(format!("gitdir: {}/.git", path.display())),
                    head: Some(STUB_HEAD.to_string()),
                    skip_teardown: false,
                    allow_known_dirty: true,
                    allow_commits_ahead: false,
                }),
            )
            .await;

        assert!(
            app.modal.is_none(),
            "a granted discard must not re-escalate"
        );
        assert!(!app.is_worktree_deleting(&path));
        assert!(
            !DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "the removal must actually have reached the backend"
        );
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Deleted worktree")));
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_reports_non_dirty_refusals_as_failures() {
        // Only `Dirty` escalates. Everything else is a refusal the operator has
        // to read, because no keypress can waive it.
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-unknown-dirty");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        set_delete_worktree_test_dirty(
            path.clone(),
            crate::worktree_ops::service::DirtyState::Unknown,
        );
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-a".to_string(),
                    false,
                )),
            )
            .await;

        assert!(
            app.modal.is_none(),
            "an unobservable dirty state must never offer a discard confirmation"
        );
        assert!(app.warning_popup.is_some());
        assert!(app
            .logs
            .iter()
            .any(|entry| entry.message.contains("Worktree delete failed")));
        assert!(
            DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "a refused delete must never reach the backend removal"
        );
    }

    // ── Explicit ahead discard ──────────────────────────────────────────────

    #[tokio::test]
    async fn tui_ahead_worktree_delete_escalates_the_services_refusal_into_a_confirmation() {
        for (name, dirty, expect_dirty) in [
            (
                "clean",
                crate::worktree_ops::service::DirtyState::Clean,
                false,
            ),
            (
                "dirty",
                crate::worktree_ops::service::DirtyState::Dirty,
                true,
            ),
        ] {
            for skip_teardown in [false, true] {
                let harness = AdapterHarness::new(&["change-a"]);
                let mut app = harness.app(&["change-a"]);
                let path = PathBuf::from(format!("/tmp/worktree-ahead-{name}-{skip_teardown}"));
                set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
                set_delete_worktree_test_dirty(path.clone(), dirty);
                set_delete_worktree_test_ahead(
                    path.clone(),
                    crate::worktree_ops::service::SafetyFact::Yes,
                );
                app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
                app.mark_worktree_deleting(path.clone());

                harness
                    .run(
                        &mut app,
                        TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                            path.clone(),
                            "feature-a".to_string(),
                            skip_teardown,
                        )),
                    )
                    .await;

                assert!(
                    app.warning_popup.is_none(),
                    "{name}: an ahead refusal must not be reported as a delete failure"
                );
                assert_eq!(
                    app.modal,
                    Some(crate::tui::types::ModalState::ConfirmAheadDiscard {
                        path: path.clone(),
                        identity: format!("gitdir: {}/.git", path.display()),
                        branch: "feature-a".to_string(),
                        head: STUB_HEAD.to_string(),
                        dirty: expect_dirty,
                        skip_teardown,
                    }),
                    "{name}: the confirmation must carry the service's own observation"
                );
                assert!(
                    DELETE_WORKTREE_TEST_OUTCOMES
                        .lock()
                        .expect("delete worktree test outcomes lock")
                        .contains_key(&path),
                    "{name}: a refused delete must never reach the backend removal"
                );
                assert!(!app.is_worktree_deleting(&path));
            }
        }
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_removes_worktree_and_branch_once_discard_is_granted() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-ahead-confirmed");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        set_delete_worktree_test_dirty(
            path.clone(),
            crate::worktree_ops::service::DirtyState::Dirty,
        );
        set_delete_worktree_test_ahead(path.clone(), crate::worktree_ops::service::SafetyFact::Yes);
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent {
                    path: path.clone(),
                    branch: "feature-a".to_string(),
                    identity: Some(format!("gitdir: {}/.git", path.display())),
                    head: Some(STUB_HEAD.to_string()),
                    skip_teardown: false,
                    // Both permissions, from the one confirmation that named
                    // both losses.
                    allow_known_dirty: true,
                    allow_commits_ahead: true,
                }),
            )
            .await;

        assert!(
            app.modal.is_none(),
            "a granted discard must not re-escalate"
        );
        assert!(!app.is_worktree_deleting(&path));
        assert!(
            !DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "the removal must actually have reached the backend"
        );
        assert!(
            app.logs
                .iter()
                .any(|entry| entry.message.contains("unmerged commits were deleted")),
            "the branch deletion must be reported: {:?}",
            app.logs.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_reports_a_retained_branch_as_partial_success() {
        // The branch ref can still move in the window between the pre-removal
        // confirmation and the compare-and-delete. The worktree is gone by then
        // and is not reconstructed, so the operator has to be told plainly that
        // a branch they still own survived.
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-ahead-ref-drift");
        let branch = "feature-ahead-drift";
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        set_delete_worktree_test_branch(path.clone(), branch);
        set_delete_worktree_test_ahead(path.clone(), crate::worktree_ops::service::SafetyFact::Yes);
        set_delete_branch_test_failure(branch, "update-ref: reference already exists");
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent {
                    path: path.clone(),
                    branch: branch.to_string(),
                    identity: Some(format!("gitdir: {}/.git", path.display())),
                    head: Some(STUB_HEAD.to_string()),
                    skip_teardown: false,
                    allow_known_dirty: false,
                    allow_commits_ahead: true,
                }),
            )
            .await;

        // Removal and branch deletion are distinct outcomes: the worktree is
        // gone even though the branch survived.
        assert!(
            !DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "the worktree removal itself must still have happened"
        );
        assert!(
            app.warning_popup.is_none(),
            "a partial success is not a failed delete"
        );
        assert!(
            app.logs
                .iter()
                .any(|entry| entry.message.contains("Partially deleted worktree")
                    && entry.message.contains("was retained")),
            "the retained branch must be reported, not folded into a success: {:?}",
            app.logs.iter().map(|e| &e.message).collect::<Vec<_>>()
        );
        assert!(!app.is_worktree_deleting(&path));
    }

    #[tokio::test]
    async fn tui_ahead_worktree_delete_reports_unwaivable_refusals_as_failures() {
        // An unobservable ahead state carries no confirmable evidence, so it is
        // a refusal to read rather than a confirmation to press through.
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-unknown-ahead");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        set_delete_worktree_test_ahead(
            path.clone(),
            crate::worktree_ops::service::SafetyFact::Unknown,
        );
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-a".to_string(),
                    false,
                )),
            )
            .await;

        assert!(
            app.modal.is_none(),
            "an unobservable ahead state must never offer a discard confirmation"
        );
        assert!(app.warning_popup.is_some());
        assert!(
            DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "a refused delete must never reach the backend removal"
        );
    }

    #[tokio::test]
    async fn tui_dirty_worktree_delete_shares_one_mutation_guard_with_the_remote_port() {
        // Both frontends are handed the same `Arc<WorktreeService>`, so an
        // in-flight mutation is visible to the other one as `root_busy` instead
        // of racing through a second guard over the same repository.
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        let path = PathBuf::from("/tmp/worktree-guarded");
        set_delete_worktree_test_outcome(path.clone(), DeleteWorktreeTestOutcome::Success);
        app.worktrees = vec![create_test_worktree(path.to_str().unwrap())];
        app.mark_worktree_deleting(path.clone());

        let held = harness
            .worktree_service
            .observe()
            .await
            .expect("the stub observes");
        assert!(!held.is_empty());

        // Hold the shared guard exactly as a concurrent `/api/v2` delete would.
        let guard = harness
            .worktree_service
            .acquire_root_for_test()
            .expect("the first caller takes the shared guard");

        harness
            .run(
                &mut app,
                TuiCommand::DeleteWorktree(DeleteIntent::ordinary(
                    path.clone(),
                    "feature-a".to_string(),
                    false,
                )),
            )
            .await;
        drop(guard);

        assert!(
            DELETE_WORKTREE_TEST_OUTCOMES
                .lock()
                .expect("delete worktree test outcomes lock")
                .contains_key(&path),
            "a keypress must not mutate the repository while another operation holds the guard"
        );
        assert!(app.logs.iter().any(|entry| entry
            .message
            .contains("another worktree operation is already mutating this repository")));
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
        assert_eq!(app.changes[0].display_status_cache, "queued");
        // Queue intent is not an execution mark: only the shared store carries one.
        harness.marks.set("change-a", true);
        app.sync_execution_marks_from_store();
        assert!(app.changes[0].selected);

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

    /// The TUI adapter's queue commands are the same explicit-intent boundary
    /// the scheduler reads. Queue add makes a change ordinarily eligible; queue
    /// remove and stop-and-dequeue revoke it until an explicit requeue.
    #[tokio::test]
    async fn tui_queue_commands_drive_the_scheduler_eligibility_boundary() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);

        assert!(
            !harness
                .state
                .read()
                .await
                .is_ordinary_queue_eligible("change-a"),
            "a visible but unqueued change is not ordinarily eligible"
        );

        harness
            .run(&mut app, TuiCommand::AddToQueue("change-a".to_string()))
            .await;
        assert!(harness
            .state
            .read()
            .await
            .is_ordinary_queue_eligible("change-a"));

        harness
            .run(
                &mut app,
                TuiCommand::RemoveFromQueue("change-a".to_string()),
            )
            .await;
        assert!(
            !harness
                .state
                .read()
                .await
                .is_ordinary_queue_eligible("change-a"),
            "queue removal revokes scheduler eligibility immediately"
        );
        assert!(
            !harness.queue.contains("change-a").await,
            "the dynamic wake-up hint is withdrawn with the intent"
        );

        harness
            .run(&mut app, TuiCommand::AddToQueue("change-a".to_string()))
            .await;
        assert!(
            harness
                .state
                .read()
                .await
                .is_ordinary_queue_eligible("change-a"),
            "explicit requeue restores scheduler eligibility"
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

    #[tokio::test]
    async fn force_kill_confirmation_reaches_the_shared_service_and_terminates_the_target() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("applying");
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
            worker_queue.unregister_kill_token("change-a").await;
        });

        // The confirmation is advisory; the command it emits is what carries the
        // intent into the shared service.
        assert!(app.request_force_kill_confirmation());
        let command = app.confirm_force_kill().expect("confirmation dispatches");
        assert!(app.modal.is_none());
        harness.run(&mut app, command).await;

        wait_for_status(&harness.state, "change-a", "not queued").await;
        worker.await.expect("worker task");
    }

    #[tokio::test]
    async fn force_kill_confirmation_missing_termination_evidence_leaves_the_target_active() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("applying");
        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::ApplyStarted {
                change_id: "change-a".to_string(),
                command: "apply".to_string(),
            },
        );

        assert!(app.request_force_kill_confirmation());
        let command = app.confirm_force_kill().expect("confirmation dispatches");
        harness.run(&mut app, command).await;

        // No cancellation handle exists, so termination cannot be proven and the
        // shared service refuses to dequeue.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(
            harness.status("change-a").await,
            "applying",
            "an unprovable termination must not mutate authoritative state"
        );
    }

    #[tokio::test]
    async fn force_kill_confirmation_refuses_a_stale_target_before_reaching_the_service() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        app.execution_mode = AppExecutionMode::Running;
        app.changes[0].set_display_status_cache("applying");
        assert!(app.request_force_kill_confirmation());

        // The target settles between display and confirmation input.
        app.changes[0].set_display_status_cache("archived");

        assert!(
            app.confirm_force_kill().is_none(),
            "a stale confirmation must not emit a stop-and-dequeue command"
        );
        assert!(app.modal.is_none());
        assert_eq!(harness.status("change-a").await, "not queued");
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
        assert_eq!(app.execution_mode, AppExecutionMode::Running);
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
        assert_eq!(app.execution_mode, AppExecutionMode::Select);
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
        app.execution_mode = AppExecutionMode::Error;
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
        app.execution_mode = AppExecutionMode::Error;
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

    /// Replay the startup refresh: no reducer history, only a workspace scan
    /// reporting the change as archived but not yet merged into base.
    async fn startup_refresh_merge_wait(harness: &AdapterHarness, change_id: &str) {
        use std::collections::{HashMap, HashSet};

        harness.state.write().await.apply_execution_event(
            &crate::events::ExecutionEvent::ChangesRefreshed {
                changes: vec![create_test_change(change_id)],
                rejected_changes: Vec::new(),
                committed_change_ids: HashSet::new(),
                uncommitted_file_change_ids: HashSet::new(),
                worktree_change_ids: HashSet::new(),
                worktree_paths: HashMap::new(),
                worktree_not_ahead_ids: HashSet::new(),
                merge_wait_ids: HashSet::from([change_id.to_string()]),
            },
        );
    }

    #[tokio::test]
    async fn resolve_merge_from_startup_refreshed_row_reaches_resolve_pending() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);

        // Startup order: the reducer reconciles the refresh, then the row syncs
        // from the reducer-owned status, then the operator presses `M`.
        startup_refresh_merge_wait(&harness, "change-a").await;
        let reducer_status = harness.status("change-a").await;
        assert_eq!(reducer_status, "merge wait");
        app.apply_display_statuses_from_reducer(&std::collections::HashMap::from([(
            "change-a".to_string(),
            "merge wait",
        )]));
        assert_eq!(app.changes[0].display_status_cache, "merge wait");

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;

        assert_eq!(harness.status("change-a").await, "resolve pending");
        assert_eq!(app.changes[0].display_status_cache, "resolve pending");
        assert_eq!(
            app.warning_message, None,
            "a reconstructed merge wait must not be refused by scheduler state"
        );
        assert_eq!(log_count(&app, "was not accepted by scheduler state"), 0);
        assert_eq!(log_count(&app, "started scheduler for manual resolve"), 1);
        assert_eq!(
            harness.state.read().await.resolve_wait_change_ids(),
            vec!["change-a".to_string()],
            "the visible resolve pending must be scheduler-consumable retry membership"
        );
        assert_eq!(
            harness.scheduler.started_targets(),
            vec![Vec::<String>::new()]
        );
        assert!(harness.resolves.is_active());
        assert_eq!(app.execution_mode, AppExecutionMode::Running);
    }

    #[tokio::test]
    async fn resolve_merge_without_startup_workspace_evidence_is_refused() {
        let harness = AdapterHarness::new(&["change-a"]);
        let mut app = harness.app(&["change-a"]);
        assert_eq!(harness.status("change-a").await, "not queued");

        harness
            .run(&mut app, TuiCommand::ResolveMerge("change-a".to_string()))
            .await;

        assert_eq!(log_count(&app, "was not accepted by scheduler state"), 1);
        assert!(app.warning_message.is_some());
        assert!(harness.scheduler.calls().is_empty());
        assert!(!harness.resolves.is_active());
        assert!(harness
            .state
            .read()
            .await
            .resolve_wait_change_ids()
            .is_empty());
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
        assert_eq!(app.execution_mode, AppExecutionMode::Running);
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
        app.execution_mode = AppExecutionMode::Running;

        harness.run(&mut app, TuiCommand::Stop).await;

        assert_eq!(app.execution_mode, AppExecutionMode::Stopping);
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
        app.execution_mode = AppExecutionMode::Stopping;
        app.stop_mode = StopMode::GracefulPending;

        harness.run(&mut app, TuiCommand::CancelStop).await;

        assert_eq!(app.execution_mode, AppExecutionMode::Running);
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
        app.execution_mode = AppExecutionMode::Stopping;
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
            app.execution_mode,
            AppExecutionMode::Stopping,
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
        assert_eq!(app.execution_mode, AppExecutionMode::Stopped);
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
            app.execution_mode,
            AppExecutionMode::Stopping,
            "terminal stop waits for the base-lane operation to reach its boundary"
        );
        assert_eq!(log_count(&app, "Processing stopped"), 0);
    }

    #[tokio::test]
    async fn idle_parallel_stop_without_live_scheduler_applies_terminal_stop() {
        let harness = AdapterHarness::new(&["change-a"]);
        let app = run_force_stop(&harness, activity(0, true), false).await;

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Stopped,
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
        app.execution_mode = AppExecutionMode::Select;

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
    use crate::orchestration::operator_coordinator::{CoreMode, OperatorApplication};
    use crate::orchestration::run_control::{
        ResolveReservations, RunControlService, RunSchedulerPort, StartEligibility,
    };
    use crate::orchestration::state::OrchestratorState;
    use crate::tui::run_supervisor::TuiRunSupervisor;
    use crate::tui::types::AppExecutionMode;
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

        let mut state = OrchestratorState::new(vec!["alpha".to_string()], 0);
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
        // The process-lifetime dispatch owner, exactly as production builds it:
        // the supervisor publishes a spawned run's events through it, and the
        // coordinator publishes accepted command outcomes through the same one.
        let core_mode = Arc::new(CoreMode::new());
        let dispatcher = Arc::new(
            crate::events::EventDispatcher::new(
                shared_state.clone(),
                vec![Arc::new(crate::tui::events::TuiEventSink::new(tx.clone()))],
            )
            .with_core_mode(Some(core_mode.clone())),
        );
        let supervisor = Arc::new(TuiRunSupervisor::new(
            root.clone(),
            config.clone(),
            dispatcher.clone(),
            queue.clone(),
            shared_state.clone(),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            PostArchiveAction::MergeToBase,
            Some(upstream_runtime()),
            Arc::new(AtomicBool::new(false)),
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
        let application = Arc::new(OperatorApplication::new(
            core_mode.clone(),
            run_control.clone(),
            dispatcher.clone(),
        ));
        let (submissions_tx, mut submissions_rx) = mpsc::channel::<Submission>(16);
        let (feedback_tx, _feedback_rx) = mpsc::channel::<CommandFeedback>(16);

        let mut app = AppState::new(vec![create_test_change("alpha")]);
        app.set_shared_state(shared_state.clone());
        app.apply_display_statuses_from_reducer(&shared_state.read().await.all_display_statuses());
        assert_eq!(app.changes[0].display_status_cache, "error");
        app.execution_mode = AppExecutionMode::Error;
        core_mode.set(app.operator_mode());
        app.changes[0].selected = true;
        let worktree_service = build_worktree_service(&root, &config, &tx);

        {
            let mut ctx = TuiCommandContext {
                app: &mut app,
                tx: &tx,
                application: &application,
                submissions: &submissions_tx,
                feedback: &feedback_tx,
                worktree_service: &worktree_service,
            };
            // A remote `start` carries no IDs either; Error mode turns both into
            // the same explicit retry.
            handle_start_processing_command(Vec::new(), &mut ctx).await;
        }
        // The runner's submission worker, inline: the command was queued off the
        // event loop, so the run only really starts once that queue is drained.
        let queued = submissions_rx
            .try_recv()
            .expect("start must be queued for the shared transaction");
        queued.run(&application).await;
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
        let (handle, cancel, _scope) = supervisor.take_run();
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
}
