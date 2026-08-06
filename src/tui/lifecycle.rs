//! Typed TUI → lifecycle state mapping.
//!
//! Semantic lifecycle state is derived from typed TUI state (`AppExecutionMode`,
//! `ModalState`, `StopMode`, the currently processing change, and the
//! reducer-synchronized row statuses). Rendered screen contents are never parsed,
//! and this mapping never feeds back into `ReducerCommand` or `EventSink`
//! ownership: it is observability-only.

use crate::lifecycle_integration::{LifecycleContext, LifecycleState};
use crate::orchestration::operator_command::is_active_status;

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
    /// Whether any row is actively executing or still queued for dispatch.
    ///
    /// A copied observability fact, never a scheduler or admission input.
    pub has_active_or_queued_change: bool,
    /// Whether any row is waiting in `blocked` or `stalled`.
    pub has_blocked_or_stalled_change: bool,
}

impl TuiLifecycleSnapshot {
    /// Capture the lifecycle-relevant subset of the TUI state.
    ///
    /// Row facts are read from the reducer-synchronized display caches, so the
    /// active vocabulary is the canonical one rather than a copy that could drift
    /// from [`is_active_status`].
    pub fn from_app(app: &AppState) -> Self {
        let mut has_active_or_queued_change = false;
        let mut has_blocked_or_stalled_change = false;
        for change in &app.changes {
            let status = change.display_status_cache.as_str();
            if is_active_status(status) || status == "queued" {
                has_active_or_queued_change = true;
            }
            if matches!(status, "blocked" | "stalled") {
                has_blocked_or_stalled_change = true;
            }
        }

        Self {
            execution_mode: app.execution_mode,
            modal: app.modal.clone(),
            stop_mode: app.stop_mode.clone(),
            current_change: app.current_change.clone(),
            has_active_or_queued_change,
            has_blocked_or_stalled_change,
        }
    }

