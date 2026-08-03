//! Typed TUI → lifecycle state mapping.
//!
//! Semantic lifecycle state is derived from typed TUI state (`AppExecutionMode`,
//! `ModalState`, `StopMode`, and the currently processing change). Rendered screen
//! contents are never parsed, and this mapping never feeds back into
//! `ReducerCommand` or `EventSink` ownership: it is observability-only.

use crate::lifecycle_integration::{LifecycleContext, LifecycleState};

use super::state::AppState;
use super::types::{AppExecutionMode, ModalState, StopMode};

/// Typed snapshot of the TUI state used to derive lifecycle reporting.
///
/// Captured from [`AppState`] so the mapping itself stays pure and testable
/// without constructing terminal or orchestration boundaries. Execution mode and
/// modal interaction are carried separately, exactly as the TUI stores them.
#[derive(Debug, Clone, PartialEq)]
pub struct TuiLifecycleSnapshot {
    /// Current execution lifecycle mode.
    pub execution_mode: AppExecutionMode,
    /// Active modal interaction, if any.
    pub modal: Option<ModalState>,
    /// Current stop mode.
    pub stop_mode: StopMode,
    /// Change currently being processed, if any.
    pub current_change: Option<String>,
}

impl TuiLifecycleSnapshot {
    /// Capture the lifecycle-relevant subset of the TUI state.
    pub fn from_app(app: &AppState) -> Self {
        Self {
            execution_mode: app.execution_mode,
            modal: app.modal.clone(),
            stop_mode: app.stop_mode.clone(),
            current_change: app.current_change.clone(),
        }
    }

    /// Semantic lifecycle state for this snapshot.
    ///
    /// Projection order is: a confirmation that awaits an operator decision blocks;
    /// a QR overlay is presentation only and reports whatever execution is doing
    /// underneath it; otherwise the execution mode maps directly.
    pub fn lifecycle_state(&self) -> LifecycleState {
        if self
            .modal
            .as_ref()
            .is_some_and(ModalState::is_user_decision)
        {
            return LifecycleState::Blocked;
        }

        lifecycle_state_for_execution_mode(self.execution_mode)
    }

    /// Privacy-safe context for this snapshot.
    pub fn lifecycle_context(&self, workspace: &str) -> LifecycleContext {
        let change_id = match &self.modal {
            // A force-kill confirmation is about a specific change even when no
            // change is the globally "current" one.
            Some(ModalState::ConfirmForceKill { change_id }) => Some(change_id.clone()),
            _ => self.current_change.clone(),
        };

        LifecycleContext::workspace(workspace).with_change_id(change_id)
    }
}

