//! Delegation from the v2 command envelope to the shared operator command service.
//!
//! This module is intentionally thin. It maps a typed [`CommandSpec`] onto the
//! existing shared behavior and maps the shared error vocabulary onto v2 error
//! codes — nothing more. There is no lifecycle matrix, no queue logic, and no
//! retry routing here, because duplicating any of it would let the remote
//! frontend drift away from the TUI.

use std::sync::Arc;

use async_trait::async_trait;

use crate::orchestration::operator_command::{
    MarkExclusion, NoOpReason, OperatorCommandError, OperatorOutcome, StopSettlement,
};
use crate::orchestration::operator_coordinator::{
    ApplicationOutcome, ApplicationResult, OperatorApplication, OperatorIntent,
};
use crate::orchestration::run_control::{
    ExcludedTarget, ResolveReservation, RunControlError, RunControlOutcome, RunNoOpReason,
    SchedulerEffect,
};
use crate::web::state::WebState;

use super::dto::{
    ApplyCommitEvidence, CommandResult, CommandSpec, ErrorCode, ExecutionPhase as DtoPhase,
};

/// What a delegated command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionSummary {
    /// True when the command produced a real effect.
    pub changed: bool,
    /// Sanitized operator-facing detail.
    pub detail: Option<String>,
    /// The revision this command's own outcome dispatch produced.
    ///
    /// `None` means "no outcome was dispatched", which the endpoint settles as
    /// the unchanged admitted revision. It is never a sampled current revision:
    /// sampling is exactly how unrelated later scheduler progress used to become
    /// a command's recorded `result_revision`.
    pub result_revision: Option<u64>,
    /// Typed settlement evidence, for the commands that produce it.
    ///
    /// Travels with the outcome rather than being read at settlement time, for
    /// the same reason `result_revision` does: by the time the record settles,
    /// the system may already describe a different instant.
    pub result: Option<CommandResult>,
}

impl ExecutionSummary {
    /// A command that produced an effect.
    pub fn changed(detail: impl Into<String>) -> Self {
        Self {
            changed: true,
            detail: Some(detail.into()),
            result_revision: None,
            result: None,
        }
    }

    /// A command that was valid but changed nothing.
    pub fn no_op(detail: impl Into<String>) -> Self {
        Self {
            changed: false,
            detail: Some(detail.into()),
            result_revision: None,
            result: None,
        }
    }

    /// Bind the revision this command's outcome dispatch produced.
    pub fn at_revision(mut self, revision: Option<u64>) -> Self {
        self.result_revision = revision;
        self
    }

    /// Bind the typed settlement evidence this command fixed.
    pub fn with_result(mut self, result: CommandResult) -> Self {
        self.result = Some(result);
        self
    }
}

/// A typed refusal from the shared behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFailure {
    /// Stable v2 error code.
    pub error_code: ErrorCode,
    /// Sanitized message.
    pub message: String,
    /// The revision to settle this refusal at.
    ///
    /// `None` for an ordinary failure, which has no effect and therefore settles
    /// at the unchanged admitted revision. A two-phase refusal after the
    /// termination wait carries the explicit unchanged revision observed under
    /// the reacquired settlement boundary, because unrelated commands may have
    /// advanced projection while it waited.
    pub result_revision: Option<u64>,
}

impl CommandFailure {
    /// Build a failure.
    pub fn new(error_code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error_code,
            message: message.into(),
            result_revision: None,
        }
    }

    /// Bind the revision this refusal settles at.
    pub fn at_revision(mut self, revision: Option<u64>) -> Self {
        self.result_revision = revision;
        self
    }
}

/// A command whose second phase settles after the endpoint released the gate.
pub type PendingCommand = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ExecutionSummary, CommandFailure>> + Send>,
>;

