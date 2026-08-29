//! TUI execution-mark ownership across the authoritative dispatch boundary.
//!
//! `ChangeState::selected` is a projection of `ExecutionMarkStore`, not a second
//! authority. These tests start from a *deliberately divergent* row/store pair
//! and prove the store wins: a row that still shows `[x]` after a revoking event
//! is exactly the drift that let the TUI and `/api/v2` disagree about the same
//! change.
//!
//! Everything here is composed the way the runner composes it — the shared
//! reducer, the shared mark store, the shared mutation guard, the real
//! `dispatch_event_with_marks`, and the real operator command service — so a
//! frontend-local shortcut cannot pass.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::AppState;
use crate::events::{dispatch_event_with_marks, EventSink, ExecutionEvent};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::mark_reconciliation::ExecutionMarkReconciler;
use crate::orchestration::operator_command::{
    HookRunnerQueueHooks, NoOpReason, OperatorCommandService, OperatorOutcome,
};
use crate::orchestration::operator_coordinator::operator_outcome_event;
use crate::orchestration::state::OrchestratorState;
use crate::tui::queue::DynamicQueue;
use crate::tui::types::AppExecutionMode;

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

/// A TUI wired to the same reducer, mark store, and mutation guard a run binds.
struct Harness {
    app: AppState,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    reconciler: ExecutionMarkReconciler,
    operator: Arc<OperatorCommandService>,
    sinks: Vec<Arc<dyn EventSink>>,
}

impl Harness {
    fn new(change_ids: &[&str]) -> Self {
        let mut app = AppState::new(change_ids.iter().map(|id| change(id)).collect());
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        app.set_shared_state(reducer.clone());

        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        let operator = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                Arc::new(DynamicQueue::new()),
                Arc::new(HookRunnerQueueHooks::new(
                    crate::hooks::HookRunner::with_event_tx(
                        Default::default(),
                        std::path::PathBuf::from("."),
                        tx,
                    ),
                )),
                app.execution_marks(),
            )
            .with_parallel(app.parallel_runtime()),
        );

        Self {
            reconciler: ExecutionMarkReconciler::new(app.execution_marks(), app.parallel_runtime()),
            app,
            reducer,
            operator,
            sinks: Vec::new(),
        }
    }

    /// Dispatch through the authoritative owner, then let the frontend paint —
    /// the exact order the run loop uses.
    async fn dispatch(&mut self, event: ExecutionEvent) {
        dispatch_event_with_marks(
            &self.reducer,
            &self.sinks,
            event.clone(),
            Some(&self.reconciler),
        )
        .await;
        let statuses = self.reducer.read().await.all_display_statuses();
        self.app.apply_display_statuses_from_reducer(&statuses);
        self.app.handle_orchestrator_event(event);
    }

    /// Submit an execution-mark request the way every *remote* operator surface
    /// does, and let the frontend paint what it published.
    ///
    /// `/api/v2`, `cflx client`, and MCP differ only in transport: each resolves
    /// to `OperatorIntent::SetExecutionMark`, which the coordinator applies
    /// through this shared service and publishes through
    /// `operator_outcome_event`. Both halves are the production ones here, so a
    /// projection that only worked for a hand-built event cannot pass.
    async fn remote_mark(&mut self, change_id: &str, marked: bool) -> OperatorOutcome {
        let outcome = self
            .operator
            .set_execution_mark(change_id, marked)
            .await
            .unwrap_or_else(|error| panic!("a remote mark on '{change_id}' must settle: {error}"));
        self.publish(&outcome).await;
        outcome
    }

    /// The bulk equivalent, for the `x`/`set_all_execution_marks` route.
    async fn remote_mark_all(&mut self) -> OperatorOutcome {
        let outcome = self
            .operator
            .set_all_execution_marks()
            .await
            .unwrap_or_else(|error| panic!("a remote bulk mark must settle: {error}"));
        self.publish(&outcome).await;
        outcome
    }

    /// Publish an accepted outcome's own event, or run the passive frontend pass
    /// a no-op leaves behind.
    async fn publish(&mut self, outcome: &OperatorOutcome) {
        match operator_outcome_event(outcome) {
            Some(event) => self.dispatch(event).await,
            // A refused or unchanged command broadcasts nothing, so the only
            // thing that reaches the frontend is the next ordinary projection
            // pass — which is exactly what must not acknowledge anything.
            None => self.app.sync_execution_marks_from_store(),
        }
    }

    /// Present `change_id` as newly detected, the way a refresh does.
    fn arrange_new(&mut self, change_id: &str) {
        for row in &mut self.app.changes {
            if row.id == change_id {
                row.is_new = true;
            }
        }
        self.app.new_change_count = self.app.changes.iter().filter(|row| row.is_new).count();
    }

    fn row_is_new(&self, change_id: &str) -> bool {
        self.app
            .changes
            .iter()
            .find(|row| row.id == change_id)
            .unwrap_or_else(|| panic!("row '{change_id}' must exist"))
            .is_new
    }

    /// Force a row/store divergence the way a missed or late frontend update would.
    fn desync_row(&mut self, change_id: &str, selected: bool) {
        for row in &mut self.app.changes {
            if row.id == change_id {
                row.selected = selected;
            }
        }
    }

    fn row_marked(&self, change_id: &str) -> bool {
        self.app
            .changes
            .iter()
            .find(|row| row.id == change_id)
            .unwrap_or_else(|| panic!("row '{change_id}' must exist"))
            .selected
    }

    fn row_status(&self, change_id: &str) -> &str {
        &self
            .app
            .changes
            .iter()
            .find(|row| row.id == change_id)
            .unwrap_or_else(|| panic!("row '{change_id}' must exist"))
            .display_status_cache
    }
}