/// Semantic lifecycle state for a typed TUI execution mode.
pub fn lifecycle_state_for_execution_mode(mode: AppExecutionMode) -> LifecycleState {
    match mode {
        // Ready/selection UI and halted processing are idle.
        AppExecutionMode::Select | AppExecutionMode::Stopped => LifecycleState::Idle,
        // Active orchestration and graceful stop are both work in progress.
        AppExecutionMode::Running | AppExecutionMode::Stopping => LifecycleState::Working,
        // A fatal error requires an explicit retry decision.
        AppExecutionMode::Error => LifecycleState::Blocked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXECUTION_MODES: [AppExecutionMode; 5] = [
        AppExecutionMode::Select,
        AppExecutionMode::Running,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ];

    fn snapshot(execution_mode: AppExecutionMode) -> TuiLifecycleSnapshot {
        TuiLifecycleSnapshot {
            execution_mode,
            modal: None,
            stop_mode: StopMode::None,
            current_change: None,
        }
    }

    fn with_modal(execution_mode: AppExecutionMode, modal: ModalState) -> TuiLifecycleSnapshot {
        let mut snapshot = snapshot(execution_mode);
        snapshot.modal = Some(modal);
        snapshot
    }

    fn worktree_confirmation() -> ModalState {
        ModalState::ConfirmWorktreeDelete {
            path: std::path::PathBuf::from("/tmp/wt-a"),
            branch: "change-a".to_string(),
        }
    }

    fn force_kill_confirmation() -> ModalState {
        ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        }
    }

    #[test]
    fn every_execution_mode_maps_without_a_modal() {
        let expected = [
            LifecycleState::Idle,
            LifecycleState::Working,
            LifecycleState::Working,
            LifecycleState::Idle,
            LifecycleState::Blocked,
        ];

        for (mode, state) in EXECUTION_MODES.into_iter().zip(expected) {
            assert_eq!(
                snapshot(mode).lifecycle_state(),
                state,
                "unexpected lifecycle for {:?}",
                mode
            );
        }
    }

    #[test]
    fn qr_overlay_reports_the_underlying_execution_mode() {
        for mode in EXECUTION_MODES {
            assert_eq!(
                with_modal(mode, ModalState::QrPopup).lifecycle_state(),
                snapshot(mode).lifecycle_state(),
                "QR presentation must not change the lifecycle for {:?}",
                mode
            );
        }
    }

    #[test]
    fn valid_confirmations_report_blocked_over_every_execution_mode() {
        for mode in EXECUTION_MODES {
            for modal in [worktree_confirmation(), force_kill_confirmation()] {
                assert_eq!(
                    with_modal(mode, modal.clone()).lifecycle_state(),
                    LifecycleState::Blocked,
                    "{:?} under {:?} must report blocked",
                    modal,
                    mode
                );
            }
        }
    }

    #[test]
    fn closing_a_confirmation_exposes_the_latest_execution_mode() {
        // A confirmation opened over Running that survives into Stopping must not
        // resurrect Running when it closes.
        let mut snapshot = with_modal(AppExecutionMode::Running, worktree_confirmation());
        snapshot.execution_mode = AppExecutionMode::Stopped;
        assert_eq!(snapshot.lifecycle_state(), LifecycleState::Blocked);

        snapshot.modal = None;
        assert_eq!(snapshot.lifecycle_state(), LifecycleState::Idle);
    }

    #[test]
    fn ready_running_confirmation_stopping_sequence_maps_to_expected_transitions() {
        let sequence = [
            snapshot(AppExecutionMode::Select),
            snapshot(AppExecutionMode::Running),
            with_modal(AppExecutionMode::Running, force_kill_confirmation()),
            snapshot(AppExecutionMode::Stopping),
            snapshot(AppExecutionMode::Stopped),
        ];

        let states: Vec<LifecycleState> = sequence
            .iter()
            .map(TuiLifecycleSnapshot::lifecycle_state)
            .collect();

        assert_eq!(
            states,
            vec![
                LifecycleState::Idle,
                LifecycleState::Working,
                LifecycleState::Blocked,
                LifecycleState::Working,
                LifecycleState::Idle,
            ]
        );
    }

    #[test]
    fn context_reports_workspace_and_current_change_only() {
        let mut running = snapshot(AppExecutionMode::Running);
        running.current_change = Some("change-a".to_string());

        let context = running.lifecycle_context("/repo");
        assert_eq!(context.workspace.as_deref(), Some("/repo"));
        assert_eq!(context.change_id.as_deref(), Some("change-a"));
        assert_eq!(context.session_id, None);
    }

    #[test]
    fn force_kill_confirmation_context_uses_the_confirmed_change() {
        let confirm = with_modal(
            AppExecutionMode::Running,
            ModalState::ConfirmForceKill {
                change_id: "change-b".to_string(),
            },
        );

        let context = confirm.lifecycle_context("/repo");
        assert_eq!(context.change_id.as_deref(), Some("change-b"));
    }

    #[test]
    fn snapshot_is_captured_from_typed_app_state() {
        let mut app = AppState::new(Vec::new());
        app.execution_mode = AppExecutionMode::Running;
        app.current_change = Some("change-a".to_string());
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.show_qr_popup();

        let captured = TuiLifecycleSnapshot::from_app(&app);

        assert_eq!(captured.execution_mode, AppExecutionMode::Running);
        assert_eq!(captured.modal, Some(ModalState::QrPopup));
        assert_eq!(captured.current_change.as_deref(), Some("change-a"));
        assert_eq!(captured.lifecycle_state(), LifecycleState::Working);
    }
}