    /// Semantic lifecycle state for this snapshot.
    ///
    /// Projection order is: a confirmation that awaits an operator decision blocks;
    /// a persistent `Running` process whose every remaining row is waiting in
    /// `blocked` or `stalled` blocks, because nothing is executing and nothing is
    /// admitted for dispatch; a QR overlay is presentation only and reports
    /// whatever execution is doing underneath it; otherwise the execution mode
    /// maps directly.
    pub fn lifecycle_state(&self) -> LifecycleState {
        if self
            .modal
            .as_ref()
            .is_some_and(ModalState::is_user_decision)
        {
            return LifecycleState::Blocked;
        }

        // Active or queued work always wins: a mixed run is still working, and a
        // queued row keeps `working` while it remains admitted for dispatch.
        if matches!(self.execution_mode, AppExecutionMode::Running)
            && !self.has_active_or_queued_change
            && self.has_blocked_or_stalled_change
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
            has_active_or_queued_change: false,
            has_blocked_or_stalled_change: false,
        }
    }

    /// Build an `AppState` whose rows carry the given reducer display statuses.
    fn app_with_row_statuses(
        execution_mode: AppExecutionMode,
        statuses: &[&str],
    ) -> crate::tui::state::AppState {
        use crate::openspec::{Change, ProposalMetadata};

        let changes: Vec<Change> = statuses
            .iter()
            .enumerate()
            .map(|(index, _)| Change {
                id: format!("change-{index}"),
                completed_tasks: 0,
                total_tasks: 1,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            })
            .collect();

        let mut app = AppState::new(changes);
        app.execution_mode = execution_mode;
        for (change, status) in app.changes.iter_mut().zip(statuses) {
            change.set_display_status_cache(status);
        }
        app
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
    fn row_statuses_project_the_documented_precedence_table() {
        // (execution mode, reducer-synchronized row statuses, expected lifecycle)
        let cases: [(AppExecutionMode, &[&str], LifecycleState); 12] = [
            // Blocked/stalled-only waits under a persistent Running process.
            (
                AppExecutionMode::Running,
                &["blocked"],
                LifecycleState::Blocked,
            ),
            (
                AppExecutionMode::Running,
                &["stalled"],
                LifecycleState::Blocked,
            ),
            (
                AppExecutionMode::Running,
                &["blocked", "stalled", "archived", "not queued"],
                LifecycleState::Blocked,
            ),
            // Any active row wins over waiting rows.
            (
                AppExecutionMode::Running,
                &["applying", "stalled"],
                LifecycleState::Working,
            ),
            (
                AppExecutionMode::Running,
                &["preparing", "blocked"],
                LifecycleState::Working,
            ),
            (
                AppExecutionMode::Running,
                &["resolving", "blocked"],
                LifecycleState::Working,
            ),
            // A queued row keeps work admitted, so the wait is not exclusive.
            (
                AppExecutionMode::Running,
                &["queued", "blocked"],
                LifecycleState::Working,
            ),
            // Ordinary zero-active Running keeps the existing fallback.
            (AppExecutionMode::Running, &[], LifecycleState::Working),
            (
                AppExecutionMode::Running,
                &["not queued", "archived", "merge wait"],
                LifecycleState::Working,
            ),
            // Non-Running modes are unchanged by row status.
            (
                AppExecutionMode::Stopping,
                &["blocked", "stalled"],
                LifecycleState::Working,
            ),
            (AppExecutionMode::Select, &["blocked"], LifecycleState::Idle),
            (
                AppExecutionMode::Stopped,
                &["stalled"],
                LifecycleState::Idle,
            ),
        ];

        for (mode, statuses, expected) in cases {
            let app = app_with_row_statuses(mode, statuses);
            assert_eq!(
                TuiLifecycleSnapshot::from_app(&app).lifecycle_state(),
                expected,
                "unexpected lifecycle for {mode:?} with rows {statuses:?}"
            );
        }
    }

    #[test]
    fn error_mode_stays_blocked_with_any_row_combination() {
        for statuses in [&[][..], &["applying"][..], &["blocked"][..]] {
            let app = app_with_row_statuses(AppExecutionMode::Error, statuses);
            assert_eq!(
                TuiLifecycleSnapshot::from_app(&app).lifecycle_state(),
                LifecycleState::Blocked,
                "error mode must stay blocked with rows {statuses:?}"
            );
        }
    }

    #[test]
    fn qr_overlay_is_transparent_over_a_blocked_only_wait() {
        let mut app = app_with_row_statuses(AppExecutionMode::Running, &["blocked"]);
        app.web_url = Some("http://127.0.0.1:8080".to_string());
        app.show_qr_popup();

        let captured = TuiLifecycleSnapshot::from_app(&app);
        assert_eq!(captured.modal, Some(ModalState::QrPopup));
        assert_eq!(captured.lifecycle_state(), LifecycleState::Blocked);
    }

    #[test]
    fn user_decision_modal_outranks_an_active_row() {
        let mut app = app_with_row_statuses(AppExecutionMode::Running, &["applying"]);
        app.modal = Some(force_kill_confirmation());

        assert_eq!(
            TuiLifecycleSnapshot::from_app(&app).lifecycle_state(),
            LifecycleState::Blocked
        );
    }

    #[test]
    fn row_facts_are_captured_from_the_display_status_caches() {
        let app = app_with_row_statuses(AppExecutionMode::Running, &["accepting", "stalled"]);
        let captured = TuiLifecycleSnapshot::from_app(&app);

        assert!(captured.has_active_or_queued_change);
        assert!(captured.has_blocked_or_stalled_change);

        let idle = app_with_row_statuses(AppExecutionMode::Running, &["not queued"]);
        let captured_idle = TuiLifecycleSnapshot::from_app(&idle);
        assert!(!captured_idle.has_active_or_queued_change);
        assert!(!captured_idle.has_blocked_or_stalled_change);
    }

    #[test]
    fn every_canonical_active_status_preserves_working() {
        for status in [
            "preparing",
            "applying",
            "accepting",
            "rejecting",
            "archiving",
            "resolving",
        ] {
            assert!(
                is_active_status(status),
                "{status} must remain a canonical active status"
            );
            let app = app_with_row_statuses(AppExecutionMode::Running, &[status, "blocked"]);
            assert_eq!(
                TuiLifecycleSnapshot::from_app(&app).lifecycle_state(),
                LifecycleState::Working,
                "{status} alongside a blocked row must report working"
            );
        }
    }

    /// The frame loop republishes every frame, so the row facts must survive the
    /// publisher's deduplication as one semantic transition rather than
    /// alternating with `working`.
    #[test]
    fn repeated_frames_over_a_blocked_only_app_state_publish_one_transition() {
        use crate::lifecycle_integration::LifecycleStateTracker;

        let mut app = app_with_row_statuses(AppExecutionMode::Running, &["blocked", "not queued"]);
        let mut tracker = LifecycleStateTracker::default();
        let mut emitted = Vec::new();

        let publish_frames = |app: &AppState,
                              tracker: &mut LifecycleStateTracker,
                              emitted: &mut Vec<LifecycleState>| {
            for _ in 0..5 {
                let snapshot = TuiLifecycleSnapshot::from_app(app);
                let state = snapshot.lifecycle_state();
                if tracker.should_emit(state, &snapshot.lifecycle_context("/repo")) {
                    emitted.push(state);
                }
            }
        };

        publish_frames(&app, &mut tracker, &mut emitted);
        // The wait changes only its blocker word: the same semantic state.
        app.changes[0].set_display_status_cache("stalled");
        publish_frames(&app, &mut tracker, &mut emitted);
        assert_eq!(emitted, vec![LifecycleState::Blocked]);

        app.changes[1].set_display_status_cache("queued");
        publish_frames(&app, &mut tracker, &mut emitted);
        assert_eq!(
            emitted,
            vec![LifecycleState::Blocked, LifecycleState::Working]
        );
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
