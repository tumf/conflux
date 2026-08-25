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
            // Which of the two run projections applies is decided from the
            // dispatch plus this frontend's own ordered facts, through the gate
            // Core and Web read too. A newly spawned scheduler proves execution
            // has begun. A dispatch that woke a scheduler already alive proves
            // the same *episode* when it arrives against persistent-idle Ready
            // with committed targets: the operator's Start was accepted, so the
            // run they asked for is theirs to see now rather than after
            // dependency analysis. Every other wake — into a live run, or with
            // nothing committed — leaves the mode to the first typed work-start
            // event as before.
            OperatorCommandEffect::RunDispatched {
                change_ids,
                scheduler_started,
                ..
            } => {
                if scheduler_started
                    || crate::events::accepted_start_opens_idle_run_episode(
                        self.execution_mode.app_mode_token(),
                        self.persistent_scheduler_idle,
                        scheduler_started,
                        &change_ids,
                    )
                {
                    // The accepted Start closes the presentation episode: it is
                    // deliberately earlier than admitted work, and the rows stay
                    // queued to say so.
                    self.persistent_scheduler_idle = false;
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
    ///
    /// A stop admitted from Ready is an idle-origin stop over a live parked
    /// scheduler, so the episode fact is established through the shared rule
    /// before the mode moves. Ready reached through `AllCompleted` settlement
    /// carries no idle edge of its own, and cancel-stop reads this fact to decide
    /// whether it is returning to Ready or to a run episode that really existed.
    pub(crate) fn handle_stopping(&mut self) {
        if crate::events::graceful_stop_is_idle_origin(self.execution_mode.app_mode_token()) {
            self.persistent_scheduler_idle = true;
        }
        self.stop_mode = StopMode::GracefulPending;
        self.execution_mode = AppExecutionMode::Stopping;
    }
}