fn refresh(
    active: &[&str],
    rejected: &[&str],
    committed: &[&str],
    dirty: &[&str],
) -> ExecutionEvent {
    ExecutionEvent::ChangesRefreshed {
        changes: active.iter().map(|id| change(id)).collect(),
        rejected_changes: rejected.iter().map(|id| change(id)).collect(),
        committed_change_ids: committed.iter().map(|id| id.to_string()).collect(),
        uncommitted_file_change_ids: dirty.iter().map(|id| id.to_string()).collect(),
        worktree_change_ids: HashSet::new(),
        worktree_paths: HashMap::new(),
        worktree_not_ahead_ids: HashSet::new(),
        merge_wait_ids: HashSet::new(),
    }
}

/// After a system event, every row's mark is the store's answer — never its own.
#[tokio::test]
async fn tui_event_rows_follow_authoritative_marks() {
    // Each case: the revoking event, the target it clears, and an unrelated
    // target whose mark must survive it.
    let cases: Vec<(&str, ExecutionEvent)> = vec![
        (
            "failure",
            ExecutionEvent::ApplyFailed {
                change_id: "alpha".to_string(),
                error: "boom".to_string(),
            },
        ),
        (
            "rejection",
            ExecutionEvent::ChangeRejected {
                change_id: "alpha".to_string(),
                reason: "blocker".to_string(),
            },
        ),
        (
            "dequeue",
            ExecutionEvent::ChangeDequeued {
                change_id: "alpha".to_string(),
            },
        ),
        (
            "on_merged hook failure",
            ExecutionEvent::HookFailed {
                change_id: "alpha".to_string(),
                hook_type: crate::hooks::HookType::OnMerged.config_key().to_string(),
                error: "publish script exited 1".to_string(),
            },
        ),
        (
            "rejected refresh row",
            refresh(&["beta"], &["alpha"], &["alpha", "beta"], &[]),
        ),
        (
            "parallel-ineligible refresh row",
            refresh(&["alpha", "beta"], &[], &["alpha", "beta"], &["alpha"]),
        ),
    ];

    for (name, event) in cases {
        let mut harness = Harness::new(&["alpha", "beta"]);
        harness.app.execution_marks().set("alpha", true);
        harness.app.execution_marks().set("beta", true);
        harness.app.sync_execution_marks_from_store();

        // Deliberate divergence: the row still claims a mark the event is about
        // to revoke, and the unrelated row claims none.
        harness.desync_row("alpha", true);
        harness.desync_row("beta", false);

        harness.dispatch(event).await;

        assert!(
            !harness.row_marked("alpha"),
            "{name}: the row kept a mark the shared store revoked"
        );
        assert!(
            harness.row_marked("beta"),
            "{name}: the row did not recover the mark the shared store still holds"
        );
        assert_eq!(
            harness.app.execution_marks().marked_ids(),
            vec!["beta".to_string()],
            "{name}: reconciliation was not target-scoped"
        );
    }
}

