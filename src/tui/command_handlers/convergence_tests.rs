//! Same-process convergence between the live TUI and `/api/v2`.
//!
//! The bug this file exists for was invisible to every single-frontend test: a
//! remote command mutated the shared reducer and mark store, the Web snapshot
//! reported the new state, and the TUI kept rendering the old one until some
//! unrelated event happened to move it. Nothing was wrong with either adapter on
//! its own — the accepted outcome simply never reached the other frontend.
//!
//! So every case here uses *one* of everything: one reducer, one mark store, one
//! resolver ledger, one coordinator, one dispatch owner, one recording scheduler,
//! with a real `AppState` and a real `WebState` attached as peers. A command is
//! submitted through one adapter and the *other* one is what gets asserted on.
//!
//! Integration-scoped by the shared `WebState` projection; no process,
//! repository, or network is involved.

use std::sync::Arc;

use super::tests::{create_test_change, AdapterHarness};
use super::*;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::OperatorMode;
use crate::tui::state::AppState;
use crate::tui::types::AppExecutionMode;
use crate::web::state::{WebEventSink, WebState};

/// One process, two frontends, one dispatch owner.
struct Converged {
    harness: AdapterHarness,
    app: AppState,
    web: Arc<WebState>,
}

impl Converged {
    async fn new(change_ids: &[&str]) -> Self {
        let harness = AdapterHarness::new(change_ids);
        let mut app = harness.app(change_ids);
        app.apply_display_statuses_from_reducer(
            &harness.state.read().await.all_display_statuses(),
        );

        let web = Arc::new(WebState::new(&[]));
        web.set_shared_state(harness.state.clone()).await;
        web.set_execution_marks(harness.marks.clone()).await;
        web.set_parallel_runtime(harness.parallel.clone()).await;
        let changes: Vec<_> = change_ids.iter().map(|id| create_test_change(id)).collect();
        web.update_with_mode(&changes, "select").await;
        web.sync_remote_control_projection().await;

        // The Web frontend joins the boundary the TUI is already on. Two owners
        // here would make every assertion below vacuous.
        harness.attach(Arc::new(WebEventSink::new(web.clone())));

        Self { harness, app, web }
    }

    /// Submit an intent the way a *remote* client does: straight to the shared
    /// transaction, with no TUI involvement at all.
    async fn remote(&mut self, intent: OperatorIntent) -> ApplicationResult {
        let result = self.harness.application.apply(intent).await;
        // One TUI event-processing pass, exactly as the runner performs it.
        self.harness.deliver(&mut self.app).await;
        result
    }

    /// Submit an intent through the TUI adapter and settle the frame.
    async fn local(&mut self, command: TuiCommand) {
        self.harness.run(&mut self.app, command).await;
    }

    fn row(&self, change_id: &str) -> &crate::tui::state::ChangeState {
        self.app
            .changes
            .iter()
            .find(|change| change.id == change_id)
            .expect("the arranged change exists")
    }

    /// The mark a `/api/v2` client would read for this change.
    ///
    /// Read from the published projection rather than the raw monitoring
    /// snapshot: marks live in the shared store and are overlaid when the
    /// snapshot is projected, so the raw snapshot's field is not the value any
    /// client ever sees.
    async fn web_mark(&self, change_id: &str) -> bool {
        let (snapshot, _, _) = self.web.remote_control().projection().snapshot();
        snapshot
            .changes
            .iter()
            .find(|change| change.id == change_id)
            .map(|change| change.execution_marked)
            .unwrap_or_default()
    }

    async fn web_mode(&self) -> String {
        self.web.get_state().await.app_mode
    }
}

/// A remote mark reaches the next TUI event-processing pass.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_remote_mark_reaches_the_next_frame() {
    let mut converged = Converged::new(&["alpha", "beta"]).await;
    assert!(!converged.row("alpha").selected);

    converged
        .remote(OperatorIntent::SetExecutionMark {
            change_id: "alpha".to_string(),
            marked: true,
        })
        .await;

    assert!(
        converged.row("alpha").selected,
        "a remote mark must be visible on the very next TUI pass"
    );
    assert!(converged.web_mark("alpha").await);
    assert!(
        !converged.row("beta").selected,
        "only the named target may be written"
    );
}

/// A local command reaches Web through the same authoritative dispatch.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_local_run_reaches_web() {
    let mut converged = Converged::new(&["alpha"]).await;
    converged.harness.marks.set("alpha", true);

    converged.local(TuiCommand::StartProcessing(Vec::new())).await;

    assert_eq!(
        converged.app.execution_mode,
        AppExecutionMode::Running,
        "the accepted run must move this frontend's mode"
    );
    assert_eq!(
        converged.web_mode().await,
        "running",
        "and the other frontend's, at the same dispatch"
    );
    assert_eq!(
        converged.harness.core_mode.get(),
        OperatorMode::Running,
        "both are projections of one Core value"
    );
    assert_eq!(converged.harness.status("alpha").await, "queued");
}

/// A local command cannot erase a remote delta for a row it never named.
///
/// This is the regression the target-delta rule exists for: a frontend that
/// rebuilt the whole mark store from its own cached rows would silently revoke
/// `beta` while the operator was only ever acting on `alpha`.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_local_command_preserves_unrelated_remote_marks()
{
    let mut converged = Converged::new(&["alpha", "beta"]).await;

    // The API marks `beta`, and the TUI row cache has not rendered it yet.
    converged.harness.marks.set("beta", true);

    converged
        .remote(OperatorIntent::SetExecutionMark {
            change_id: "alpha".to_string(),
            marked: true,
        })
        .await;

    assert!(
        converged.harness.marks.is_marked("beta"),
        "the shared store must retain the API-provided mark"
    );
    assert!(
        converged.row("beta").selected,
        "and the TUI must render it rather than the stale row it held"
    );
    assert!(converged.web_mark("beta").await);
}

