//! Mark-stability settlement as the TUI adapter actually reaches it.
//!
//! The orchestration tests own the policy; these own the *adapter*. What has to
//! be true here is that Space and bulk `x` reach the shared coordinator with no
//! frontend timer and no new key, that an overlay which owns the keypress
//! produces no mark and therefore no deadline, that a parked persistent
//! scheduler is still eligible, that a process with no live scheduler stays
//! mark-only, and that a rejected Start leaves nothing behind for a deadline to
//! discover ten seconds later.
//!
//! Integration-scoped within the process: the real coordinator, run-control
//! service, reducer, `DynamicQueue`, and dispatch owner are all wired, with only
//! the scheduler replaced by the shared recording double. No process,
//! repository, or network boundary is touched, and every deadline runs under
//! paused Tokio time.

use std::sync::Arc;
use std::time::Duration;

use super::tests::AdapterHarness;
use super::*;
use crate::orchestration::mark_settlement::{MarkSettlementCoordinator, MARK_STABILITY_WINDOW};
use crate::tui::state::AppState;
use crate::tui::types::AppExecutionMode;

/// Advance past the stability deadline and wait for the settlement pass.
async fn settle(coordinator: &Arc<MarkSettlementCoordinator>) {
    let mut passes = coordinator.passes();
    let target = *passes.borrow() + 1;
    tokio::time::advance(MARK_STABILITY_WINDOW + Duration::from_millis(1)).await;
    passes
        .wait_for(|observed| *observed >= target)
        .await
        .expect("the coordinator outlives its watchers");
}

/// Advance past the deadline and assert no settlement pass ran at all.
async fn expect_no_settlement(coordinator: &Arc<MarkSettlementCoordinator>) {
    let before = *coordinator.passes().borrow();
    tokio::time::advance(MARK_STABILITY_WINDOW * 2).await;
    tokio::task::yield_now().await;
    assert_eq!(
        *coordinator.passes().borrow(),
        before,
        "no settlement pass may run"
    );
}

/// Drain the mark writes an interaction recorded, exactly as the runner does.
async fn drain_marks(harness: &AdapterHarness, app: &mut AppState) {
    let service = harness.application.run_control().operator();
    crate::tui::runner::apply_pending_mark_writes(app, &service).await;
}

fn coordinator(harness: &AdapterHarness) -> Arc<MarkSettlementCoordinator> {
    harness.marks.settlement()
}