/// What admitting one command decided about how it will be executed.
pub enum Applied {
    /// Nothing special. The caller executes it, still holding the returned gate.
    ///
    /// Execution runs on its own task so a slow command reports through its
    /// record instead of pinning the connection; the gate travels with it, so
    /// the command is still serialized through settlement.
    Ordinary(Option<GateGuard>),
    /// Phase one already ran under the gate and released it. The continuation
    /// waits for confirmed termination *outside* the gate, reacquires it,
    /// revalidates, and settles.
    ///
    /// Returning this instead of blocking is what keeps force stop, unrelated
    /// operator commands, event fan-out, and TUI rendering live while a
    /// stop-and-dequeue is pending.
    Pending(PendingCommand),
    /// Phase one refused before any effect. Settle the record immediately.
    Settled(Result<ExecutionSummary, CommandFailure>),
}

/// Exclusive ownership of the application gate, handed from the endpoint to the
/// executor.
///
/// It is *moved*, not borrowed, because the executor decides when it is
/// released: an ordinary command holds it through settlement, while a two-phase
/// command must drop it before waiting for confirmed termination. The gate is
/// not reentrant, so a caller that kept its own copy and then re-entered the
/// coordinator would deadlock.
pub type GateGuard = tokio::sync::OwnedMutexGuard<()>;

/// The port the command endpoint calls once a command has been admitted.
#[async_trait]
pub trait RemoteControlExecutor: Send + Sync {
    /// Execute an admitted command. Called at most once per command record.
    ///
    /// Acquires the application gate itself, so it is the entry point for a
    /// caller that holds none.
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure>;

    /// Decide how an admitted command will be executed, under the caller's gate.
    ///
    /// This is *not* where the command runs. It exists so a command that must
    /// issue cancellation inside the gate and then wait outside it can do the
    /// first half here; everything else hands the gate straight back.
    async fn begin(&self, _command: &CommandSpec, gate: Option<GateGuard>) -> Applied {
        Applied::Ordinary(gate)
    }

    /// Execute under a gate the caller already holds.
    ///
    /// The default releases the guard first, which is right for a port with no
    /// coordinator behind it: there is nothing to serialize, and holding a
    /// process-wide lock across an unrelated implementation would only make one
    /// slow port block every command.
    async fn execute_held(
        &self,
        command: &CommandSpec,
        gate: Option<GateGuard>,
    ) -> Result<ExecutionSummary, CommandFailure> {
        drop(gate);
        self.execute(command).await
    }
}

/// Map an operator-service refusal onto a v2 error code.
///
/// The distinction that matters to a client is *who* has to change: a
/// `lifecycle_conflict` needs the run to move on, a `target_ineligible` needs a
/// different target.
pub fn map_operator_error(error: &OperatorCommandError) -> CommandFailure {
    match error {
        // The active run owns this target's exhausted Apply ceiling. It is a
        // target problem, not a mode problem: unrelated targets in the same run
        // are still retryable, and this one becomes retryable again once its
        // owning boundary closes.
        OperatorCommandError::MissingCancellationHandle { .. }
        | OperatorCommandError::RetryUnsupported { .. }
        | OperatorCommandError::ApplyIterationLimitActive { .. } => {
            CommandFailure::new(ErrorCode::TargetIneligible, error.to_string())
        }
        // Termination did not confirm: the change is still occupying the root.
        OperatorCommandError::TerminationTimeout { .. } => {
            CommandFailure::new(ErrorCode::RootBusy, error.to_string())
        }
        OperatorCommandError::CancellationFailed { .. } => {
            CommandFailure::new(ErrorCode::InternalError, error.to_string())
        }
    }
}

