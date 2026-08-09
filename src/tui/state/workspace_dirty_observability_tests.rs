//! The workspace dirty badge is observability, and this is where that is proved.
//!
//! Two TUIs are driven identically — same reducer, same mark store, same
//! operator command service, same event sequence, same commands — and differ
//! only in whether the local refresh task observed a dirty workspace. Every
//! workflow-visible answer must be indistinguishable afterwards; the rendered
//! header is the only place the difference is allowed to exist.
//!
//! This is the constitutional check for the badge: presentation state that could
//! reach a next-action decision would be out-of-band workflow input, and a
//! reviewer cannot see that from the field's type alone.

use std::collections::HashMap;
use std::sync::Arc;

use super::AppState;
use crate::events::{dispatch_event_with_marks, EventSink, ExecutionEvent};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::mark_reconciliation::ExecutionMarkReconciler;
use crate::orchestration::operator_command::{HookRunnerQueueHooks, OperatorCommandService};
use crate::orchestration::state::OrchestratorState;
use crate::tui::events::TuiRefreshObservation;
use crate::tui::queue::DynamicQueue;
use crate::tui::types::{AppExecutionMode, WorkspaceDirtyState};

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

/// A TUI wired the way the runner wires one: shared reducer, shared mark store,
/// shared mutation guard, real dispatch, real operator command service.
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

    /// Dispatch through the authoritative owner, then let the frontend paint.
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

    fn observe_workspace_dirty(&mut self, dirty: bool) {
        self.app
            .adopt_workspace_dirty_observation(TuiRefreshObservation::WorkspaceDirty { dirty });
    }

    /// Everything a workflow decision could be read from, in one comparable value.
    async fn workflow_facts(&self) -> WorkflowFacts {
        let reducer = self.reducer.read().await;
        WorkflowFacts {
            reducer_display_statuses: reducer
                .all_display_statuses()
                .into_iter()
                .map(|(id, status)| (id, status.to_string()))
                .collect(),
            queued: {
                let mut queued = reducer.queued_change_ids();
                queued.sort();
                queued
            },
            queue_eligible: {
                let mut eligible: Vec<String> = reducer
                    .ordinary_queue_eligible_change_ids()
                    .into_iter()
                    .collect();
                eligible.sort();
                eligible
            },
            row_display_statuses: self
                .app
                .changes
                .iter()
                .map(|row| (row.id.clone(), row.display_status_cache.clone()))
                .collect(),
            marks: {
                let mut marked = self.app.execution_marks().marked_ids();
                marked.sort();
                marked
            },
            execution_mode: self.app.execution_mode,
        }
    }

    /// The header row as the operator sees it.
    fn header(&mut self) -> String {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal init");
        terminal
            .draw(|frame| crate::tui::render::render(frame, &mut self.app))
            .expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.width)
            .map(|x| buffer[(x, 1)].symbol())
            .collect()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WorkflowFacts {
    reducer_display_statuses: HashMap<String, String>,
    queued: Vec<String>,
    queue_eligible: Vec<String>,
    row_display_statuses: Vec<(String, String)>,
    marks: Vec<String>,
    execution_mode: AppExecutionMode,
}

/// The same lifecycle traffic and the same operator commands both TUIs see.
async fn drive(harness: &mut Harness, observe_dirty: bool) -> Vec<String> {
    let mut command_outcomes = Vec::new();
    let mode = harness.app.execution_mode.operator_mode();

    if observe_dirty {
        harness.observe_workspace_dirty(true);
    }

    command_outcomes.push(format!(
        "{:?}",
        harness
            .operator
            .set_execution_mark(mode, "alpha", true)
            .await
    ));
    command_outcomes.push(format!(
        "{:?}",
        harness.operator.add_to_queue("alpha").await
    ));
    command_outcomes.push(format!("{:?}", harness.operator.add_to_queue("beta").await));

    harness
        .dispatch(ExecutionEvent::ProcessingStarted("alpha".to_string()))
        .await;

    if observe_dirty {
        harness.observe_workspace_dirty(true);
    }

    harness
        .dispatch(ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "cflx apply".to_string(),
        })
        .await;
    harness
        .dispatch(ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        })
        .await;

    if observe_dirty {
        harness.observe_workspace_dirty(true);
    }

    command_outcomes.push(format!(
        "{:?}",
        harness.operator.remove_from_queue("beta").await
    ));
    command_outcomes.push(format!(
        "{:?}",
        harness
            .operator
            .set_execution_mark(mode, "beta", true)
            .await
    ));

    command_outcomes
}

/// Same inputs, one extra dirty observation: identical workflow, different header.
#[tokio::test]
async fn workspace_dirty_header_is_observability_only() {
    let mut clean = Harness::new(&["alpha", "beta"]);
    let mut dirty = Harness::new(&["alpha", "beta"]);

    let clean_commands = drive(&mut clean, false).await;
    let dirty_commands = drive(&mut dirty, true).await;

    assert_eq!(
        clean.app.workspace_dirty(),
        WorkspaceDirtyState::Unknown,
        "the control TUI must never have observed a dirty workspace"
    );
    assert_eq!(dirty.app.workspace_dirty(), WorkspaceDirtyState::Dirty);

    // Equality is only worth asserting if the drive produced real decisions, so
    // pin what it must have reached before comparing the two runs.
    let facts = dirty.workflow_facts().await;
    assert_eq!(
        facts
            .reducer_display_statuses
            .get("alpha")
            .map(String::as_str),
        Some("error"),
        "the drive must have moved the reducer: {facts:?}"
    );
    assert!(
        dirty_commands.iter().any(|outcome| outcome.contains("Ok(")),
        "the drive must have exercised at least one accepted command: {dirty_commands:?}"
    );

    assert_eq!(
        clean_commands, dirty_commands,
        "command admission must not depend on the dirty observation"
    );
    assert_eq!(
        clean.workflow_facts().await,
        facts,
        "reducer statuses, queue membership, rows, marks, and mode must all be identical"
    );

    let clean_header = clean.header();
    let dirty_header = dirty.header();
    assert!(!clean_header.contains("[dirty]"));
    assert!(dirty_header.contains("[dirty]"));

    // Whitespace-normalized, because removing the badge shifts the
    // right-aligned version area back by its width.
    fn without_badge(header: &str) -> String {
        header
            .replace("[dirty]", "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    assert_eq!(
        without_badge(&clean_header),
        without_badge(&dirty_header),
        "the badge must be the only rendered difference"
    );
}