/// Queue presentation and execution marks stay separate axes.
///
/// A Running row is checked because it carries queue intent; an Error row can
/// carry hidden explicit-retry intent. Broad synchronization that conflated the
/// two would turn the hidden intent into a checked row the operator never asked
/// for.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_queue_presentation_is_not_mark_authority() {
    let mut converged = Converged::new(&["alpha", "beta"]).await;
    converged
        .harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "beta".to_string(),
            error: "boom".to_string(),
        });
    converged.harness.core_mode.set(OperatorMode::Running);
    converged.app.execution_mode = AppExecutionMode::Running;

    converged
        .remote(OperatorIntent::SetQueueIntent {
            change_id: "alpha".to_string(),
            queued: true,
        })
        .await;

    assert_eq!(
        converged.harness.status("alpha").await,
        "queued",
        "the queue intent really committed"
    );
    assert!(
        !converged.row("beta").selected,
        "an Error row's hidden retry intent must not become a checked row"
    );
    assert!(!converged.harness.marks.is_marked("beta"));
}

/// A successful target dequeue clears only that target's mark.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_dequeue_clears_only_its_target() {
    let mut converged = Converged::new(&["alpha", "beta"]).await;
    converged.harness.marks.set("alpha", true);
    converged.harness.marks.set("beta", true);
    {
        let mut guard = converged.harness.state.write().await;
        for id in ["alpha", "beta"] {
            guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                id.to_string(),
            ));
        }
    }
    converged.harness.core_mode.set(OperatorMode::Running);
    converged.app.execution_mode = AppExecutionMode::Running;

    let result = converged
        .remote(OperatorIntent::StopAndDequeue {
            change_id: "alpha".to_string(),
        })
        .await;
    assert!(
        matches!(
            &result.outcome,
            Ok(ApplicationOutcome::Operator(OperatorOutcome::Dequeued { .. }))
        ),
        "an idle queued row dequeues once cancellation confirms: {result:?}"
    );

    assert!(
        !converged.harness.marks.is_marked("alpha"),
        "the dequeued target loses its mark"
    );
    assert!(
        converged.harness.marks.is_marked("beta"),
        "an unrelated target keeps its mark"
    );
    assert!(!converged.row("alpha").selected);
    assert!(converged.row("beta").selected);
    assert!(converged.web_mark("beta").await);
}

/// A process-level `Stopped` does not revoke execution marks.
///
/// Marks are the operator's next-run intent. A run that was stopped is exactly
/// the case where that intent has to survive, or resuming would silently start
/// nothing.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_process_stop_retains_marks() {
    let mut converged = Converged::new(&["alpha", "beta"]).await;
    converged.harness.marks.set("alpha", true);
    converged.harness.core_mode.set(OperatorMode::Running);
    converged.app.execution_mode = AppExecutionMode::Running;

    converged
        .harness
        .dispatcher
        .dispatch(ExecutionEvent::Stopped)
        .await;
    converged.harness.deliver(&mut converged.app).await;

    assert_eq!(converged.harness.core_mode.get(), OperatorMode::Stopped);
    assert_eq!(converged.app.execution_mode, AppExecutionMode::Stopped);
    assert_eq!(converged.web_mode().await, "stopped");
    assert!(
        converged.harness.marks.is_marked("alpha"),
        "a process-level stop must not revoke the operator's next-run intent"
    );
    assert!(converged.row("alpha").selected);
}

/// A remote graceful stop, cancel-stop, and force stop each reach both frontends.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_remote_stop_family_reaches_both_frontends() {
    let mut converged = Converged::new(&["alpha"]).await;
    converged.harness.scheduler.set_running(true);
    converged.harness.core_mode.set(OperatorMode::Running);
    converged.app.execution_mode = AppExecutionMode::Running;

    converged.remote(OperatorIntent::Stop).await;
    assert_eq!(converged.app.execution_mode, AppExecutionMode::Stopping);
    assert_eq!(converged.web_mode().await, "stopping");

    converged.remote(OperatorIntent::CancelStop).await;
    assert_eq!(converged.app.execution_mode, AppExecutionMode::Running);
    assert_eq!(converged.web_mode().await, "running");

    // No in-flight execution was registered, so the stop settles immediately and
    // publishes the authoritative `Stopped` rather than a waiting variant.
    converged.remote(OperatorIntent::ForceStop).await;
    assert_eq!(converged.app.execution_mode, AppExecutionMode::Stopped);
    assert_eq!(converged.web_mode().await, "stopped");
}

/// A remote resolve reaches the TUI's row cache and resolver state.
#[tokio::test]
async fn accepted_operator_command_tui_convergence_remote_resolve_reaches_the_next_frame() {
    let mut converged = Converged::new(&["alpha"]).await;
    converged
        .harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        });

    converged
        .remote(OperatorIntent::ResolveMerge {
            change_id: "alpha".to_string(),
        })
        .await;

    assert_eq!(
        converged.harness.resolves.active().as_deref(),
        Some("alpha"),
        "the shared ledger owns the single resolver"
    );
    assert!(
        converged.app.is_resolving(),
        "the TUI reads that ledger rather than a cache of its own"
    );
    assert_eq!(converged.app.execution_mode, AppExecutionMode::Running);
    assert!(converged.web.get_state().await.is_resolving);
}
