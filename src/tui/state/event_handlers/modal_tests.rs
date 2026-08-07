//! Execution-event ownership versus modal validity.
//!
//! Event handlers own row state, timers, `current_change`, and `StopMode`. The
//! modal axis is theirs only through the explicit invalidation policy applied
//! after dispatch, so these tests pin both halves: a background transition must
//! not clear a still-valid overlay, and it must clear one whose target is gone.

use std::path::PathBuf;

use crate::openspec::{Change, ProposalMetadata};
use crate::tui::events::OrchestratorEvent;
use crate::tui::state::AppState;
use crate::tui::types::{AppExecutionMode, ModalState, StopMode, WorktreeInfo};

fn change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn worktree(path: &str, branch: &str) -> WorktreeInfo {
    WorktreeInfo {
        path: PathBuf::from(path),
        head: "abc1234".to_string(),
        branch: branch.to_string(),
        is_detached: false,
        is_main: false,
        merge_conflict: None,
        has_commits_ahead: false,
        is_merging: false,
        inspection: crate::worktree_ops::InspectionState::Checked,
    }
}

/// Running app with one active change, one idle change, and web monitoring on.
fn running_app() -> AppState {
    let mut app = AppState::new(vec![change("change-a"), change("change-b")]);
    app.execution_mode = AppExecutionMode::Running;
    app.changes[0].set_display_status_cache("applying");
    app.changes[1].set_display_status_cache("not queued");
    app.web_url = Some("http://127.0.0.1:8080".to_string());
    app.worktrees = vec![worktree("/tmp/wt-b", "change-b")];
    app
}

fn force_kill_modal() -> ModalState {
    ModalState::ConfirmForceKill {
        change_id: "change-a".to_string(),
    }
}

fn delete_modal() -> ModalState {
    ModalState::ConfirmWorktreeDelete {
        path: PathBuf::from("/tmp/wt-b"),
        branch: "change-b".to_string(),
    }
}

#[test]
fn change_local_processing_error_preserves_execution_mode_and_valid_modals() {
    for modal in [ModalState::QrPopup, delete_modal()] {
        let mut app = running_app();
        app.modal = Some(modal.clone());

        app.handle_orchestrator_event(OrchestratorEvent::ProcessingError {
            id: "change-b".to_string(),
            error: "boom".to_string(),
        });

        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "a change-local failure must not move the execution axis"
        );
        assert_eq!(app.changes[1].display_status_cache, "error");
        assert_eq!(
            app.modal,
            Some(modal),
            "a change-local failure must not clear a still-valid overlay"
        );
    }
}

#[test]
fn fatal_global_error_enters_error_mode_and_keeps_qr_while_clearing_force_kill() {
    let mut qr = running_app();
    qr.modal = Some(ModalState::QrPopup);
    qr.handle_orchestrator_event(OrchestratorEvent::Error {
        message: "fatal orchestration failure".to_string(),
    });
    assert_eq!(qr.execution_mode, AppExecutionMode::Error);
    assert_eq!(
        qr.modal,
        Some(ModalState::QrPopup),
        "QR carries no destructive payload, so Error does not invalidate it"
    );

    let mut kill = running_app();
    kill.modal = Some(force_kill_modal());
    kill.handle_orchestrator_event(OrchestratorEvent::Error {
        message: "fatal orchestration failure".to_string(),
    });
    assert_eq!(kill.execution_mode, AppExecutionMode::Error);
    assert!(
        kill.modal.is_none(),
        "Error has no in-flight task to stop and dequeue"
    );
}

#[test]
fn fatal_error_retains_handler_ownership_of_current_change_and_error_row() {
    let mut app = running_app();
    app.current_change = Some("change-a".to_string());
    app.modal = Some(ModalState::QrPopup);

    app.handle_orchestrator_event(OrchestratorEvent::Error {
        message: "fatal orchestration failure".to_string(),
    });

    assert_eq!(app.execution_mode, AppExecutionMode::Error);
    assert_eq!(app.current_change, None);
    assert_eq!(app.error_change_id, None);
}

#[test]
fn stopped_event_clears_a_force_kill_confirmation_atomically() {
    let mut app = running_app();
    app.stop_mode = StopMode::GracefulPending;
    app.modal = Some(force_kill_modal());

    app.handle_orchestrator_event(OrchestratorEvent::Stopped);

    assert_eq!(app.execution_mode, AppExecutionMode::Stopped);
    assert_eq!(app.stop_mode, StopMode::None);
    // The row's own status is the reducer's answer to the same stop; this
    // handler owns the mode, the stop mode, and the modal only.
    assert!(
        app.modal.is_none(),
        "the modal and its target payload are cleared together"
    );
}

