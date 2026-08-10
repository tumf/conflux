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
use crate::orchestration::operator_command::{HookRunnerQueueHooks, OperatorCommandService};
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