/// Reducer `queued` is queue presentation; it never creates a row mark.
#[tokio::test]
async fn queued_rows_stay_unmarked_without_an_execution_mark() {
    use crate::orchestration::state::ReducerCommand;

    let mut harness = Harness::new(&["alpha"]);
    harness
        .reducer
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue("alpha".to_string()));

    harness
        .dispatch(refresh(&["alpha"], &[], &["alpha"], &[]))
        .await;

    assert_eq!(harness.row_status("alpha"), "queued");
    assert!(
        !harness.row_marked("alpha"),
        "queue intent must not present as an execution mark"
    );
    assert!(harness.app.execution_marks().marked_ids().is_empty());
}

/// An operator action that settles *after* a revoking event mutates only its own
/// target and cannot restore the revoked one.
///
/// The interleaving is deterministic: the event's reconciliation completes
/// first, and only then does the already-started interaction reach the shared
/// service. Before the store became authoritative, that interaction republished
/// the whole cached row set and brought the revoked mark back.
#[tokio::test]
async fn stale_tui_rows_cannot_resurrect_revoked_mark() {
    let mut harness = Harness::new(&["alpha", "beta"]);
    harness.app.execution_marks().set("alpha", true);
    harness.app.sync_execution_marks_from_store();

    // The operator starts marking `beta` while `alpha` is still marked. The rows
    // captured by that interaction are the pre-event ones.
    harness.app.request_mark_write("beta", true);

    // The revoking event settles first, under the shared mutation guard.
    harness
        .dispatch(ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        })
        .await;
    assert!(harness.app.execution_marks().marked_ids().is_empty());

    // Now the interaction settles, through the shared service and the same guard.
    for (change_id, marked) in harness.app.take_pending_mark_writes() {
        harness
            .operator
            .apply_execution_mark(&change_id, marked)
            .await;
    }
    harness.app.sync_execution_marks_from_store();

    assert_eq!(
        harness.app.execution_marks().marked_ids(),
        vec!["beta".to_string()],
        "the interaction must mutate only its requested target"
    );
    assert!(
        !harness.row_marked("alpha"),
        "a stale cached row resurrected a mark the event revoked"
    );
    assert!(harness.row_marked("beta"));
}