/// Map an operator-service success onto a v2 execution summary.
pub fn summarize_outcome(outcome: &OperatorOutcome) -> ExecutionSummary {
    match outcome {
        OperatorOutcome::MarkSet { change_id, marked } => {
            ExecutionSummary::changed(format!("execution mark for '{change_id}' set to {marked}"))
        }
        OperatorOutcome::Queue(queue) => {
            if queue.reducer_changed || queue.dynamic_queue_mutated {
                ExecutionSummary::changed(format!(
                    "queue intent for '{}' is now '{}'",
                    queue.change_id, queue.display_status
                ))
            } else {
                ExecutionSummary::no_op(format!(
                    "queue intent for '{}' already '{}'",
                    queue.change_id, queue.display_status
                ))
            }
        }
        OperatorOutcome::Dequeued {
            change_id,
            settlement,
        } => ExecutionSummary::changed(settlement.describe(change_id))
            .with_result(stop_result(settlement)),
        OperatorOutcome::Retry(plan) => {
            if plan.is_empty() {
                ExecutionSummary::no_op("no change carried retryable evidence")
            } else {
                ExecutionSummary::changed(format!("retry accepted for {:?}", plan.change_ids))
            }
        }
        OperatorOutcome::BulkMarks {
            marked,
            changed,
            excluded,
        } => {
            let action = if *marked { "marked" } else { "unmarked" };
            let mut detail = format!("{} change(s) {action}: {changed:?}", changed.len());
            if !excluded.is_empty() {
                detail.push_str(&format!(
                    ", {} excluded ({})",
                    excluded.len(),
                    summarize_exclusions(excluded)
                ));
            }
            ExecutionSummary::changed(detail)
        }
        OperatorOutcome::NoOp { change_id, reason } => {
            let why = match reason {
                NoOpReason::MarkUnchanged => "execution mark already had the requested value",
                // Stable terminal-target reason: the row is archived, merged,
                // pushed, or rejected, so next-run intent has no meaning for it.
                NoOpReason::TerminalMarkTarget => {
                    "the target is terminal and carries no next-run intent"
                }
                // Stable archive-complete reason: the reducer recorded the
                // archive, so the row's remaining post-archive work is not a
                // next-run target even though its display status is still live.
                NoOpReason::ArchiveCompleteMarkTarget => {
                    "the target has completed archive and carries no next-run intent"
                }
                NoOpReason::ReducerRejected => "the reducer produced no state change",
                NoOpReason::BulkMarksUnchanged => {
                    "every eligible change already carried the derived mark"
                }
                NoOpReason::NoEligibleMarkTarget => {
                    "no change is eligible for a bulk execution-mark mutation"
                }
            };
            if change_id.is_empty() {
                ExecutionSummary::no_op(why)
            } else {
                ExecutionSummary::no_op(format!("'{change_id}': {why}"))
            }
        }
    }
}

/// The typed result a settled successful stop-and-dequeue carries.
///
/// `effects_rolled_back` is a literal `false` rather than a computed value: a
/// dequeue cancels and removes, and there is no code path in which it could undo
/// a completed worktree effect. Publishing it explicitly is what stops a client
/// from assuming the opposite.
pub fn stop_result(settlement: &StopSettlement) -> CommandResult {
    CommandResult::StopAndDequeue {
        cancelled_phase: DtoPhase::from_shared(settlement.cancelled_phase),
        last_completed_phase: settlement.last_completed_phase.map(DtoPhase::from_shared),
        apply_commit: ApplyCommitEvidence {
            present: settlement.apply_commit_present,
            oid: settlement.apply_commit_oid.clone(),
        },
        effects_rolled_back: false,
    }
}