#[test]
fn stopped_event_keeps_a_valid_worktree_confirmation_visible() {
    let mut app = running_app();
    app.modal = Some(delete_modal());

    app.handle_orchestrator_event(OrchestratorEvent::Stopped);

    assert_eq!(app.execution_mode, AppExecutionMode::Stopped);
    assert_eq!(
        app.modal,
        Some(delete_modal()),
        "the confirmation target is still present and delete-eligible"
    );
}

#[test]
fn running_to_stopping_keeps_force_kill_while_the_target_stays_active() {
    let mut app = running_app();
    app.modal = Some(force_kill_modal());

    // The stop request moves the execution axis; the target keeps running.
    app.execution_mode = AppExecutionMode::Stopping;
    app.stop_mode = StopMode::GracefulPending;
    app.handle_orchestrator_event(OrchestratorEvent::Log(crate::tui::events::LogEntry::info(
        "stopping after current change",
    )));

    assert_eq!(app.execution_mode, AppExecutionMode::Stopping);
    assert_eq!(app.modal, Some(force_kill_modal()));
}

#[test]
fn all_completed_transition_clears_a_force_kill_confirmation() {
    let mut app = running_app();
    app.modal = Some(force_kill_modal());

    app.handle_orchestrator_event(OrchestratorEvent::AllCompleted);

    assert_eq!(app.execution_mode, AppExecutionMode::Select);
    assert!(app.modal.is_none());
}

#[test]
fn all_completed_retains_terminal_modes_and_still_revalidates_the_modal() {
    for terminal in [AppExecutionMode::Stopped, AppExecutionMode::Error] {
        let mut app = running_app();
        app.execution_mode = terminal;
        app.modal = Some(force_kill_modal());

        app.handle_orchestrator_event(OrchestratorEvent::AllCompleted);

        assert_eq!(
            app.execution_mode, terminal,
            "AllCompleted must not overwrite a retained terminal mode"
        );
        assert!(app.modal.is_none());
    }
}

#[test]
fn worktrees_refresh_invalidates_a_stale_delete_confirmation() {
    let cases: [(&str, Vec<WorktreeInfo>); 4] = [
        ("absent", Vec::new()),
        ("rebranded", vec![worktree("/tmp/wt-b", "change-z")]),
        (
            "main",
            vec![WorktreeInfo {
                is_main: true,
                ..worktree("/tmp/wt-b", "change-b")
            }],
        ),
        (
            "detached",
            vec![WorktreeInfo {
                is_detached: true,
                ..worktree("/tmp/wt-b", "change-b")
            }],
        ),
    ];

    for (name, worktrees) in cases {
        let mut app = running_app();
        app.modal = Some(delete_modal());

        app.handle_orchestrator_event(OrchestratorEvent::WorktreesRefreshed { worktrees });

        assert!(
            app.modal.is_none(),
            "{name}: a fresh observation that breaks identity must clear the confirmation"
        );
        assert_eq!(
            app.execution_mode,
            AppExecutionMode::Running,
            "{name}: invalidation must not rewrite execution state"
        );
    }
}

#[test]
fn worktrees_refresh_keeps_a_matching_delete_confirmation() {
    let mut app = running_app();
    app.modal = Some(delete_modal());

    app.handle_orchestrator_event(OrchestratorEvent::WorktreesRefreshed {
        worktrees: vec![worktree("/tmp/wt-b", "change-b")],
    });

    assert_eq!(app.modal, Some(delete_modal()));
}

#[test]
fn losing_the_web_url_clears_the_qr_overlay_without_touching_execution() {
    let mut app = running_app();
    app.modal = Some(ModalState::QrPopup);
    app.web_url = None;

    app.handle_orchestrator_event(OrchestratorEvent::Log(crate::tui::events::LogEntry::info(
        "web monitoring stopped",
    )));

    assert!(app.modal.is_none());
    assert_eq!(app.execution_mode, AppExecutionMode::Running);
}

#[test]
fn a_change_becoming_active_again_invalidates_its_worktree_confirmation() {
    let mut app = running_app();
    app.modal = Some(delete_modal());

    app.handle_orchestrator_event(OrchestratorEvent::ProcessingStarted("change-b".to_string()));

    assert!(
        app.modal.is_none(),
        "an active change must not be deletable from a stale confirmation"
    );
    assert_eq!(app.execution_mode, AppExecutionMode::Running);
}