/// A cleared mark is recoverable: an explicit re-mark creates fresh intent that a
/// duplicate failure cannot take away, and Start still consumes it.
#[tokio::test]
async fn duplicate_failure_after_remark_preserves_fresh_intent() {
    // Each case pairs the revoking event with a mode whose supported re-mark
    // route is mark-only, so the re-mark leaves the reducer row *steady*. That
    // is what makes the replay a duplicate rather than a genuinely new edge: in
    // Running mode a re-mark on an `error` row is queue intent, which retries the
    // change, and a later failure of that retry is a real new transition that
    // must revoke again.
    let cases = [
        (
            "stopped error row",
            ExecutionEvent::ApplyFailed {
                change_id: "alpha".to_string(),
                error: "boom".to_string(),
            },
            AppExecutionMode::Stopped,
            "error",
        ),
        (
            "running merge-wait recovery row",
            ExecutionEvent::HookFailed {
                change_id: "alpha".to_string(),
                hook_type: crate::hooks::HookType::OnMerged.config_key().to_string(),
                error: "publish script exited 1".to_string(),
            },
            AppExecutionMode::Running,
            "merge wait",
        ),
    ];

    for (name, revoking, mode, expected_status) in cases {
        let mut harness = Harness::new(&["alpha"]);
        harness.app.execution_marks().set("alpha", true);
        harness.app.sync_execution_marks_from_store();

        harness.dispatch(revoking.clone()).await;
        harness.app.execution_mode = mode;
        assert_eq!(harness.row_status("alpha"), expected_status, "{name}");
        assert!(
            !harness.row_marked("alpha"),
            "{name}: the revoking edge must clear both projections"
        );
        assert!(harness.app.execution_marks().marked_ids().is_empty());

        // The operator re-marks the steady recovery row through the shared
        // lifecycle matrix — the same route `/api/v2` takes.
        harness
            .operator
            .set_execution_mark("alpha", true)
            .await
            .unwrap_or_else(|error| {
                panic!("{name}: a steady recovery row must accept a fresh mark: {error}")
            });
        harness.app.sync_execution_marks_from_store();
        assert!(harness.row_marked("alpha"), "{name}");
        assert_eq!(
            harness.reducer.read().await.display_status("alpha"),
            expected_status,
            "{name}: a mark-only re-mark must leave the reducer row steady"
        );

        // The same event again creates no new reducer edge, so the fresh intent
        // survives in the store and in the row.
        harness.dispatch(revoking).await;
        assert_eq!(
            harness.app.execution_marks().marked_ids(),
            vec!["alpha".to_string()],
            "{name}: a duplicate delivery discarded a fresh re-mark"
        );
        assert!(harness.row_marked("alpha"), "{name}");

        // And the existing retry route still consumes the fresh intent, unchanged.
        let plan = harness.operator.retry_errors(&["alpha".to_string()]).await;
        if expected_status == "error" {
            assert_eq!(plan.change_ids, vec!["alpha".to_string()], "{name}");
            assert!(
                plan.explicit_retry,
                "{name}: retry must keep explicit-retry semantics"
            );
        } else {
            assert!(
                plan.change_ids.is_empty(),
                "{name}: merge-wait recovery is not a retry route"
            );
        }
        assert_eq!(
            harness.app.execution_marks().marked_ids(),
            vec!["alpha".to_string()],
            "{name}: retry routing must not disturb the mark"
        );
    }
}

// ============================================================================
// NEW attention state across the operator-mark boundary
// ============================================================================
//
// `NEW` is ephemeral frontend attention state, but the *interaction* that
// retires it is not frontend-local: it settles in the shared mark store, and
// `/api/v2`, `cflx client`, and MCP all reach that store through the same
// service the TUI does. The bug these cover is a change rendering as both
// remotely marked and still new, purely because the acknowledgement lived in
// key handling instead of in the projection every frontend shares.

/// A remote mark or unmark retires the target's badge and nothing else's.
#[tokio::test]
async fn remote_mark_interaction_acknowledges_new_attention() {
    // Both directions of a real operator interaction. Unmarking is arranged
    // from an already-marked row so the request genuinely flips the store.
    for (name, arrange_marked, requested) in [("mark", false, true), ("unmark", true, false)] {
        let mut harness = Harness::new(&["alpha", "beta"]);
        if arrange_marked {
            harness.app.execution_marks().set("alpha", true);
            harness.app.sync_execution_marks_from_store();
        }
        harness.arrange_new("alpha");
        harness.arrange_new("beta");
        assert_eq!(harness.app.new_change_count, 2, "{name}");

        let outcome = harness.remote_mark("alpha", requested).await;

        assert!(
            matches!(outcome, OperatorOutcome::MarkSet { ref change_id, marked }
                if change_id == "alpha" && marked == requested),
            "{name}: the request must settle as a real mark delta, not a no-op"
        );
        assert!(
            !harness.row_is_new("alpha"),
            "{name}: a settled remote interaction left the target rendering as new"
        );
        assert_eq!(
            harness.row_marked("alpha"),
            requested,
            "{name}: the row must also project the requested mark"
        );
        assert!(
            harness.row_is_new("beta"),
            "{name}: an unrelated change lost its badge"
        );
        assert_eq!(
            harness.app.new_change_count, 1,
            "{name}: the footer must count exactly the rows that kept a badge"
        );
    }
}

