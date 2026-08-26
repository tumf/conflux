//! Mark-stability settlement as the TUI adapter actually reaches it.
//!
//! The orchestration tests own the policy; these own the *adapter*. What has to
//! be true here is that Space and bulk `x` reach the shared coordinator with no
//! frontend timer and no new key, that an overlay which owns the keypress
//! produces no mark and therefore no deadline, that a parked persistent
//! scheduler is still eligible, that a process with no live scheduler stays
//! mark-only, and that a rejected Start leaves nothing behind for a deadline to
//! discover ten seconds later. It also owns the running-owner admission
//! regression: a live owner already executing one change must admit a second
//! marked target through settlement alone, including when the catalog admitted
//! that mark before the reducer had ever heard of the change.
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

/// A subscription to the settlement pass an armed deadline will produce.
///
/// Captured *before* time advances on purpose. Two harnesses armed in the same
/// paused instant both settle on the first advance, so a target computed after
/// that advance would already be in the past and could never be reached.
struct PendingPass {
    passes: tokio::sync::watch::Receiver<u64>,
    target: u64,
}

fn pending_pass(coordinator: &Arc<MarkSettlementCoordinator>) -> PendingPass {
    let passes = coordinator.passes();
    let target = *passes.borrow() + 1;
    PendingPass { passes, target }
}

impl PendingPass {
    async fn wait(mut self) {
        self.passes
            .wait_for(|observed| *observed >= self.target)
            .await
            .expect("the coordinator outlives its watchers");
    }
}

/// Advance past the stability deadline and wait for the settlement pass.
async fn settle(coordinator: &Arc<MarkSettlementCoordinator>) {
    let pending = pending_pass(coordinator);
    tokio::time::advance(MARK_STABILITY_WINDOW + Duration::from_millis(1)).await;
    pending.wait().await;
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

/// Dispatch the catalog refresh the TUI's own refresh loop produces.
///
/// Through the shared dispatch owner, not straight into the reducer: this is
/// what gives a newly created proposal reducer runtime state, and it is also
/// what runs mark reconciliation, so a test cannot accidentally admit a row the
/// production refresh would have cleaned up. Every named change is committed and
/// clean, i.e. parallel-eligible.
async fn refresh_catalog(harness: &AdapterHarness, change_ids: &[&str]) {
    harness
        .dispatcher
        .dispatch(crate::events::ExecutionEvent::ChangesRefreshed {
            changes: change_ids
                .iter()
                .map(|id| super::tests::create_test_change(id))
                .collect(),
            rejected_changes: Vec::new(),
            committed_change_ids: change_ids.iter().map(|id| (*id).to_string()).collect(),
            uncommitted_file_change_ids: std::collections::HashSet::new(),
            worktree_change_ids: std::collections::HashSet::new(),
            worktree_paths: std::collections::HashMap::new(),
            worktree_not_ahead_ids: std::collections::HashSet::new(),
            merge_wait_ids: std::collections::HashSet::new(),
        })
        .await;
}

/// Put `change_id` into live agent-driven execution, as a dispatched run does.
async fn start_executing(harness: &AdapterHarness, change_id: &str) {
    harness
        .application
        .apply(OperatorIntent::SetQueueIntent {
            change_id: change_id.to_string(),
            queued: true,
        })
        .await;
    harness
        .dispatcher
        .dispatch(crate::events::ExecutionEvent::ApplyStarted {
            change_id: change_id.to_string(),
            command: "echo".to_string(),
        })
        .await;
    assert_eq!(harness.status(change_id).await, "applying");
}

/// A live owner admits a newly marked target that the reducer has not seen yet.
///
/// The production failure this pins, taken from a live v0.6.298 owner: an
/// execution mark was accepted for a proposal created seconds earlier, the
/// stability window expired *before* the next catalog refresh gave that
/// proposal reducer runtime state, and settlement classified it `not_loadable`
/// and discarded the batch. One and a half seconds later the row was tracked,
/// eligible, still marked — and permanently `not queued`, with only a debug line
/// to say why. Nothing but another Start could recover it, which is exactly what
/// a mark on a running owner is supposed to make unnecessary.
///
/// Everything here is the production owner wiring: the shared operator/API
/// transaction, the real reducer, the real `DynamicQueue`, the real dispatch
/// owner with mark reconciliation bound, and a live scheduler with genuinely
/// concurrent active work.
#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_running_owner_admits_a_late_catalog_target() {
    // The owner is running `active` and knows nothing about `alpha` yet.
    let harness = running_harness(&["active"]);
    start_executing(&harness, "active").await;
    let wakes_before = harness.queue_port.wakes();

    // The operator marks `alpha` through the shared operator/API path. The
    // catalog admits it; the reducer has no runtime state for it at all.
    assert!(
        harness.state.read().await.change_runtime("alpha").is_none(),
        "the reducer must not yet track the freshly created proposal"
    );
    harness
        .application
        .apply(OperatorIntent::SetExecutionMark {
            change_id: "alpha".to_string(),
            marked: true,
        })
        .await;
    assert!(coordinator(&harness).is_armed());

    // The window expires first. There is nothing loadable to admit, so this pass
    // must keep the batch rather than answer with the absence of evidence.
    settle(&coordinator(&harness)).await;
    assert_eq!(
        coordinator(&harness).pending_snapshot(),
        Some(vec!["alpha".to_string()]),
        "an unloadable target keeps its deadline instead of losing the mark"
    );
    assert_eq!(
        harness.queue_port.wakes(),
        wakes_before,
        "nothing was applied"
    );

    // The refresh loop catches up and the row becomes loadable.
    refresh_catalog(&harness, &["active", "alpha"]).await;
    assert!(
        harness.marks.is_marked("alpha"),
        "the mark survives the refresh"
    );

    settle(&coordinator(&harness)).await;

    assert_eq!(
        harness.status("alpha").await,
        "queued",
        "a marked, eligible, ordinary target must gain queue intent without another Start; plan={:?}",
        coordinator(&harness).last_plan()
    );
    assert_eq!(
        harness.queue_port.wakes(),
        wakes_before + 1,
        "one applied membership change wakes scheduler analysis exactly once"
    );
    assert!(
        !coordinator(&harness).is_armed(),
        "a reconciled batch releases its deadline"
    );
    assert!(
        coordinator(&harness).last_failure().is_none(),
        "a batch that reconciled reports no lifecycle failure"
    );

    // The unrelated active change is untouched in every respect.
    assert_eq!(harness.status("active").await, "applying");
    assert!(harness.queue.contains("alpha").await);
}

