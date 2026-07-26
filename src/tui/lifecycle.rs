//! Typed TUI → lifecycle state mapping.
//!
//! Semantic lifecycle state is derived from typed TUI state (`AppMode`,
//! `StopMode`, and the currently processing change). Rendered screen contents
//! are never parsed, and this mapping never feeds back into `ReducerCommand`
//! or `EventSink` ownership: it is observability-only.

use crate::lifecycle_integration::{LifecycleContext, LifecycleState};

use super::state::AppState;
use super::types::{AppMode, StopMode};

/// Typed snapshot of the TUI state used to derive lifecycle reporting.
///
/// Captured from [`AppState`] so the mapping itself stays pure and testable
/// without constructing terminal or orchestration boundaries.
#[derive(Debug, Clone, PartialEq)]
pub struct TuiLifecycleSnapshot {
    /// Current application mode.
    pub mode: AppMode,
    /// Mode to return to after a modal popup closes.
    pub previous_mode: Option<AppMode>,
    /// Current stop mode.
    pub stop_mode: StopMode,
    /// Change currently being processed, if any.
    pub current_change: Option<String>,
}

impl TuiLifecycleSnapshot {
    /// Capture the lifecycle-relevant subset of the TUI state.
    pub fn from_app(app: &AppState) -> Self {
        Self {
            mode: app.mode.clone(),
            previous_mode: app.previous_mode.clone(),
            stop_mode: app.stop_mode.clone(),
            current_change: app.current_change.clone(),
        }
    }

    /// Semantic lifecycle state for this snapshot.
    pub fn lifecycle_state(&self) -> LifecycleState {
        // Popups are presentation-only overlays. QR display does not block the
        // user on a workflow decision, so report the underlying mode instead.
        if matches!(self.mode, AppMode::QrPopup) {
            return self
                .previous_mode
                .as_ref()
                .map(lifecycle_state_for_mode)
                .unwrap_or(LifecycleState::Idle);
        }

        lifecycle_state_for_mode(&self.mode)
    }

    /// Privacy-safe context for this snapshot.
    pub fn lifecycle_context(&self, workspace: &str) -> LifecycleContext {
        let change_id = match &self.mode {
            // A force-kill confirmation is about a specific change even when no
            // change is the globally "current" one.
            AppMode::ConfirmForceKill { change_id } => Some(change_id.clone()),
            _ => self.current_change.clone(),
        };

        LifecycleContext::workspace(workspace).with_change_id(change_id)
    }
}

/// Semantic lifecycle state for a typed TUI mode.
pub fn lifecycle_state_for_mode(mode: &AppMode) -> LifecycleState {
    match mode {
        // Ready/selection UI and halted processing are idle.
        AppMode::Select | AppMode::Stopped => LifecycleState::Idle,
        // Active orchestration and graceful stop are both work in progress.
        AppMode::Running | AppMode::Stopping => LifecycleState::Working,
        // Interactions that require a user decision block the agent.
        AppMode::Error | AppMode::ConfirmWorktreeDelete | AppMode::ConfirmForceKill { .. } => {
            LifecycleState::Blocked
        }
        // Handled by the snapshot before reaching here; treated as idle standalone.
        AppMode::QrPopup => LifecycleState::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(mode: AppMode) -> TuiLifecycleSnapshot {
        TuiLifecycleSnapshot {
            mode,
            previous_mode: None,
            stop_mode: StopMode::None,
            current_change: None,
        }
    }

    #[test]
    fn selection_and_stopped_ui_report_idle() {
        assert_eq!(
            snapshot(AppMode::Select).lifecycle_state(),
            LifecycleState::Idle
        );
        assert_eq!(
            snapshot(AppMode::Stopped).lifecycle_state(),
            LifecycleState::Idle
        );
    }

    #[test]
    fn running_and_stopping_report_working() {
        assert_eq!(
            snapshot(AppMode::Running).lifecycle_state(),
            LifecycleState::Working
        );
        assert_eq!(
            snapshot(AppMode::Stopping).lifecycle_state(),
            LifecycleState::Working
        );
    }

    #[test]
    fn confirmation_and_retry_decisions_report_blocked() {
        assert_eq!(
            snapshot(AppMode::ConfirmWorktreeDelete).lifecycle_state(),
            LifecycleState::Blocked
        );
        assert_eq!(
            snapshot(AppMode::ConfirmForceKill {
                change_id: "change-a".to_string()
            })
            .lifecycle_state(),
            LifecycleState::Blocked
        );
        assert_eq!(
            snapshot(AppMode::Error).lifecycle_state(),
            LifecycleState::Blocked
        );
    }

    #[test]
    fn qr_popup_reports_the_underlying_mode() {
        let mut running_popup = snapshot(AppMode::QrPopup);
        running_popup.previous_mode = Some(AppMode::Running);
        assert_eq!(running_popup.lifecycle_state(), LifecycleState::Working);

        let mut select_popup = snapshot(AppMode::QrPopup);
        select_popup.previous_mode = Some(AppMode::Select);
        assert_eq!(select_popup.lifecycle_state(), LifecycleState::Idle);

        assert_eq!(
            snapshot(AppMode::QrPopup).lifecycle_state(),
            LifecycleState::Idle
        );
    }

    #[test]
    fn ready_running_confirmation_stopping_sequence_maps_to_expected_transitions() {
        let sequence = [
            AppMode::Select,
            AppMode::Running,
            AppMode::ConfirmForceKill {
                change_id: "change-a".to_string(),
            },
            AppMode::Stopping,
            AppMode::Stopped,
        ];

        let states: Vec<LifecycleState> = sequence
            .iter()
            .map(|mode| snapshot(mode.clone()).lifecycle_state())
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
        let mut running = snapshot(AppMode::Running);
        running.current_change = Some("change-a".to_string());

        let context = running.lifecycle_context("/repo");
        assert_eq!(context.workspace.as_deref(), Some("/repo"));
        assert_eq!(context.change_id.as_deref(), Some("change-a"));
        assert_eq!(context.session_id, None);
    }

    #[test]
    fn force_kill_confirmation_context_uses_the_confirmed_change() {
        let confirm = snapshot(AppMode::ConfirmForceKill {
            change_id: "change-b".to_string(),
        });

        let context = confirm.lifecycle_context("/repo");
        assert_eq!(context.change_id.as_deref(), Some("change-b"));
    }

    #[test]
    fn snapshot_is_captured_from_typed_app_state() {
        let mut app = AppState::new(Vec::new());
        app.mode = AppMode::Running;
        app.current_change = Some("change-a".to_string());

        let captured = TuiLifecycleSnapshot::from_app(&app);

        assert_eq!(captured.mode, AppMode::Running);
        assert_eq!(captured.current_change.as_deref(), Some("change-a"));
        assert_eq!(captured.lifecycle_state(), LifecycleState::Working);
    }
}