/// A remote bulk mark acknowledges every row it actually flipped.
#[tokio::test]
async fn remote_bulk_mark_acknowledges_only_the_rows_it_changed() {
    let mut harness = Harness::new(&["alpha", "beta"]);
    // `beta` already holds the mark the bulk plan is about to apply, so the bulk
    // write flips `alpha` only — and only `alpha` is an interaction.
    harness.app.execution_marks().set("beta", true);
    harness.app.sync_execution_marks_from_store();
    harness.arrange_new("alpha");
    harness.arrange_new("beta");

    let outcome = harness.remote_mark_all().await;

    assert!(
        matches!(outcome, OperatorOutcome::BulkMarks { marked: true, ref changed, .. }
            if changed == &vec!["alpha".to_string()]),
        "the bulk plan must report exactly the rows it flipped"
    );
    assert!(
        !harness.row_is_new("alpha"),
        "the flipped row must be acknowledged"
    );
    assert!(
        harness.row_is_new("beta"),
        "a row the bulk write did not move is not an interaction"
    );
    assert_eq!(harness.app.new_change_count, 1);
}

/// Passive projection of the shared store acknowledges nothing.
#[tokio::test]
async fn passive_mark_synchronization_preserves_new_attention() {
    let mut harness = Harness::new(&["alpha"]);
    harness.arrange_new("alpha");

    // The store is written the way a non-operator writer does — no service, no
    // settlement, no interaction — and then projected repeatedly.
    harness.app.execution_marks().set("alpha", true);
    for _ in 0..3 {
        harness.app.sync_execution_marks_from_store();
    }
    // A whole refresh pass is the other passive route into the same projection.
    harness
        .dispatch(refresh(&["alpha"], &[], &["alpha"], &[]))
        .await;

    assert!(
        harness.row_marked("alpha"),
        "the passive projection must still mirror the store"
    );
    assert!(
        harness.row_is_new("alpha"),
        "synchronizing a mark nobody just set is not an operator interaction"
    );
    assert_eq!(harness.app.new_change_count, 1);
}

/// A remote request that changes nothing is not an interaction.
#[tokio::test]
async fn unchanged_remote_request_does_not_acknowledge_new_attention() {
    let mut harness = Harness::new(&["alpha"]);
    harness.app.execution_marks().set("alpha", true);
    harness.app.sync_execution_marks_from_store();
    harness.arrange_new("alpha");

    // The authoritative mark already equals the requested value.
    let outcome = harness.remote_mark("alpha", true).await;

    assert!(
        matches!(
            outcome,
            OperatorOutcome::NoOp {
                reason: NoOpReason::MarkUnchanged,
                ..
            }
        ),
        "the request must settle as an unchanged no-op"
    );
    assert!(
        harness.row_is_new("alpha"),
        "a no-op carries no new operator interaction to acknowledge"
    );
    assert_eq!(harness.app.new_change_count, 1);
}

/// Lifecycle-driven mark revocation is the system acting, not the operator.
#[tokio::test]
async fn system_mark_revocation_does_not_acknowledge_new_attention() {
    let mut harness = Harness::new(&["alpha"]);
    harness.app.execution_marks().set("alpha", true);
    harness.app.sync_execution_marks_from_store();
    harness.arrange_new("alpha");

    // The reconciler revokes the mark on the failure edge, through the real
    // dispatch boundary — the one mark write that never arms settlement.
    harness
        .dispatch(ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        })
        .await;

    assert!(
        !harness.row_marked("alpha"),
        "the revocation must still reach the row"
    );
    assert!(
        harness.row_is_new("alpha"),
        "a mark the system took away was never looked at by an operator"
    );
    assert_eq!(harness.app.new_change_count, 1);
}