/// A mark for a change that never becomes loadable stops silently waiting.
///
/// The bound on the repair: retaining the batch may not become an unbounded
/// re-arm loop, and the operator must be told the accepted mark produced no
/// admission instead of watching a marked row sit at `not queued` forever.
#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_unloadable_target_reports_a_stable_reason() {
    let harness = running_harness(&["active"]);
    start_executing(&harness, "active").await;

    harness
        .application
        .apply(OperatorIntent::SetExecutionMark {
            change_id: "ghost".to_string(),
            marked: true,
        })
        .await;

    for _ in 0..crate::orchestration::mark_settlement::MARK_SETTLEMENT_ATTEMPTS {
        settle(&coordinator(&harness)).await;
    }

    assert!(
        !coordinator(&harness).is_armed(),
        "the retry budget is finite"
    );
    assert_eq!(
        coordinator(&harness).last_failure(),
        Some(crate::orchestration::mark_settlement::MarkSettlementFailure::UnreconciledBatch),
    );
    expect_no_settlement(&coordinator(&harness)).await;
    assert_eq!(harness.status("active").await, "applying");
}

/// Which adapter accepted the mark must not change what settlement does.
#[derive(Debug, Clone, Copy)]
enum MarkEntryPoint {
    /// TUI `Space` on the cursor row.
    Space,
    /// TUI bulk `x` over every visible row.
    BulkX,
    /// `cflx client` / `/api/v2`, through the shared application transaction.
    Api,
}

impl MarkEntryPoint {
    /// Mark `change_id` the way this adapter does, and drain what it recorded.
    async fn mark(self, harness: &AdapterHarness, change_id: &str) {
        match self {
            Self::Space => {
                let mut app = harness.app(&[change_id]);
                app.execution_mode = AppExecutionMode::Running;
                app.cursor_index = 0;
                app.toggle_selection();
                drain_marks(harness, &mut app).await;
            }
            Self::BulkX => {
                let mut app = harness.app(&[change_id]);
                app.execution_mode = AppExecutionMode::Running;
                assert!(
                    crate::tui::key_handlers::handle_bulk_toggle_key(&mut app).is_empty(),
                    "a bulk mark emits no TUI command"
                );
                drain_marks(harness, &mut app).await;
            }
            Self::Api => {
                harness
                    .application
                    .apply(OperatorIntent::SetExecutionMark {
                        change_id: change_id.to_string(),
                        marked: true,
                    })
                    .await;
            }
        }
    }
}

