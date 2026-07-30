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
    MarkRoute, NoOpReason, OperatorCommandError, OperatorCommandService, OperatorMode,
    OperatorOutcome,
};
use crate::web::state::{ControlCommand, WebState};

use super::dto::{CommandSpec, ErrorCode};

/// What a delegated command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionSummary {
    /// True when the command produced a real effect.
    pub changed: bool,
    /// Sanitized operator-facing detail.
    pub detail: Option<String>,
}

impl ExecutionSummary {
    /// A command that produced an effect.
    pub fn changed(detail: impl Into<String>) -> Self {
        Self {
            changed: true,
            detail: Some(detail.into()),
        }
    }

    /// A command that was valid but changed nothing.
    pub fn no_op(detail: impl Into<String>) -> Self {
        Self {
            changed: false,
            detail: Some(detail.into()),
        }
    }
}

/// A typed refusal from the shared behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFailure {
    /// Stable v2 error code.
    pub error_code: ErrorCode,
    /// Sanitized message.
    pub message: String,
}

impl CommandFailure {
    /// Build a failure.
    pub fn new(error_code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error_code,
            message: message.into(),
        }
    }
}

/// The port the command endpoint calls once a command has been admitted.
#[async_trait]
pub trait RemoteControlExecutor: Send + Sync {
    /// Execute an admitted command. Called at most once per command record.
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure>;
}

/// Map an operator-service refusal onto a v2 error code.
///
/// The distinction that matters to a client is *who* has to change: a
/// `lifecycle_conflict` needs the run to move on, a `target_ineligible` needs a
/// different target.
pub fn map_operator_error(error: &OperatorCommandError) -> CommandFailure {
    match error {
        OperatorCommandError::MarkNotAllowed { route, .. } => {
            let code = match route {
                // Recovery is owned by retry commands in this mode.
                MarkRoute::RetryRequired => ErrorCode::LifecycleConflict,
                _ => ErrorCode::TargetIneligible,
            };
            CommandFailure::new(code, error.to_string())
        }
        OperatorCommandError::MissingCancellationHandle { .. }
        | OperatorCommandError::RetryUnsupported { .. } => {
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
        OperatorOutcome::Dequeued { change_id } => {
            ExecutionSummary::changed(format!("'{change_id}' was cancelled and dequeued"))
        }
        OperatorOutcome::Retry(plan) => {
            if plan.is_empty() {
                ExecutionSummary::no_op("no change carried retryable evidence")
            } else {
                ExecutionSummary::changed(format!("retry accepted for {:?}", plan.change_ids))
            }
        }
        OperatorOutcome::NoOp { change_id, reason } => {
            let why = match reason {
                NoOpReason::MarkUnchanged => "execution mark already had the requested value",
                NoOpReason::ReducerRejected => "the reducer produced no state change",
            };
            if change_id.is_empty() {
                ExecutionSummary::no_op(why)
            } else {
                ExecutionSummary::no_op(format!("'{change_id}': {why}"))
            }
        }
    }
}

/// Map the operator-facing application mode string onto the shared enum.
pub fn operator_mode(app_mode: &str) -> OperatorMode {
    match app_mode {
        "running" => OperatorMode::Running,
        "stopping" => OperatorMode::Stopping,
        "stopped" => OperatorMode::Stopped,
        "error" => OperatorMode::Error,
        _ => OperatorMode::Select,
    }
}

/// Production executor: shared operator command service plus the existing
/// frontend control channel for run lifecycle commands.
pub struct SharedServiceExecutor {
    service: Arc<OperatorCommandService>,
    web_state: Arc<WebState>,
    projection: Arc<super::projection::Projection>,
}

impl SharedServiceExecutor {
    /// Wire the executor to the shared service, the control channel owner, and
    /// the projection it reads the current mode from.
    pub fn new(
        service: Arc<OperatorCommandService>,
        web_state: Arc<WebState>,
        projection: Arc<super::projection::Projection>,
    ) -> Self {
        Self {
            service,
            web_state,
            projection,
        }
    }

    fn lifecycle(
        &self,
        command: ControlCommand,
        detail: &str,
    ) -> Result<ExecutionSummary, CommandFailure> {
        self.web_state.send_control_command(command).map_err(|e| {
            CommandFailure::new(
                ErrorCode::LifecycleConflict,
                format!("this instance cannot accept run lifecycle commands: {e}"),
            )
        })?;
        Ok(ExecutionSummary::changed(detail.to_string()))
    }
}

#[async_trait]
impl RemoteControlExecutor for SharedServiceExecutor {
    async fn execute(&self, command: &CommandSpec) -> Result<ExecutionSummary, CommandFailure> {
        let mode = operator_mode(&self.projection.snapshot().0.app_mode);

        match command {
            CommandSpec::Start => self.lifecycle(ControlCommand::Start, "run start requested"),
            CommandSpec::Stop => self.lifecycle(ControlCommand::Stop, "graceful stop requested"),
            CommandSpec::CancelStop => {
                self.lifecycle(ControlCommand::CancelStop, "pending stop cancelled")
            }
            CommandSpec::ForceStop => {
                self.lifecycle(ControlCommand::ForceStop, "force stop requested")
            }
            CommandSpec::SetExecutionMark { change_id, marked } => self
                .service
                .set_execution_mark(mode, change_id, *marked)
                .await
                .map(|outcome| summarize_outcome(&outcome))
                .map_err(|error| map_operator_error(&error)),
            CommandSpec::SetQueueIntent { change_id, queued } => {
                let outcome = if *queued {
                    self.service.add_to_queue(change_id).await
                } else {
                    self.service.remove_from_queue(change_id).await
                };
                outcome
                    .map(|queue| summarize_outcome(&OperatorOutcome::Queue(queue)))
                    .map_err(|error| map_operator_error(&error))
            }
            CommandSpec::RetryChange { change_id } => self
                .service
                .retry_change(change_id)
                .await
                .map(|plan| summarize_outcome(&OperatorOutcome::Retry(plan)))
                .map_err(|error| map_operator_error(&error)),
            CommandSpec::RetryErrors { change_ids } => {
                let plan = self.service.retry_errors(change_ids).await;
                Ok(summarize_outcome(&OperatorOutcome::Retry(plan)))
            }
            CommandSpec::StopAndDequeue { change_id } => self
                .service
                .stop_and_dequeue(change_id)
                .await
                .map(|outcome| summarize_outcome(&outcome))
                .map_err(|error| map_operator_error(&error)),
            CommandSpec::ResolveMerge { change_id } => {
                if self.service.resolve_merge(change_id).await {
                    Ok(ExecutionSummary::changed(format!(
                        "merge resolution requested for '{change_id}'"
                    )))
                } else {
                    Ok(ExecutionSummary::no_op(format!(
                        "'{change_id}' is not waiting on a merge"
                    )))
                }
            }
        }
    }
}
