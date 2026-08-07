//! Projection of accepted operator-command outcomes onto TUI presentation.
//!
//! Every handler here is *painting*. The command was already validated, already
//! committed, and already published by the process-lifetime dispatch boundary
//! before this frontend saw it; what is left is the row cache and the lifecycle
//! mode the screen renders.
//!
//! That is the whole reason these live on the event path rather than in the
//! command handlers. A command submitted from this TUI and the same command
//! submitted through `/api/v2` arrive here identically, so the next frame is the
//! same either way — which is the convergence the change exists for.

use super::AppState;
use crate::events::OperatorCommandEffect;
use crate::tui::events::LogEntry;
use crate::tui::types::{AppExecutionMode, StopMode};

impl AppState {
    /// Project one accepted operator command's decision facts.
    pub(crate) fn handle_operator_command_applied(&mut self, effect: OperatorCommandEffect) {
        match effect {
            // Which of the two run projections applies is carried by the
            // dispatch, not re-derived here: only a newly spawned scheduler
            // proves execution has begun, while a dispatch that woke a scheduler
            // already alive — including one parked in persistent-idle Ready —
            // leaves the execution axis to the first typed work-start event.
            OperatorCommandEffect::RunDispatched {
                change_ids,
                scheduler_started,
                ..
            } => {
                if scheduler_started {
                    self.begin_run(&change_ids);
                } else {
                    self.queue_run(&change_ids);
                }
            }
            // Withdrawing a stop restores where the stop came from. A stop
            // requested from persistent-idle Ready returns to Ready: nothing has
            // started, and claiming Running would advertise execution no typed
            // event ever proved.
            OperatorCommandEffect::StopCancelled => {
                self.stop_mode = StopMode::None;
                self.execution_mode = if self.persistent_scheduler_idle {
                    AppExecutionMode::Select
                } else {
                    AppExecutionMode::Running
                };
            }
            OperatorCommandEffect::ForceStopAwaitingBoundary { force_stop } => {
                if force_stop {
                    self.stop_mode = StopMode::ForceStopped;
                }
                self.execution_mode = AppExecutionMode::Stopping;
                self.add_log(LogEntry::info(
                    "Waiting for in-flight work to reach a safe stop boundary...",
                ));
            }
            // Marks are read back from the shared store by
            // `handle_orchestrator_event`, which is the one-way store-to-row
            // projection. Naming the changed IDs here is what keeps the *write*
            // side target-scoped: a row this command never named is left to
            // whatever the store says, so a remote delta for an unrelated change
            // cannot be erased by a local command.
            OperatorCommandEffect::MarkDelta { change_ids, marked } => {
                for change_id in change_ids {
                    if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                        change.selected = marked;
                    }
                }
            }
            // Queue membership is reducer-derived: the run loop re-reads display
            // statuses for this event. Presentation and execution marks stay
            // separate axes here, so a checked Running row and a hidden Error
            // retry intent cannot be confused for each other.
            OperatorCommandEffect::QueueDelta { .. } => {}
            OperatorCommandEffect::ResolveReserved { change_id, active } => {
                if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
                    change.set_display_status_cache("resolve pending");
                }
                if active
                    && matches!(
                        self.execution_mode,
                        AppExecutionMode::Select | AppExecutionMode::Stopped
                    )
                {
                    self.execution_mode = AppExecutionMode::Running;
                }
            }
        }
    }

    /// Project an accepted graceful stop.
    ///
    /// `Stopping` is the exact existing event for this, so the projection lives
    /// on the event path like every other lifecycle transition rather than in the
    /// command handler that happened to cause it.
    pub(crate) fn handle_stopping(&mut self) {
        self.stop_mode = StopMode::GracefulPending;
        self.execution_mode = AppExecutionMode::Stopping;
    }
}