/// Every mark adapter converges on the same settlement, and disturbs nothing.
///
/// The scope the repair must not widen: a retained batch re-reads its own
/// targets and only those, so an unmarked row the operator queued explicitly and
/// a marked row they removed from the queue explicitly both keep their intent
/// across every extra pass the retry costs.
#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_every_mark_adapter_settles_a_late_catalog_target() {
    for entry in [
        MarkEntryPoint::Space,
        MarkEntryPoint::BulkX,
        MarkEntryPoint::Api,
    ] {
        let harness = running_harness(&["active", "queued_only", "marked_but_removed"]);
        start_executing(&harness, "active").await;

        // An unmarked row the operator queued explicitly.
        harness
            .application
            .apply(OperatorIntent::SetQueueIntent {
                change_id: "queued_only".to_string(),
                queued: true,
            })
            .await;
        // A marked row the operator then removed from the queue explicitly.
        harness
            .application
            .apply(OperatorIntent::SetExecutionMark {
                change_id: "marked_but_removed".to_string(),
                marked: true,
            })
            .await;
        settle(&coordinator(&harness)).await;
        harness
            .application
            .apply(OperatorIntent::SetQueueIntent {
                change_id: "marked_but_removed".to_string(),
                queued: false,
            })
            .await;
        let wakes_before = harness.queue_port.wakes();

        entry.mark(&harness, "alpha").await;
        assert!(
            coordinator(&harness).is_armed(),
            "{entry:?} must reach the shared coordinator"
        );

        settle(&coordinator(&harness)).await;
        refresh_catalog(
            &harness,
            &["active", "queued_only", "marked_but_removed", "alpha"],
        )
        .await;
        settle(&coordinator(&harness)).await;

        assert_eq!(
            harness.status("alpha").await,
            "queued",
            "{entry:?} did not admit the late-catalog target"
        );
        assert_eq!(
            harness.queue_port.wakes(),
            wakes_before + 1,
            "{entry:?} woke scheduler analysis more than once"
        );
        assert_eq!(harness.status("active").await, "applying");
        assert_eq!(
            harness.status("queued_only").await,
            "queued",
            "{entry:?} disturbed an explicit queue addition"
        );
        assert_eq!(
            harness.status("marked_but_removed").await,
            "not queued",
            "{entry:?} undid an explicit queue removal"
        );
    }
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

    // One advance settles both: paused time is shared, so the two deadlines
    // armed in the same instant expire together. Both subscriptions are taken
    // before it for exactly that reason.
    let space_pass = pending_pass(&coordinator(&space));
    let bulk_pass = pending_pass(&coordinator(&bulk));
    tokio::time::advance(MARK_STABILITY_WINDOW + Duration::from_millis(1)).await;
    space_pass.wait().await;
    bulk_pass.wait().await;

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
    assert!(
        harness.queue.pop().await.is_none(),
        "DynamicQueue is untouched"
    );
}

#[tokio::test(start_paused = true)]
async fn running_mark_reanalysis_rejected_start_leaves_no_delayed_queue_effect() {
    // A parked persistent scheduler: live, so a deadline *could* be armed, which
    // is what makes "none was armed" a real assertion rather than a vacuous one.
    let harness = running_harness(&["alpha"]);
    let mut app = harness.app(&["alpha"]);
    app.execution_mode = AppExecutionMode::Select;
    // Arranged on the row and published from it, the way production does: an
    // `AppState` republishes the whole eligibility set from its own rows, so a
    // set written straight into the shared store would be erased the moment the
    // app joined it — and the Start fence would then admit the target.
    app.changes[0].parallel_eligibility =
        crate::orchestration::operator_command::ParallelEligibility::UncommittedProposalFiles;
    app.publish_parallel_runtime();
    assert_eq!(harness.parallel.ineligible_ids(), vec!["alpha".to_string()]);

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