/// Group bulk-mark exclusions by reason with counts, e.g. `2 change_active`.
///
/// Stable tokens rather than prose: a client reading a command record's detail
/// branches on the same vocabulary the change's `actions` block uses.
fn summarize_exclusions(excluded: &[(String, MarkExclusion)]) -> String {
    MarkExclusion::ALL
        .iter()
        .filter_map(|reason| {
            let count = excluded
                .iter()
                .filter(|(_, actual)| actual == reason)
                .count();
            (count > 0).then(|| format!("{count} {}", reason.as_str()))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Map a run-lifecycle refusal onto a v2 error code.
///
/// The distinction a client acts on is the same one the TUI shows an operator:
/// a mode that has to move on, a target that has to change, or a runtime that
/// could not dispatch at all.
pub fn map_run_control_error(error: &RunControlError) -> CommandFailure {
    match error {
        RunControlError::InvalidMode { .. } => {
            CommandFailure::new(ErrorCode::LifecycleConflict, error.to_string())
        }
        RunControlError::NoEligibleTarget { .. } | RunControlError::TargetIneligible { .. } => {
            CommandFailure::new(ErrorCode::TargetIneligible, error.to_string())
        }
        RunControlError::DispatchFailed { .. } => {
            CommandFailure::new(ErrorCode::InternalError, error.to_string())
        }
        RunControlError::Operator(inner) => map_operator_error(inner),
    }
}

/// Map a run-lifecycle success onto a v2 execution summary.
///
/// `changed` is set from the effect the service reported, never from the fact
/// that a command was accepted: a reservation that only queued, or a retry that
/// found nothing retryable, settles as `no_op`.
pub fn summarize_run_outcome(outcome: &RunControlOutcome) -> ExecutionSummary {
    match outcome {
        RunControlOutcome::RunDispatched {
            change_ids,
            explicit_retry,
            scheduler,
            excluded,
        } => {
            let how = match scheduler {
                SchedulerEffect::Started => "started the scheduler",
                SchedulerEffect::Notified => "woke the running scheduler",
                SchedulerEffect::None => "dispatched no scheduler work",
            };
            let kind = if *explicit_retry { "retry" } else { "run" };
            // A mark the admission could not take is still the operator's mark:
            // naming it here is what keeps an accepted run honest about the
            // targets it left behind.
            let left_behind = if excluded.is_empty() {
                String::new()
            } else {
                let detail = excluded
                    .iter()
                    .map(ExcludedTarget::describe)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("; excluded: {detail}")
            };
            ExecutionSummary::changed(format!("{kind} for {change_ids:?} {how}{left_behind}"))
        }
        RunControlOutcome::StopRequested => {
            ExecutionSummary::changed("graceful stop requested; the run stops at its next boundary")
        }
        RunControlOutcome::StopCancelled => {
            ExecutionSummary::changed("pending graceful stop was withdrawn")
        }
        RunControlOutcome::ForceStopped {
            classification,
            awaiting_safe_boundary,
        } => {
            // Only a snapshot that actually saw in-flight execution may be
            // reported as a force stop; everything else is an ordinary stop.
            let report = if classification.process_report.is_force_stop() {
                "force stop applied to active execution"
            } else {
                "run cancelled; no agent execution was active"
            };
            let boundary = if *awaiting_safe_boundary {
                "; waiting for in-flight work to reach a safe stop boundary"
            } else {
                ""
            };
            ExecutionSummary::changed(format!("{report}{boundary}"))
        }
        RunControlOutcome::ResolveReserved {
            change_id,
            reservation,
            ..
        } => match reservation {
            ResolveReservation::Active => ExecutionSummary::changed(format!(
                "merge resolution for '{change_id}' is the active resolve"
            )),
            ResolveReservation::Queued { position } => ExecutionSummary::changed(format!(
                "merge resolution for '{change_id}' is queued at position {position}"
            )),
        },
        RunControlOutcome::NoOp { reason } => match reason {
            RunNoOpReason::ResolveAlreadyReserved { change_id } => ExecutionSummary::no_op(
                format!("'{change_id}' already holds a resolve reservation"),
            ),
            RunNoOpReason::NoRetryableTarget => {
                ExecutionSummary::no_op("no marked change carried retryable evidence")
            }
        },
    }
}

/// Production executor over the shared process-local application transaction.
///
/// There is no control channel here on purpose, and no lifecycle decision
/// either. Every operator command is translated into an [`OperatorIntent`] and
/// handed to the *same* coordinator a keypress reaches, which is what makes
/// "equivalent intent takes the same path" structural rather than a property two
/// implementations have to keep agreeing about.
///
/// It is also why this adapter no longer publishes a projection refresh of its
/// own: the coordinator dispatches one authoritative outcome and reports the
/// exact revision that dispatch produced, so a client reading `result_revision`
/// finds the command's synchronous effect there by construction.
pub struct SharedServiceExecutor {
    application: Arc<OperatorApplication>,
    web_state: Arc<WebState>,
    /// The same worktree port the read routes use, so a client cannot be told a
    /// worktree exists by one half of the API and denied by the other.
    worktrees: Arc<dyn super::worktrees::WorktreeOperations>,
}

impl SharedServiceExecutor {
    /// Wire the executor to the shared application transaction.
    pub fn new(application: Arc<OperatorApplication>, web_state: Arc<WebState>) -> Self {
        Self {
            application,
            web_state,
            worktrees: Arc::new(super::worktrees::UnboundWorktreeOperations),
        }
    }

    /// Attach the worktree operation port.
    pub fn with_worktrees(
        mut self,
        worktrees: Arc<dyn super::worktrees::WorktreeOperations>,
    ) -> Self {
        self.worktrees = worktrees;
        self
    }

    /// Translate a v2 command into the shared intent vocabulary.
    ///
    /// `None` for the worktree commands, which are repository operations rather
    /// than operator lifecycle intent and own their own guards.
    fn intent(command: &CommandSpec) -> Option<OperatorIntent> {
        Some(match command {
            CommandSpec::Start => OperatorIntent::Start,
            CommandSpec::Stop => OperatorIntent::Stop,
            CommandSpec::CancelStop => OperatorIntent::CancelStop,
            CommandSpec::ForceStop => OperatorIntent::ForceStop,
            CommandSpec::SetExecutionMark { change_id, marked } => {
                OperatorIntent::SetExecutionMark {
                    change_id: change_id.clone(),
                    marked: *marked,
                }
            }
            CommandSpec::SetQueueIntent { change_id, queued } => OperatorIntent::SetQueueIntent {
                change_id: change_id.clone(),
                queued: *queued,
            },
            // Retry routes through run control rather than the per-change
            // service so the reducer transition and the scheduler dispatch that
            // makes it real settle together.
            CommandSpec::RetryChange { change_id } => OperatorIntent::RetryChange {
                change_id: change_id.clone(),
            },
            CommandSpec::RetryErrors { change_ids } => OperatorIntent::RetryErrors {
                change_ids: change_ids.clone(),
            },
            CommandSpec::StopAndDequeue { change_id } => OperatorIntent::StopAndDequeue {
                change_id: change_id.clone(),
            },
            CommandSpec::ResolveMerge { change_id } => OperatorIntent::ResolveMerge {
                change_id: change_id.clone(),
            },
            CommandSpec::SetAllExecutionMarks {} => OperatorIntent::SetAllExecutionMarks,
            CommandSpec::CreateWorktree { .. }
            | CommandSpec::DeleteWorktree { .. }
            | CommandSpec::MergeWorktree { .. } => return None,
        })
    }

    /// Run a worktree command and republish the monitoring snapshot.
    ///
    /// Repository mutations do not travel through the operator outcome
    /// vocabulary, so this is the one path that still refreshes the projection
    /// itself before the record settles.
    async fn worktree(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        let outcome = match command {
            CommandSpec::CreateWorktree { target, .. } => {
                self.worktrees.create(&target.change_id).await
            }
            CommandSpec::DeleteWorktree { target, .. } => {
                self.worktrees.delete(&target.worktree_id).await
            }
            CommandSpec::MergeWorktree { target, .. } => {
                self.worktrees.merge(&target.worktree_id).await
            }
            other => {
                debug_assert!(false, "not a worktree command: {other:?}");
                return Err(CommandFailure::new(
                    ErrorCode::InternalError,
                    "command is not a worktree operation",
                ));
            }
        };
        if matches!(&outcome, Ok(summary) if summary.changed) {
            self.web_state.sync_remote_control_projection().await;
        }
        outcome
    }
}

/// Wire an executor the way production wires one: one core mode, one
/// process-lifetime dispatch owner over the shared reducer, one coordinator.
///
/// Test-only, and deliberately *not* a stub. A test that assembled a different
/// shape here would stop covering the property these tests exist for — that a
/// remote command takes the same path a keypress takes.
///
/// `core_mode` is supplied rather than created here because it is the admission
/// authority: a test that let this build a second one would arrange a mode the
/// transaction never sees, and every lifecycle assertion would silently become
/// an assertion about `Select`.
#[cfg(test)]
pub fn wired_for_test(
    reducer: Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    run_control: Arc<crate::orchestration::run_control::RunControlService>,
    web_state: Arc<WebState>,
    core_mode: Arc<crate::orchestration::operator_coordinator::CoreMode>,
) -> (SharedServiceExecutor, Arc<OperatorApplication>) {
    let dispatcher = Arc::new(
        crate::events::EventDispatcher::new(
            reducer,
            vec![Arc::new(crate::web::state::WebEventSink::new(
                web_state.clone(),
            ))],
        )
        .with_core_mode(Some(core_mode.clone())),
    );
    let revisions: Arc<dyn crate::events::OutcomeRevisions> = web_state.clone();
    let application = Arc::new(
        OperatorApplication::new(core_mode.clone(), run_control, dispatcher)
            .with_revisions(Some(revisions)),
    );
    (
        SharedServiceExecutor::new(application.clone(), web_state),
        application,
    )
}

/// Project one settled application result onto the v2 vocabulary.
pub fn summarize_application(
    result: ApplicationResult,
) -> Result<ExecutionSummary, CommandFailure> {
    let ApplicationResult { outcome, revision } = result;
    match outcome {
        Ok(ApplicationOutcome::Run(outcome)) => {
            Ok(summarize_run_outcome(&outcome).at_revision(revision))
        }
        Ok(ApplicationOutcome::Operator(outcome)) => {
            Ok(summarize_outcome(&outcome).at_revision(revision))
        }
        Err(error) => Err(map_run_control_error(&error).at_revision(revision)),
    }
}

#[async_trait]
impl RemoteControlExecutor for SharedServiceExecutor {
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        match Self::intent(command) {
            Some(intent) => summarize_application(self.application.apply(intent).await),
            None => self.worktree(command).await,
        }
    }

    /// Execute under the endpoint's gate rather than acquiring a second one.
    ///
    /// The gate is a plain mutex and is not reentrant: re-acquiring it here
    /// would deadlock against the endpoint that is holding it through
    /// settlement.
    async fn execute_held(
        &self,
        command: &CommandSpec,
        gate: Option<GateGuard>,
    ) -> Result<ExecutionSummary, CommandFailure> {
        match (Self::intent(command), gate) {
            (Some(intent), Some(gate)) => {
                summarize_application(self.application.apply_held(intent, gate).await)
            }
            (Some(intent), None) => summarize_application(self.application.apply(intent).await),
            // A worktree operation is a repository mutation, not operator
            // lifecycle intent; the gate is released before it runs so a long
            // merge cannot monopolize command admission.
            (None, gate) => {
                drop(gate);
                self.worktree(command).await
            }
        }
    }

    async fn begin(&self, command: &CommandSpec, gate: Option<GateGuard>) -> Applied {
        // A stop-and-dequeue is the only two-phase command: it issues
        // cancellation under the caller's gate and settles later, so the gate
        // and the TUI event loop are both free while termination confirms.
        let CommandSpec::StopAndDequeue { change_id } = command else {
            return Applied::Ordinary(gate);
        };

        let pending = match self.application.begin_stop_and_dequeue(change_id).await {
            Ok(pending) => pending,
            Err(error) => {
                // Phase one refused before cancellation was issued, so this is
                // an ordinary failure with no effect and no revision.
                return Applied::Settled(Err(map_run_control_error(&error)));
            }
        };

        // Phase one is over. Releasing the gate here — before the wait, not
        // during it — is what keeps force stop, unrelated commands, event
        // fan-out, and rendering live while confirmation is pending.
        drop(gate);

        let application = self.application.clone();
        Applied::Pending(Box::pin(async move {
            summarize_application(application.settle_stop_and_dequeue(pending).await)
        }))
    }
}