/// The local TUI toggle keeps its own behavior, in every execution mode.
///
/// One case per mode rather than the historical Select/Running split: both ran
/// through this single execution-mark toggle path already, so a mode-specific
/// rule would be a rule no code expresses.
#[tokio::test]
async fn local_toggle_acknowledges_new_attention_in_every_mode() {
    for mode in [
        AppExecutionMode::Select,
        AppExecutionMode::Running,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ] {
        let mut harness = Harness::new(&["alpha", "beta"]);
        harness.app.execution_mode = mode;
        harness.arrange_new("alpha");
        harness.arrange_new("beta");

        harness.app.cursor_index = 0;
        harness.app.toggle_selection();

        assert!(!harness.row_is_new("alpha"), "{mode:?}");
        assert!(harness.row_marked("alpha"), "{mode:?}");
        assert!(harness.row_is_new("beta"), "{mode:?}");
        assert_eq!(harness.app.new_change_count, 1, "{mode:?}");

        // The deferred write then settles through the shared service, and the
        // projection it triggers must not double-count anything.
        for (change_id, marked) in harness.app.take_pending_mark_writes() {
            harness
                .operator
                .apply_execution_mark(&change_id, marked)
                .await;
        }
        harness.app.sync_execution_marks_from_store();

        assert!(harness.row_marked("alpha"), "{mode:?}");
        assert!(harness.row_is_new("beta"), "{mode:?}");
        assert_eq!(harness.app.new_change_count, 1, "{mode:?}");
    }
}

/// Acknowledgement writes attention state and no workflow state.
#[tokio::test]
async fn remote_mark_acknowledgement_mutates_no_queue_or_lifecycle_state() {
    let mut harness = Harness::new(&["alpha"]);
    harness.arrange_new("alpha");
    let mode_before = harness.app.execution_mode;
    let status_before = harness.row_status("alpha").to_string();

    harness.remote_mark("alpha", true).await;

    assert!(!harness.row_is_new("alpha"));
    assert_eq!(
        harness.row_status("alpha"),
        status_before,
        "acknowledgement must not move the reducer-derived status"
    );
    assert_eq!(
        harness.reducer.read().await.display_status("alpha"),
        status_before,
        "and must not reach the reducer at all"
    );
    assert_eq!(
        harness.app.execution_mode, mode_before,
        "acknowledgement is not a lifecycle transition"
    );
    assert_eq!(
        harness.app.execution_marks().marked_ids(),
        vec!["alpha".to_string()],
        "the mark itself is the only shared mutation"
    );
}

/// A rejected row carries no badge, so there is nothing to acknowledge.
#[tokio::test]
async fn rejected_rows_have_no_new_attention_to_acknowledge() {
    let mut harness = Harness::new(&["alpha", "beta"]);
    harness.arrange_new("beta");

    // `alpha` becomes terminal through the reducer edge the mark admission rules
    // actually read; a rejected row is never given a badge in the first place.
    harness
        .dispatch(ExecutionEvent::ChangeRejected {
            change_id: "alpha".to_string(),
            reason: "blocker".to_string(),
        })
        .await;
    assert_eq!(harness.row_status("alpha"), "rejected");
    assert!(!harness.row_is_new("alpha"));

    let outcome = harness.remote_mark("alpha", true).await;

    assert!(
        matches!(
            outcome,
            OperatorOutcome::NoOp {
                reason: NoOpReason::TerminalMarkTarget,
                ..
            }
        ),
        "a terminal row is not a mark target"
    );
    assert!(
        harness.row_is_new("beta"),
        "and the refused request must not disturb an unrelated badge"
    );
    assert_eq!(harness.app.new_change_count, 1);
}