/// A harness whose scheduler is live, i.e. capable of dynamic queue admission.
fn running_harness(change_ids: &[&str]) -> AdapterHarness {
    let harness = AdapterHarness::new(change_ids);
    harness.scheduler.set_running(true);
    harness
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_space_and_bulk_x_reach_the_same_settlement() {
    // Space: one row toggled, drained through the shared service.
    let space = running_harness(&["alpha"]);
    let mut app = space.app(&["alpha"]);
    app.execution_mode = AppExecutionMode::Running;
    app.cursor_index = 0;
    app.toggle_selection();
    drain_marks(&space, &mut app).await;
    assert!(
        coordinator(&space).is_armed(),
        "Space must reach the shared coordinator"
    );

    // Bulk `x`: the same store, the same coordinator, still no frontend timer.
    let bulk = running_harness(&["alpha"]);
    let mut bulk_app = bulk.app(&["alpha"]);
    bulk_app.execution_mode = AppExecutionMode::Running;
    assert!(
        crate::tui::key_handlers::handle_bulk_toggle_key(&mut bulk_app).is_empty(),
        "a bulk mark emits no TUI command"
    );
    drain_marks(&bulk, &mut bulk_app).await;
    assert!(
        coordinator(&bulk).is_armed(),
        "bulk `x` must reach the shared coordinator"
    );

    settle(&coordinator(&space)).await;
    settle(&coordinator(&bulk)).await;

    assert_eq!(space.status("alpha").await, "queued");
    assert_eq!(
        space.status("alpha").await,
        bulk.status("alpha").await,
        "both interactions settle into the identical queue state"
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_shared_operator_mark_command_schedules_the_same_settlement() {
    // The `/api/v2` equivalent of Space, through the same application
    // transaction. Neither frontend owns a timer, so both land on one deadline.
    let harness = running_harness(&["alpha"]);
    harness
        .application
        .apply(OperatorIntent::SetExecutionMark {
            change_id: "alpha".to_string(),
            marked: true,
        })
        .await;
    assert!(coordinator(&harness).is_armed());

    settle(&coordinator(&harness)).await;
    assert_eq!(harness.status("alpha").await, "queued");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_overlay_input_ownership_arms_nothing() {
    let harness = running_harness(&["alpha"]);
    let mut app = harness.app(&["alpha"]);
    app.execution_mode = AppExecutionMode::Running;
    app.show_warning_popup("blocked".to_string(), "an overlay owns input".to_string());

    // The overlay consumes the keypress, so `x` never reaches the bulk toggle.
    assert!(
        crate::tui::key_handlers::handle_warning_popup_key(
            &mut app,
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('x'),
                crossterm::event::KeyModifiers::NONE,
            ),
        ),
        "the warning popup must own the keypress"
    );
    drain_marks(&harness, &mut app).await;

    assert!(harness.marks.marked_ids().is_empty());
    assert!(!coordinator(&harness).is_armed());
    expect_no_settlement(&coordinator(&harness)).await;
    assert_eq!(harness.status("alpha").await, "not queued");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_parked_persistent_scheduler_remains_eligible() {
    // A persistent scheduler reports Select while parked, and is still perfectly
    // able to admit work. Scheduler liveness is the authority, not the mode the
    // TUI happens to be presenting.
    let harness = running_harness(&["alpha"]);
    let mut app = harness.app(&["alpha"]);
    app.execution_mode = AppExecutionMode::Select;
    app.persistent_scheduler_idle = true;
    app.cursor_index = 0;
    app.toggle_selection();
    drain_marks(&harness, &mut app).await;

    assert!(coordinator(&harness).is_armed());
    settle(&coordinator(&harness)).await;
    assert_eq!(harness.status("alpha").await, "queued");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_process_without_live_scheduler_stays_mark_only() {
    // Default recording scheduler: nothing running, so nothing to admit into.
    let harness = AdapterHarness::new(&["alpha"]);
    for mode in [
        AppExecutionMode::Select,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ] {
        let mut app = harness.app(&["alpha"]);
        app.execution_mode = mode;
        app.cursor_index = 0;
        app.toggle_selection();
        drain_marks(&harness, &mut app).await;

        assert_eq!(
            harness.marks.is_marked("alpha"),
            app.changes[0].selected,
            "{mode:?} still applies the mark itself"
        );
        assert!(
            !coordinator(&harness).is_armed(),
            "{mode:?} must arm no stability deadline without a live scheduler"
        );
    }

    expect_no_settlement(&coordinator(&harness)).await;
    assert_eq!(harness.status("alpha").await, "not queued");
    assert!(harness.queue.pop().await.is_none(), "DynamicQueue is untouched");
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_rejected_start_leaves_no_delayed_queue_effect() {
    // A parked persistent scheduler: live, so a deadline *could* be armed, which
    // is what makes "none was armed" a real assertion rather than a vacuous one.
    let harness = running_harness(&["alpha"]);
    harness.parallel.set_parallel_ineligible([(
        "alpha".to_string(),
        crate::orchestration::operator_command::ParallelEligibility::UncommittedProposalFiles,
    )]);
    let mut app = harness.app(&["alpha"]);
    app.execution_mode = AppExecutionMode::Select;

    harness
        .run(
            &mut app,
            TuiCommand::StartProcessing(vec!["alpha".to_string()]),
        )
        .await;

    assert!(
        harness.marks.is_marked("alpha"),
        "the admission mark write still happened"
    );
    assert!(
        !coordinator(&harness).is_armed(),
        "a Start-admission mark write must arm no delayed settlement"
    );

    expect_no_settlement(&coordinator(&harness)).await;
    assert_eq!(
        harness.status("alpha").await,
        "not queued",
        "a rejected Start leaves no queue effect, immediate or delayed"
    );
    assert!(harness.queue.pop().await.is_none());
}
