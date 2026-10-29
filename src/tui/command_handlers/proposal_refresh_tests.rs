//! Candidate proposal refresh through the TUI adapter.
//!
//! Integration-scoped: the real application transaction and a disposable
//! real-Git candidate. The property under test is that `p` → preview → `Enter`
//! submits exactly the intent `/api/v2 refresh_proposal` submits, and settles
//! through the same shared service.

use std::sync::Arc;

use tokio::sync::mpsc;

use super::*;
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, WorktreeBaseDirCandidates,
};
use crate::orchestration::operator_coordinator::CoreMode;
use crate::orchestration::proposal_refresh::tests::{Fixture as GitFixture, CHANGE};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService, StartEligibility};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::CommandSpec;
use crate::web::remote_control_api::executor::SharedServiceExecutor;

#[tokio::test]
#[cfg_attr(not(feature = "heavy-tests"), ignore)]
async fn candidate_proposal_refresh_tui_submits_the_same_intent_the_api_submits() {
    let git = GitFixture::new();
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec![CHANGE.to_string()],
        10,
    )));
    state
        .write()
        .await
        .apply_execution_event(&crate::events::ExecutionEvent::ProcessingError {
            id: CHANGE.to_string(),
            error: "stale proposal".to_string(),
        });
    let marks = Arc::new(ExecutionMarkStore::new());
    let scheduler = Arc::new(RecordingScheduler::new());
    let service = Arc::new(
        OperatorCommandService::new(
            state.clone(),
            Arc::new(crate::tui::queue::DynamicQueue::new()),
            Arc::new(NoopQueueHooks),
            marks.clone(),
        )
        .with_candidate_worktrees(Arc::new(WorktreeBaseDirCandidates::new(
            git.root.clone(),
            git.worktree.parent().unwrap().to_path_buf(),
        ))),
    );
    let run_control = Arc::new(RunControlService::new(
        state.clone(),
        service,
        scheduler.clone(),
        Arc::new(ResolveReservations::new()),
        Arc::new(StartEligibility::new()),
    ));
    let core_mode = Arc::new(CoreMode::new());
    let dispatcher = Arc::new(
        crate::events::EventDispatcher::new(state.clone(), Vec::new())
            .with_core_mode(Some(core_mode.clone())),
    );
    let application = Arc::new(OperatorApplication::new(core_mode, run_control, dispatcher));
    let (tx, _rx) = mpsc::channel(16);
    let (submissions_tx, mut submissions_rx) = mpsc::channel::<Submission>(16);
    let (feedback_tx, mut feedback_rx) = mpsc::channel::<CommandFeedback>(16);
    let config = OrchestratorConfig::default();
    let worktree_service = build_worktree_service(&git.root, &config, &tx);

    let mut app = AppState::new(vec![crate::openspec::Change {
        id: CHANGE.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }]);
    app.changes[0].set_display_status_cache("error");
    app.changes[0].has_worktree = true;
    app.set_base_branch(Some("develop"));

    // `p`: the shared read-only preview, delivered back as feedback.
    let preview_command = app
        .request_proposal_refresh()
        .expect("an eligible row previews");
    {
        let mut ctx = TuiCommandContext {
            app: &mut app,
            tx: &tx,
            application: &application,
            submissions: &submissions_tx,
            feedback: &feedback_tx,
            worktree_service: &worktree_service,
        };
        handle_tui_command(preview_command, &mut ctx, &state)
            .await
            .unwrap();
    }
    let previewed = feedback_rx.recv().await.expect("the preview settles");
    apply_command_feedback(&mut app, previewed);
    let Some(crate::tui::types::ModalState::ConfirmProposalRefresh { preview }) = app.modal.clone()
    else {
        panic!("the preview opens the confirmation popup");
    };
    assert!(preview.eligible, "{preview:?}");
    assert!(
        submissions_rx.try_recv().is_err(),
        "a preview submits no intent at all"
    );

    // `Enter`: the apply, queued for the shared transaction.
    let apply_command = app.confirm_proposal_refresh().expect("Enter submits");
    {
        let mut ctx = TuiCommandContext {
            app: &mut app,
            tx: &tx,
            application: &application,
            submissions: &submissions_tx,
            feedback: &feedback_tx,
            worktree_service: &worktree_service,
        };
        handle_tui_command(apply_command, &mut ctx, &state)
            .await
            .unwrap();
    }
    let submission = submissions_rx.try_recv().expect("the apply is queued");

    // Identical to what `/api/v2` derives from the same previewed expectations.
    let api_intent = SharedServiceExecutor::intent(&CommandSpec::RefreshProposal {
        change_id: CHANGE.to_string(),
        source_ref: "develop".to_string(),
        expected_source: git.develop(),
        expected_head: git.head(),
    })
    .expect("refresh_proposal is operator intent");
    assert_eq!(submission.intent, api_intent);

    let before = git.head();
    let settled = submission.run(&application).await;
    assert!(settled.warning.is_none(), "{:?}", settled.log);
    assert!(
        settled.log.message.contains("refreshed"),
        "{:?}",
        settled.log
    );
    assert_eq!(git.rev(&git.worktree, "HEAD^"), before);
    assert_eq!(state.read().await.display_status(CHANGE), "error");
    assert!(scheduler.calls().is_empty());
    assert!(!marks.is_marked(CHANGE));
}
